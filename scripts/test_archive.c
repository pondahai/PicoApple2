/* Host run-test for the uzlib driving logic in ../disk_archive.cpp.
 * Mirrors that file's functions exactly, with SD `File` replaced by stdio
 * FILE*. Purpose: execute the multi-member gzip loop, streaming inflate
 * (dict_ring), and per-chunk compressor reset on real data and cross-check
 * with the system gzip/unzip tools. The ARM firmware build cannot run, so
 * this is where the *logic* (not just syntax) gets verified.
 *
 * Build (from repo root, in a VS x64 dev shell):
 *   cl /nologo /O2 /I src\uzlib scripts\test_archive.c ^
 *      src\uzlib\tinflate.c src\uzlib\tinfgzip.c src\uzlib\defl_static.c ^
 *      src\uzlib\genlz77.c src\uzlib\adler32.c src\uzlib\crc32.c ^
 *      /Fe:scripts\test_archive.exe
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "uzlib.h"

#define DICT_SIZE   32768u
#define OUT_CHUNK    2048u
#define GZ_CHUNK    16384u
#define ZIP_INNER  "DISK.DSK"

#define TRACK 4096u
#define NTRACKS 35u
#define DSK_LEN (TRACK * NTRACKS)

static inline uint16_t rd16(const uint8_t* p){ return (uint16_t)(p[0] | (p[1]<<8)); }
static inline uint32_t rd32(const uint8_t* p){ return (uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24); }
static inline void wr16(uint8_t* p, uint16_t v){ p[0]=v&0xff; p[1]=(v>>8)&0xff; }
static inline void wr32(uint8_t* p, uint32_t v){ p[0]=v&0xff; p[1]=(v>>8)&0xff; p[2]=(v>>16)&0xff; p[3]=(v>>24)&0xff; }

/* ---- streaming source, mirror of ArcSrc/arc_src_cb ---- */
struct ArcSrc { FILE* f; uint32_t remaining; uint8_t buf[512]; };
static struct ArcSrc s_src;

static int arc_src_cb(struct uzlib_uncomp* d){
    if (s_src.remaining == 0) return -1;
    uint32_t want = s_src.remaining < sizeof(s_src.buf) ? s_src.remaining : sizeof(s_src.buf);
    size_t n = fread(s_src.buf, 1, want, s_src.f);
    if (n == 0) { s_src.remaining = 0; return -1; }
    s_src.remaining -= (uint32_t)n;
    d->source = s_src.buf + 1;
    d->source_limit = s_src.buf + n;
    return s_src.buf[0];
}

static long fsize(FILE* f){ long c=ftell(f); fseek(f,0,SEEK_END); long s=ftell(f); fseek(f,c,SEEK_SET); return s; }

/* ---- extract_gz (mirror) ---- */
static int extract_gz(FILE* out){
    uint8_t* dict=(uint8_t*)malloc(DICT_SIZE);
    uint8_t* outb=(uint8_t*)malloc(OUT_CHUNK);
    if(!dict||!outb){ free(dict); free(outb); return 0; }
    struct uzlib_uncomp d; memset(&d,0,sizeof d);
    d.source=NULL; d.source_limit=NULL; d.source_read_cb=arc_src_cb; d.eof=0;
    int ok=1, more=1;
    while(more && ok){
        uzlib_uncompress_init(&d, dict, DICT_SIZE);
        if(uzlib_gzip_parse_header(&d)!=TINF_OK){ ok=0; break; }
        for(;;){
            d.dest_start=d.dest=outb; d.dest_limit=outb+OUT_CHUNK;
            int r=uzlib_uncompress_chksum(&d);
            if(r<0){ ok=0; break; }
            uint32_t produced=(uint32_t)(d.dest-outb);
            if(produced && fwrite(outb,1,produced,out)!=produced){ ok=0; break; }
            if(r==TINF_DONE) break;
        }
        if(!ok) break;
        uint32_t leftover=(uint32_t)(d.source_limit-d.source)+s_src.remaining;
        more = leftover>=18;
    }
    free(dict); free(outb);
    return ok;
}

