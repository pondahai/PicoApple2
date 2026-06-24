#!/usr/bin/env python3
"""
Prototype + validation of the gz/zip framing that disk_archive.cpp will emit
on the RP2040. The goal is to prove the *container framing* (the bytes I am
inventing) is correct and round-trips through the real `gzip`/`unzip` tools,
BEFORE porting it to C with uzlib as the deflate/inflate primitive.

uzlib's zlib_finish_block() emits a single RAW DEFLATE stream (BFINAL=1,
static Huffman). In Python that is zlib.compressobj(wbits=-15). So everywhere
the C code will call uzlib, this prototype uses raw deflate to mirror it.
"""
import os, struct, zlib, subprocess, tempfile, sys

TRACK = 4096
NTRACKS = 35
DSK_LEN = TRACK * NTRACKS  # 143360
GZ_CHUNK = 32768           # multi-member gzip chunk size (== uzlib dict_size)


def make_fake_dsk():
    """Realistic-ish image: lots of 0xFF/0x00 runs + some pseudo-random data,
    so compression actually does something (like a real DOS 3.3 disk)."""
    import random
    random.seed(1234)
    b = bytearray()
    for t in range(NTRACKS):
        if t < 3:
            # DOS / catalog area: structured, very compressible
            b += bytes(([0xFF] * 1024) + ([0x00] * 1024) + list(range(256)) * 8)
        else:
            # mix of runs and random
            chunk = bytearray()
            while len(chunk) < TRACK:
                if random.random() < 0.5:
                    chunk += bytes([random.randint(0, 255)] * random.randint(4, 64))
                else:
                    chunk += bytes(random.randint(0, 255) for _ in range(32))
            b += chunk[:TRACK]
    return bytes(b[:DSK_LEN])


# ---------- gz write-back: multi-member gzip ----------
def gz_pack(data: bytes) -> bytes:
    """Emit a multi-member gzip: one full gzip member per GZ_CHUNK slice.
    Concatenated gzip members are standard and any decompressor joins them."""
    out = bytearray()
    for off in range(0, len(data), GZ_CHUNK):
        chunk = data[off:off + GZ_CHUNK]
        # raw deflate (mirrors uzlib zlib_start_block/compress/finish_block)
        co = zlib.compressobj(6, zlib.DEFLATED, -15)
        raw = co.compress(chunk) + co.flush()
        # gzip member header: magic, CM=deflate, FLG=0, MTIME=0, XFL=0, OS=255
        out += b"\x1f\x8b\x08\x00" + b"\x00\x00\x00\x00" + b"\x00\xff"
        out += raw
        out += struct.pack("<I", zlib.crc32(chunk) & 0xffffffff)
        out += struct.pack("<I", len(chunk) & 0xffffffff)
    return bytes(out)


# ---------- gz read: streaming inflate ----------
def gz_unpack(blob: bytes) -> bytes:
    """Decompress (possibly multi-member) gzip. Mirrors uzlib_gzip_parse_header
    + uzlib_uncompress loop, repeated for each member."""
    d = zlib.decompressobj(15 + 16)  # gzip auto, handles multi-member via unused_data
    out = bytearray()
    rest = blob
    while rest:
        out += d.decompress(rest)
        out += d.flush()
        rest = d.unused_data
        if rest:
            d = zlib.decompressobj(15 + 16)
    return bytes(out)


# ---------- zip stored write-back ----------
def zip_pack_stored(name: str, data: bytes) -> bytes:
    crc = zlib.crc32(data) & 0xffffffff
    n = name.encode()
    # local file header (method 0 = stored)
    lfh = b"PK\x03\x04" + struct.pack("<HHHHHIIIHH",
        20, 0, 0, 0, 0, crc, len(data), len(data), len(n), 0) + n
    local_off = 0
    body = lfh + data
    # central directory
    cdh = b"PK\x01\x02" + struct.pack("<HHHHHHIIIHHHHHII",
        20, 20, 0, 0, 0, 0, crc, len(data), len(data),
        len(n), 0, 0, 0, 0, 0, local_off) + n
    cd_off = len(body)
    eocd = b"PK\x05\x06" + struct.pack("<HHHHIIH",
        0, 0, 1, 1, len(cdh), cd_off, 0)
    return body + cdh + eocd


# ---------- zip read: find first entry, inflate or copy ----------
def zip_unpack_first(blob: bytes) -> bytes:
    """Parse the FIRST local file header the way the C code will: read method,
    sizes, name/extra lengths, then inflate (8) or copy (0)."""
    assert blob[:4] == b"PK\x03\x04", "not a zip local header"
    (ver, flags, method, mtime, mdate, crc, csize, usize,
     nlen, elen) = struct.unpack("<HHHHHIIIHH", blob[4:30])
    data_off = 30 + nlen + elen
    comp = blob[data_off:data_off + csize]
    if method == 0:
        return comp[:usize]
    elif method == 8:
        return zlib.decompress(comp, -15)  # raw inflate
    else:
        raise ValueError(f"unsupported method {method}")


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, **kw)


def main():
    dsk = make_fake_dsk()
    print(f"fake dsk: {len(dsk)} bytes")
    tmp = tempfile.mkdtemp(prefix="arcproto_")
    ok = True

    # --- gz round-trip (internal) ---
    packed = gz_pack(dsk)
    print(f"gz multi-member: {len(packed)} bytes "
          f"({100*len(packed)//len(dsk)}% of original)")
    rt = gz_unpack(packed)
    assert rt == dsk, "gz internal round-trip FAILED"
    print("gz internal round-trip: OK")

    # --- gz validated by the real `gzip -d` ---
    gzpath = os.path.join(tmp, "test.dsk.gz")
    with open(gzpath, "wb") as f:
        f.write(packed)
    r = run(["gzip", "-dc", gzpath])
    if r.returncode != 0 or r.stdout != dsk:
        ok = False
        print(f"gzip -d validation: FAILED rc={r.returncode} err={r.stderr[:200]}")
    else:
        print("gzip -d external validation: OK")

    # --- read a gz produced by the real `gzip` (32K window) ---
    plain = os.path.join(tmp, "plain.dsk")
    with open(plain, "wb") as f:
        f.write(dsk)
    run(["gzip", "-k", "-f", plain])
    with open(plain + ".gz", "rb") as f:
        ext_gz = f.read()
    assert gz_unpack(ext_gz) == dsk, "reading real gzip FAILED"
    print("read real gzip output: OK")

    # --- zip stored round-trip + external `unzip` ---
    zblob = zip_pack_stored("DISK.DSK", dsk)
    assert zip_unpack_first(zblob) == dsk, "zip stored internal RT FAILED"
    zpath = os.path.join(tmp, "stored.zip")
    with open(zpath, "wb") as f:
        f.write(zblob)
    r = run(["unzip", "-p", zpath, "DISK.DSK"])
    if r.returncode != 0 or r.stdout != dsk:
        ok = False
        print(f"unzip stored validation: FAILED rc={r.returncode} err={r.stderr[:200]}")
    else:
        print("unzip stored external validation: OK")

    # --- read a zip produced by the real `zip` (deflate method 8) ---
    # build with python zipfile (uses deflate)
    import zipfile
    zp2 = os.path.join(tmp, "deflated.zip")
    with zipfile.ZipFile(zp2, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("DISK.DSK", dsk)
    with open(zp2, "rb") as f:
        ext_zip = f.read()
    assert zip_unpack_first(ext_zip) == dsk, "reading real deflated zip FAILED"
    print("read real deflated zip: OK")

    print("\nALL CHECKS PASSED" if ok else "\nSOME CHECKS FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
