# 🍎 PicoApple2 DevLog - 2026-03-29

## 🛡️ SD Card Stability & Write Protection

### 1. Problem: SD Write Wear-out
**Observation**: Frequent 4KB track updates (especially during disk formatting or ProDOS operations) combined with `close()` / `open()` cycles led to SD card hardware failure (corruption of FAT tables).
**Fix**:
- **Motor-Stop Trigger**: Implemented a delayed write-back strategy. The system now waits for the Disk II motor to stop (`motor_on` false) before flushing the dirty track to the SD card.
- **Persistence**: Replaced the `close/open` cycle in `flushDirtyTrack()` with a simple `flush()`. This keeps the file handle open and reduces metadata updates on the SD card's physical sectors.
- **Safety Sync**: Forced flushes are still performed during track seeks and disk swaps to ensure no data is lost during active operations.

## ⌨️ Input Latency & Terminal Performance

### 1. Refactored Serial Protocol Parser
**Old Logic**: Used a blocking `while` loop with a `micros() + 1000` timeout to wait for 4-byte protocol packets (0x02 header). This caused 1ms of simulation jitter per keypress.
**New Logic**: Implemented a **Non-blocking State Machine** (`proto_state`) for the custom 0x02 protocol. Bytes are handled as they arrive without pausing the emulator core.

### 2. Buffer Capacity
- Increased `KEY_FIFO_SIZE` from **32 to 128**.
- This enables smooth "copy-paste" of BASIC code snippets from the host terminal without dropping characters.

### 3. Optimization
- Replaced `String` object manipulation in ANSI escape sequence parsing (F1-F3 keys) with static `strcmp` calls to avoid heap fragmentation and CPU overhead.

## 📺 Rendering Efficiency (Core 1)
- Verified the **DMA Ping-pong Buffering** implementation.
- Core 1 handles scanline conversion (Text/Hires/Lores) while the SPI controller pushes the previous line via DMA at 30MHz.
- Result: Stable ~25 FPS without tearing, even during heavy disk I/O.