/* ---- zip_locate + extract_zip (mirror) ---- */
static int zip_locate(FILE* in, uint32_t* data_off, uint32_t* csize, uint16_t* method){
    long fs=fsize(in);
    if(fs<22) return 0;
    uint32_t tail = (uint32_t)(fs<512?fs:512);
    uint8_t buf[512];
    fseek(in, fs-tail, SEEK_SET);
    if(fread(buf,1,tail,in)!=tail) return 0;
    int eocd=-1;
    for(int i=(int)tail-22;i>=0;i--){ if(buf[i]==0x50&&buf[i+1]==0x4b&&buf[i+2]==0x05&&buf[i+3]==0x06){eocd=i;break;} }
    if(eocd<0) return 0;
    uint32_t cd_off=rd32(buf+eocd+16);
    uint8_t cd[46]; fseek(in,cd_off,SEEK_SET);
    if(fread(cd,1,46,in)!=46) return 0;
    if(!(cd[0]==0x50&&cd[1]==0x4b&&cd[2]==0x01&&cd[3]==0x02)) return 0;
    *method=rd16(cd+10); *csize=rd32(cd+20);
    uint32_t lho=rd32(cd+42);
    uint8_t lh[30]; fseek(in,lho,SEEK_SET);
    if(fread(lh,1,30,in)!=30) return 0;
    if(!(lh[0]==0x50&&lh[1]==0x4b&&lh[2]==0x03&&lh[3]==0x04)) return 0;
    *data_off = lho+30+rd16(lh+26)+rd16(lh+28);
    return 1;
}

static int extract_zip(FILE* in, FILE* out){
    uint32_t data_off,csize; uint16_t method;
    if(!zip_locate(in,&data_off,&csize,&method)) return 0;
    fseek(in,data_off,SEEK_SET);
    if(method==0){
        uint8_t b[512]; uint32_t left=csize;
        while(left){ uint32_t n=left<sizeof(b)?left:sizeof(b); size_t r=fread(b,1,n,in); if(r==0)return 0; if(fwrite(b,1,r,out)!=r)return 0; left-=(uint32_t)r; }
        return 1;
    }
    if(method!=8) return 0;
    uint8_t* dict=(uint8_t*)malloc(DICT_SIZE);
    uint8_t* outb=(uint8_t*)malloc(OUT_CHUNK);
    if(!dict||!outb){ free(dict); free(outb); return 0; }
    s_src.remaining=csize;
    struct uzlib_uncomp d; memset(&d,0,sizeof d);
    uzlib_uncompress_init(&d,dict,DICT_SIZE);
    d.source=NULL; d.source_limit=NULL; d.source_read_cb=arc_src_cb; d.eof=0;
    d.checksum_type=TINF_CHKSUM_NONE;
    int ok=1;
    for(;;){
        d.dest_start=d.dest=outb; d.dest_limit=outb+OUT_CHUNK;
        int r=uzlib_uncompress(&d);
        if(r<0){ ok=0; break; }
        uint32_t produced=(uint32_t)(d.dest-outb);
        if(produced && fwrite(outb,1,produced,out)!=produced){ ok=0; break; }
        if(r==TINF_DONE) break;
    }
    free(dict); free(outb);
    return ok;
}

/* ---- compress_gz (mirror) ---- */
static int compress_gz(FILE* in, FILE* out){
    struct uzlib_comp c; memset(&c,0,sizeof c);
    c.dict_size=GZ_CHUNK; c.hash_bits=12;
    unsigned hash_size=sizeof(uzlib_hash_entry_t)*(1u<<c.hash_bits);
    c.hash_table=(uzlib_hash_entry_t*)malloc(hash_size);
    uint8_t* src=(uint8_t*)malloc(GZ_CHUNK);
    c.outsize=GZ_CHUNK+GZ_CHUNK/2+128;
    c.outbuf=(unsigned char*)malloc(c.outsize);
    if(!c.hash_table||!src||!c.outbuf){ free(c.hash_table);free(src);free(c.outbuf); return 0; }
    static const uint8_t hdr[10]={0x1f,0x8b,0x08,0x00,0,0,0,0,0,0xff};
    int ok=1;
    for(;;){
        size_t n=fread(src,1,GZ_CHUNK,in);
        if(n==0) break;
        memset(c.hash_table,0,hash_size);
        c.outlen=0; c.outbits=0; c.noutbits=0; c.comp_disabled=0;
        zlib_start_block(&c);
        uzlib_compress(&c, src, (unsigned)n);
        zlib_finish_block(&c);
        uint8_t tr[8];
        wr32(tr, ~uzlib_crc32(src,(unsigned)n,~0u));
        wr32(tr+4,(uint32_t)n);
        if(fwrite(hdr,1,10,out)!=10 || fwrite(c.outbuf,1,c.outlen,out)!=(size_t)c.outlen || fwrite(tr,1,8,out)!=8){ ok=0; break; }
    }
    free(c.hash_table); free(src); free(c.outbuf);
    return ok;
}

