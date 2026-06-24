// disk_archive.cpp - see disk_archive.h. Uses uzlib (vendored under src/uzlib,
// originally from the rp2040 core's OTA support) for deflate/inflate.
#include <Arduino.h>
#include <SD.h>
#include <string.h>
#include <stdlib.h>
#include "disk_archive.h"

extern "C" {
#include "src/uzlib/uzlib.h"
}

#define DICT_SIZE   32768u   // inflate window; external gz/zip may use up to 32K
#define OUT_CHUNK    2048u   // streaming output flush buffer
#define GZ_CHUNK    16384u   // multi-member gzip slice (also compressor window)
#define ZIP_INNER  "DISK.DSK"
#define REPACK_TMP "/_REPACK.TMP"

// ---- little-endian helpers --------------------------------------------------
static inline uint16_t rd16(const uint8_t* p) { return p[0] | (p[1] << 8); }
static inline uint32_t rd32(const uint8_t* p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}
static inline void wr16(uint8_t* p, uint16_t v) { p[0] = v & 0xff; p[1] = (v >> 8) & 0xff; }
static inline void wr32(uint8_t* p, uint32_t v) {
    p[0] = v & 0xff; p[1] = (v >> 8) & 0xff; p[2] = (v >> 16) & 0xff; p[3] = (v >> 24) & 0xff;
}

// ---- streaming byte source for uzlib (single-threaded; never re-entrant) ----
// uzlib's source callback receives only the TINF_DATA*, so the file/limit live
// in this file-scope context. Decompression is only ever driven from one place
// at a time (disk load, emu paused), so a static context is safe.
struct ArcSrc {
    File*    f;
    uint32_t remaining;     // bytes still allowed to be read from f
    uint8_t  buf[512];
};
static ArcSrc s_src;

static int arc_src_cb(struct uzlib_uncomp* d) {
    if (s_src.remaining == 0) return -1;
    uint32_t want = s_src.remaining < sizeof(s_src.buf) ? s_src.remaining : sizeof(s_src.buf);
    int n = s_src.f->read(s_src.buf, want);
    if (n <= 0) { s_src.remaining = 0; return -1; }
    s_src.remaining -= (uint32_t)n;
    d->source = s_src.buf + 1;
    d->source_limit = s_src.buf + n;
    return s_src.buf[0];
}

// ---- extension classification ----------------------------------------------
static bool ends_with_ci(const char* s, const char* suf) {
    size_t ls = strlen(s), lf = strlen(suf);
    if (lf > ls) return false;
    const char* p = s + (ls - lf);
    for (size_t i = 0; i < lf; i++) {
        char a = p[i], b = suf[i];
        if (a >= 'a' && a <= 'z') a -= 32;
        if (b >= 'a' && b <= 'z') b -= 32;
        if (a != b) return false;
    }
    return true;
}

int archive_kind(const char* name) {
    if (ends_with_ci(name, ".gz"))  return ARC_GZ;
    if (ends_with_ci(name, ".zip")) return ARC_ZIP;
    if (ends_with_ci(name, ".dsk")) return ARC_RAW;
    return ARC_UNKNOWN;
}

// ---- decompression ----------------------------------------------------------
// Multi-member gzip: loop parsing a gzip header + inflating until the file is
// consumed. Single-member external gz files fall out after one iteration.
static bool extract_gz(File& out) {
    uint8_t* dict = (uint8_t*)malloc(DICT_SIZE);
    uint8_t* outb = (uint8_t*)malloc(OUT_CHUNK);
    if (!dict || !outb) { free(dict); free(outb); return false; }

    // One TINF_DATA reused across members: uzlib_uncompress_init() resets the
    // decode state (btype=-1, bitcount=0, ...) but keeps source/source_limit,
    // so bytes already buffered from the next member are not discarded.
    struct uzlib_uncomp d;
    memset(&d, 0, sizeof(d));
    d.source = NULL; d.source_limit = NULL; d.source_read_cb = arc_src_cb; d.eof = false;

    bool ok = true, more = true;
    while (more && ok) {
        uzlib_uncompress_init(&d, dict, DICT_SIZE);

        if (uzlib_gzip_parse_header(&d) != TINF_OK) { ok = false; break; }

        for (;;) {
            d.dest_start = d.dest = outb;
            d.dest_limit = outb + OUT_CHUNK;
            int r = uzlib_uncompress_chksum(&d);
            if (r < 0) { ok = false; break; }
            uint32_t produced = (uint32_t)(d.dest - outb);
            if (produced && out.write(outb, produced) != produced) { ok = false; break; }
            if (r == TINF_DONE) break;
        }
        if (!ok) break;

        // Another member only if enough bytes remain for a minimal gzip member.
        uint32_t leftover = (uint32_t)(d.source_limit - d.source) + s_src.remaining;
        more = leftover >= 18;
    }
    free(dict); free(outb);
    return ok;
}

