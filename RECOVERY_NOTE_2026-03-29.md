# PicoApple2 Recovery Note - 2026-03-29

## Reverted Changes Summary (For Future Reference)

### 1. Stability Fixes (PicoApple2.ino)
- Moved `loadSingleTrack`, `flushDirtyTrack`, and `Serial.print` OUTSIDE the simulation lock.
- Introduced `g_emu_paused` to coordinate IO without blocking Core 1.
- Reduced SD SPI frequency to 4MHz and added `SD.end()` retry logic.

### 2. Disk Controller Improvements (disk2.rs)
- **Linear Motor**: Restricted head movement to ±1 half-track per change.
- **Windowed MSB**: 30-cycle validity for data latch to support tight loops.
- **Reset Fix**: `current_qtr_track = 0` on reset.

### 3. Core Engine (lib.rs / nibble.rs)
- Added `apple2_clear_disk_reload_flag()` to stop infinite loading loops.
- Modified `apple2_load_track` to prevent redundant `byte_index` resets.
- Optimized `denibblize_track` performance and safety.

### 4. Diagnostic Features
- Rust Panic handler with Serial reporting.
- PC-based heartbeat logic for tracking the Goonies loader.

---
*Action: Reverted to last known working commit. Use these notes to re-implement features surgically.*