/* ---- compress_zip_stored (mirror) ---- */
static int compress_zip_stored(FILE* in, FILE* out){
    long us=fsize(in); uint32_t usize=(uint32_t)us;
    uint8_t b[512];
    uint32_t crc=~0u, left=usize;
    fseek(in,0,SEEK_SET);
    while(left){ uint32_t n=left<sizeof(b)?left:sizeof(b); size_t r=fread(b,1,n,in); if(r==0)return 0; crc=uzlib_crc32(b,(unsigned)r,crc); left-=(uint32_t)r; }
    crc=~crc;
    const char* inner=ZIP_INNER; uint16_t nlen=(uint16_t)strlen(inner);
    uint8_t lh[30]; memset(lh,0,30);
    lh[0]=0x50;lh[1]=0x4b;lh[2]=0x03;lh[3]=0x04; wr16(lh+4,20);
    wr32(lh+14,crc); wr32(lh+18,usize); wr32(lh+22,usize); wr16(lh+26,nlen);
    if(fwrite(lh,1,30,out)!=30) return 0;
    if(fwrite(inner,1,nlen,out)!=nlen) return 0;
    fseek(in,0,SEEK_SET); left=usize;
    while(left){ uint32_t n=left<sizeof(b)?left:sizeof(b); size_t r=fread(b,1,n,in); if(r==0)return 0; if(fwrite(b,1,r,out)!=r)return 0; left-=(uint32_t)r; }
    uint32_t cd_off=30u+nlen+usize;
    uint8_t cd[46]; memset(cd,0,46);
    cd[0]=0x50;cd[1]=0x4b;cd[2]=0x01;cd[3]=0x02; wr16(cd+4,20); wr16(cd+6,20);
    wr32(cd+16,crc); wr32(cd+20,usize); wr32(cd+24,usize); wr16(cd+28,nlen);
    if(fwrite(cd,1,46,out)!=46) return 0;
    if(fwrite(inner,1,nlen,out)!=nlen) return 0;
    uint8_t eo[22]; memset(eo,0,22);
    eo[0]=0x50;eo[1]=0x4b;eo[2]=0x05;eo[3]=0x06; wr16(eo+8,1); wr16(eo+10,1);
    wr32(eo+12,46u+nlen); wr32(eo+16,cd_off);
    return fwrite(eo,1,22,out)==22;
}

/* ---- test scaffolding ---- */
static void make_fake_dsk(const char* path){
    /* same flavour as archive_proto.py: compressible structure + pseudo-random */
    FILE* f=fopen(path,"wb");
    unsigned long seed=1234;
    for(unsigned t=0;t<NTRACKS;t++){
        uint8_t trk[TRACK];
        if(t<3){
            for(unsigned i=0;i<1024;i++) trk[i]=0xFF;
            for(unsigned i=0;i<1024;i++) trk[1024+i]=0x00;
            for(unsigned i=0;i<2048;i++) trk[2048+i]=(uint8_t)(i&0xFF);
        } else {
            unsigned i=0;
            while(i<TRACK){
                seed=seed*1103515245u+12345u; unsigned r=(seed>>16)&0x7fff;
                if((r&1)==0){
                    unsigned runlen=4+(r%61); uint8_t v=(uint8_t)((r>>3)&0xff);
                    for(unsigned j=0;j<runlen && i<TRACK;j++) trk[i++]=v;
                } else {
                    for(unsigned j=0;j<32 && i<TRACK;j++){ seed=seed*1103515245u+12345u; trk[i++]=(uint8_t)((seed>>16)&0xff); }
                }
            }
        }
        fwrite(trk,1,TRACK,f);
    }
    fclose(f);
}

static int files_equal(const char* a, const char* b){
    FILE* fa=fopen(a,"rb"); FILE* fb=fopen(b,"rb");
    if(!fa||!fb){ if(fa)fclose(fa); if(fb)fclose(fb); return 0; }
    int eq=1; for(;;){ int ca=fgetc(fa), cb=fgetc(fb); if(ca!=cb){eq=0;break;} if(ca==EOF) break; }
    fclose(fa); fclose(fb); return eq;
}