// Locate the first entry's data via the central directory (authoritative for
// sizes even when the local header used a data descriptor).
static bool zip_locate(File& in, uint32_t* data_off, uint32_t* csize,
                       uint16_t* method) {
    uint32_t fsize = in.size();
    if (fsize < 22) return false;
    uint32_t tail = fsize < 512 ? fsize : 512;   // EOCD lives in the tail (no comment expected)
    uint8_t buf[512];
    in.seek(fsize - tail);
    if ((uint32_t)in.read(buf, tail) != tail) return false;
    int eocd = -1;
    for (int i = (int)tail - 22; i >= 0; i--) {
        if (buf[i] == 0x50 && buf[i+1] == 0x4b && buf[i+2] == 0x05 && buf[i+3] == 0x06) { eocd = i; break; }
    }
    if (eocd < 0) return false;
    uint32_t cd_off = rd32(buf + eocd + 16);

    uint8_t cd[46];
    in.seek(cd_off);
    if (in.read(cd, 46) != 46) return false;
    if (!(cd[0] == 0x50 && cd[1] == 0x4b && cd[2] == 0x01 && cd[3] == 0x02)) return false;
    *method = rd16(cd + 10);
    *csize  = rd32(cd + 20);
    uint32_t lho = rd32(cd + 42);

    uint8_t lh[30];
    in.seek(lho);
    if (in.read(lh, 30) != 30) return false;
    if (!(lh[0] == 0x50 && lh[1] == 0x4b && lh[2] == 0x03 && lh[3] == 0x04)) return false;
    *data_off = lho + 30 + rd16(lh + 26) + rd16(lh + 28);
    return true;
}

static bool extract_zip(File& in, File& out) {
    uint32_t data_off, csize; uint16_t method;
    if (!zip_locate(in, &data_off, &csize, &method)) return false;
    in.seek(data_off);

    if (method == 0) {                 // stored: straight copy
        uint8_t b[512]; uint32_t left = csize;
        while (left) {
            uint32_t n = left < sizeof(b) ? left : sizeof(b);
            int r = in.read(b, n);
            if (r <= 0) return false;
            if (out.write(b, r) != (size_t)r) return false;
            left -= (uint32_t)r;
        }
        return true;
    }
    if (method != 8) return false;     // only stored/deflate supported

    uint8_t* dict = (uint8_t*)malloc(DICT_SIZE);
    uint8_t* outb = (uint8_t*)malloc(OUT_CHUNK);
    if (!dict || !outb) { free(dict); free(outb); return false; }

    s_src.remaining = csize;           // raw deflate stream is exactly csize bytes
    struct uzlib_uncomp d;
    memset(&d, 0, sizeof(d));
    uzlib_uncompress_init(&d, dict, DICT_SIZE);
    d.source = NULL; d.source_limit = NULL; d.source_read_cb = arc_src_cb; d.eof = false;
    d.checksum_type = TINF_CHKSUM_NONE;

    bool ok = true;
    for (;;) {
        d.dest_start = d.dest = outb;
        d.dest_limit = outb + OUT_CHUNK;
        int r = uzlib_uncompress(&d);
        if (r < 0) { ok = false; break; }
        uint32_t produced = (uint32_t)(d.dest - outb);
        if (produced && out.write(outb, produced) != produced) { ok = false; break; }
        if (r == TINF_DONE) break;
    }
    free(dict); free(outb);
    return ok;
}

bool archive_extract(const char* src_path, const char* dst_path, int kind) {
    uzlib_init();
    if (SD.exists(dst_path)) SD.remove(dst_path);
    File in = SD.open(src_path, FILE_READ);
    if (!in) return false;
    File out = SD.open(dst_path, FILE_WRITE);
    if (!out) { in.close(); return false; }

    s_src.f = &in;
    s_src.remaining = in.size();
    in.seek(0);

    bool ok = (kind == ARC_GZ)  ? extract_gz(out)
            : (kind == ARC_ZIP) ? extract_zip(in, out)
            : false;

    out.flush(); out.close(); in.close();
    if (!ok) SD.remove(dst_path);
    return ok;
}

