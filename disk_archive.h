// disk_archive.h - gz / zip read + write-back for Apple II .dsk images on SD.
//
// Strategy (see DevLog 2026-06-24): a compressed disk is decompressed to a
// plain work .dsk on the SD card; the existing per-track engine runs against
// that work file unchanged; on disk-swap/eject the work file is repacked back
// into the original archive. This keeps the bit-level track engine untouched.
//
// Decompression streams through a 32 KB uzlib dictionary window, so the full
// 140 KB image never has to live in RAM. gz write-back uses multi-member gzip
// (one member per 16 KB chunk) so the in-RAM compressor only ever sees a 16 KB
// slice. zip write-back is "stored" (method 0) for now - real-compressed zip
// would need a streaming deflate the RP2040 can't afford yet.
#pragma once
#include <stdint.h>

// Archive kind by file extension (case-insensitive).
enum ArchiveKind { ARC_RAW = 0, ARC_GZ = 1, ARC_ZIP = 2, ARC_UNKNOWN = -1 };

// Classify a filename ("/GAME.DSK.GZ" -> ARC_GZ, "/X.ZIP" -> ARC_ZIP,
// "/Y.DSK" -> ARC_RAW, otherwise ARC_UNKNOWN).
int archive_kind(const char* name);

// Decompress archive at src_path (kind ARC_GZ/ARC_ZIP) into a plain .dsk at
// dst_path (overwritten). Returns true on success; on failure dst is removed.
bool archive_extract(const char* src_path, const char* dst_path, int kind);

// Repack the plain .dsk at src_dsk_path into an archive at dst_path
// (kind ARC_GZ multi-member gzip, or ARC_ZIP stored). Writes to a temp file
// first and renames over dst, so a failure/power-loss leaves the old archive
// intact. Returns true on success.
bool archive_compress(const char* src_dsk_path, const char* dst_path, int kind);

// --- 分片回壓 ---------------------------------------------------------------
// archive_compress() 會一路壓完才返回（140KB 約 400ms）。跑在 Core 1 上時，那段
// 期間畫面渲染完全停擺 -> 停格約 25 個影格。以下 API 把同一份工作拆成許多小步，
// 呼叫端可以在兩步之間回去跑渲染。
//
// 用法：begin() 成功後反覆呼叫 step()，直到它回傳 0（完成，已 rename 覆蓋 dst）
// 或 -1（失敗，暫存檔已清掉、dst 原檔完好）。中途要放棄就呼叫 abort()。
// 同一時間只能有一份分片工作在進行（內部是單一靜態狀態）。
bool archive_compress_begin(const char* src_dsk_path, const char* dst_path, int kind);
int  archive_compress_step(void);   // 1 = 還有工作, 0 = 完成, -1 = 失敗
void archive_compress_abort(void);  // 放棄並清理；未開始時呼叫是 no-op