int main(void){
    int fail=0;
    make_fake_dsk("t_orig.dsk");
    long orig=0; { FILE*f=fopen("t_orig.dsk","rb"); orig=fsize(f); fclose(f); }
    printf("fake dsk: %ld bytes\n", orig);

    /* gz: compress -> extract -> compare */
    { FILE* in=fopen("t_orig.dsk","rb"); FILE* out=fopen("t.gz","wb");
      int ok=compress_gz(in,out); fclose(in); fclose(out);
      long gz=0; { FILE*f=fopen("t.gz","rb"); gz=fsize(f); fclose(f); }
      printf("compress_gz: %s, %ld bytes (%ld%%)\n", ok?"ok":"FAIL", gz, orig?gz*100/orig:0);
      if(!ok) fail=1;
      FILE* gin=fopen("t.gz","rb"); FILE* gout=fopen("t_gz_out.dsk","wb");
      s_src.f=gin; s_src.remaining=(uint32_t)gz; fseek(gin,0,SEEK_SET);
      ok=extract_gz(gout); fclose(gin); fclose(gout);
      printf("extract_gz round-trip: %s\n", (ok && files_equal("t_orig.dsk","t_gz_out.dsk"))?"OK":"FAIL");
      if(!(ok && files_equal("t_orig.dsk","t_gz_out.dsk"))) fail=1;
    }
    /* gz validated by system gzip */
    if(system("gzip -t t.gz")==0) printf("system gzip -t: OK\n"); else { printf("system gzip -t: FAIL\n"); fail=1; }

    /* read a gz made by the system gzip (32K window, single member) */
    system("gzip -c -k -f t_orig.dsk > t_ext.gz 2>/dev/null || (cp t_orig.dsk t_tmp && gzip -f t_tmp && mv t_tmp.gz t_ext.gz)");
    { FILE* gin=fopen("t_ext.gz","rb"); if(gin){ long gz=fsize(gin); FILE* gout=fopen("t_ext_out.dsk","wb");
      s_src.f=gin; s_src.remaining=(uint32_t)gz; fseek(gin,0,SEEK_SET);
      int ok=extract_gz(gout); fclose(gin); fclose(gout);
      printf("read system gzip: %s\n", (ok&&files_equal("t_orig.dsk","t_ext_out.dsk"))?"OK":"FAIL");
      if(!(ok&&files_equal("t_orig.dsk","t_ext_out.dsk"))) fail=1;
    } else printf("read system gzip: SKIP (no t_ext.gz)\n"); }

    /* zip stored: compress -> extract -> compare + system unzip */
    { FILE* in=fopen("t_orig.dsk","rb"); FILE* out=fopen("t.zip","wb");
      int ok=compress_zip_stored(in,out); fclose(in); fclose(out);
      printf("compress_zip_stored: %s\n", ok?"ok":"FAIL"); if(!ok) fail=1;
      FILE* zin=fopen("t.zip","rb"); FILE* zout=fopen("t_zip_out.dsk","wb");
      ok=extract_zip(zin,zout); fclose(zin); fclose(zout);
      printf("extract_zip(stored) round-trip: %s\n", (ok&&files_equal("t_orig.dsk","t_zip_out.dsk"))?"OK":"FAIL");
      if(!(ok&&files_equal("t_orig.dsk","t_zip_out.dsk"))) fail=1;
    }
    if(system("unzip -tq t.zip")==0) printf("system unzip -t: OK\n"); else { printf("system unzip -t: FAIL\n"); fail=1; }

    /* read a deflated zip (method 8) made by python zipfile */
    system("python -c \"import zipfile;zipfile.ZipFile('t_def.zip','w',zipfile.ZIP_DEFLATED).write('t_orig.dsk','DISK.DSK')\"");
    { FILE* zin=fopen("t_def.zip","rb"); if(zin){ FILE* zout=fopen("t_def_out.dsk","wb");
      int ok=extract_zip(zin,zout); fclose(zin); fclose(zout);
      printf("read deflated zip(method8): %s\n", (ok&&files_equal("t_orig.dsk","t_def_out.dsk"))?"OK":"FAIL");
      if(!(ok&&files_equal("t_orig.dsk","t_def_out.dsk"))) fail=1;
    } else printf("read deflated zip: SKIP\n"); }

    printf(fail? "\nSOME CHECKS FAILED\n" : "\nALL CHECKS PASSED\n");
    return fail;
}