// ---- compression ------------------------------------------------------------
static bool compress_gz(File& in, File& out) {
    struct uzlib_comp c;
    memset(&c, 0, sizeof(c));
    c.dict_size = GZ_CHUNK;
    c.hash_bits = 12;
    unsigned hash_size = sizeof(uzlib_hash_entry_t) * (1u << c.hash_bits);
    c.hash_table = (uzlib_hash_entry_t*)malloc(hash_size);
    uint8_t* src = (uint8_t*)malloc(GZ_CHUNK);
    c.outsize = GZ_CHUNK + GZ_CHUNK / 2 + 128;     // pre-size to avoid realloc churn
    c.outbuf = (unsigned char*)malloc(c.outsize);
    if (!c.hash_table || !src || !c.outbuf) { free(c.hash_table); free(src); free(c.outbuf); return false; }

    static const uint8_t hdr[10] = { 0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0, 0xff };
    bool ok = true;
    for (;;) {
        int n = in.read(src, GZ_CHUNK);
        if (n < 0) { ok = false; break; }
        if (n == 0) break;

        memset(c.hash_table, 0, hash_size);
        c.outlen = 0; c.outbits = 0; c.noutbits = 0; c.comp_disabled = 0;
        zlib_start_block(&c);
        uzlib_compress(&c, src, (unsigned)n);
        zlib_finish_block(&c);

        uint8_t tr[8];
        wr32(tr, ~uzlib_crc32(src, n, ~0u));
        wr32(tr + 4, (uint32_t)n);
        if (out.write(hdr, 10) != 10 ||
            out.write(c.outbuf, c.outlen) != (size_t)c.outlen ||
            out.write(tr, 8) != 8) { ok = false; break; }
    }
    free(c.hash_table); free(src); free(c.outbuf);
    return ok;
}

static bool compress_zip_stored(File& in, File& out) {
    uint32_t usize = in.size();
    uint8_t b[512];

    // streaming CRC over the whole image
    uint32_t crc = ~0u, left = usize;
    in.seek(0);
    while (left) {
        uint32_t n = left < sizeof(b) ? left : sizeof(b);
        int r = in.read(b, n);
        if (r <= 0) return false;
        crc = uzlib_crc32(b, r, crc);
        left -= (uint32_t)r;
    }
    crc = ~crc;

    const char* inner = ZIP_INNER;
    uint16_t nlen = (uint16_t)strlen(inner);

    uint8_t lh[30]; memset(lh, 0, 30);
    lh[0] = 0x50; lh[1] = 0x4b; lh[2] = 0x03; lh[3] = 0x04;
    wr16(lh + 4, 20);                       // version needed
    // flags=0, method=0 (stored), time/date=0
    wr32(lh + 14, crc);
    wr32(lh + 18, usize);                   // compressed size == uncompressed
    wr32(lh + 22, usize);
    wr16(lh + 26, nlen);                    // extra len = 0
    if (out.write(lh, 30) != 30) return false;
    if (out.write((const uint8_t*)inner, nlen) != nlen) return false;

    in.seek(0); left = usize;
    while (left) {
        uint32_t n = left < sizeof(b) ? left : sizeof(b);
        int r = in.read(b, n);
        if (r <= 0) return false;
        if (out.write(b, r) != (size_t)r) return false;
        left -= (uint32_t)r;
    }

    uint32_t cd_off = 30u + nlen + usize;
    uint8_t cd[46]; memset(cd, 0, 46);
    cd[0] = 0x50; cd[1] = 0x4b; cd[2] = 0x01; cd[3] = 0x02;
    wr16(cd + 4, 20); wr16(cd + 6, 20);
    wr32(cd + 16, crc);
    wr32(cd + 20, usize);
    wr32(cd + 24, usize);
    wr16(cd + 28, nlen);
    // local header offset = 0 (cd+42 already zero)
    if (out.write(cd, 46) != 46) return false;
    if (out.write((const uint8_t*)inner, nlen) != nlen) return false;

    uint8_t eo[22]; memset(eo, 0, 22);
    eo[0] = 0x50; eo[1] = 0x4b; eo[2] = 0x05; eo[3] = 0x06;
    wr16(eo + 8, 1); wr16(eo + 10, 1);      // 1 entry
    wr32(eo + 12, 46u + nlen);              // central dir size
    wr32(eo + 16, cd_off);                  // central dir offset
    return out.write(eo, 22) == 22;
}

bool archive_compress(const char* src_dsk_path, const char* dst_path, int kind) {
    if (SD.exists(REPACK_TMP)) SD.remove(REPACK_TMP);
    File in = SD.open(src_dsk_path, FILE_READ);
    if (!in) return false;
    File out = SD.open(REPACK_TMP, FILE_WRITE);
    if (!out) { in.close(); return false; }

    bool ok = (kind == ARC_GZ)  ? compress_gz(in, out)
            : (kind == ARC_ZIP) ? compress_zip_stored(in, out)
            : false;

    out.flush(); out.close(); in.close();
    if (!ok) { SD.remove(REPACK_TMP); return false; }

    // swap the new archive in atomically-ish
    if (SD.exists(dst_path)) SD.remove(dst_path);
    if (!SD.rename(REPACK_TMP, dst_path)) { SD.remove(REPACK_TMP); return false; }
    return true;
}
