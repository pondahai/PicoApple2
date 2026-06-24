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
