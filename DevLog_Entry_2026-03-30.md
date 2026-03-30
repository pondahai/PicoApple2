# 🍎 PicoApple2 DevLog - 2026-03-30

## 🛡️ Copy Protection Analysis: The Goonies (Datasoft, 1985)

### 1. Technical Context: "Weak Bits" Protection
**Observation**: Standard Apple II `.dsk` (Sector-based) images of *The Goonies* often fail to boot, hanging or returning to a title screen loop.
**Root Cause**: Datasoft utilized a "Weak Bits" protection scheme. 
- **Mechanism**: The protection code reads specific sectors (notably `$F7` and `$F8`, or Absolute Sector 720/721) **three times** into separate memory buffers (e.g., `$A000`, `$A100`, `$A200`).
- **Check logic**: The CPU then compares these buffers. On an original physical floppy, "Weak Bits" have unstable magnetic flux, causing a few bits to flip randomly between reads. The code expects at least one byte to be different across the three reads to confirm it's a "genuine" physical disk.
- **The DSK Limitation**: Since `.dsk` files only store static, error-corrected data bytes, every read returns identical results. The protection check fails because the data is "too perfect."

### 2. Evaluated Solutions for PicoApple2

| Approach | Feasibility on RP2040 | Conclusion |
| :--- | :--- | :--- |
| **`.woz` Format Support** | **Medium/Hard**: Requires high-precision bitstream simulation and significant RAM/CPU for state machine. | Future goal for "Accuracy Mode." |
| **Emulator HLE (Fake Noise)** | **Easy**: Inject random bit-flips when reading specific tracks/sectors in `disk2.rs`. | **Rejected**: Breaks "Universal Emulator" principle. Too many edge cases for different games. |
| **Cracked DSK (Logic Patch)** | **N/A (External)**: The game logic itself is modified to bypass the check (e.g., changing `CPX #$03` to `CPX #$01`). | **Recommended**: Best balance for performance and compatibility. |

### 3. Final Conclusion & Strategy
- **Universal Integrity**: `PicoApple2` (Rust Core) must remain "Honest." It should return exactly what is in the `.dsk` file without injecting synthetic noise. This ensures maximum compatibility across the entire Apple II library.
- **Cracked DSK Standard**: For protected games like *The Goonies*, users should utilize "Cracked" `.dsk` versions. These versions have had their protection routines patched at the 6502 logic level, making them fully compatible with standard sector-based emulation.
- **Verification**: If a `.dsk` fails to boot, it is likely an uncracked image. The user should be directed to use a `.woz` version (once supported) or a verified cracked `.dsk`.

---
*Note: This analysis serves as a foundational reference for why physical-layer protection requires either bitstream-level simulation (.woz) or logic-level cracking to function in a sector-based emulator.*

## 🐛 Bug Fix: Core Hangs & Serial Terminal Unresponsiveness

### 1. Diagnosis
Users reported that the emulator would sporadically freeze entirely, taking the USB Serial terminal down with it. A full architectural review revealed two lethal issues in the dual-core design:

*   **Fatal SD Card Race Condition**: In the RP2040 Arduino environment, the `SD.h` library (wrapping FatFS) is **not thread-safe**. 
    *   **Core 0** routinely hits the SD card during emulation to load/flush virtual floppy tracks (`flushDirtyTrack`, `loadSingleTrack`).
    *   **Core 1** was allowed to independently call SD functions when the user invoked the disk menu (e.g., `scanDiskFiles()`, `SD.open()`).
    *   **Result**: When both cores hit the SPI1 bus simultaneously, the hardware driver locked up permanently, halting both cores.

*   **Spinlock Deadlock via Rust Panic**: The `apple2_tick()` emulation loop executes inside a hardware spinlock (`res_lock`).
    *   If the Rust core experienced a `panic!` (e.g., due to an array bounds violation), the `#![no_std]` panic handler was set to enter an infinite loop (`loop {}`).
    *   **Result**: Core 0 halted *while still holding the lock*. Core 1 would then attempt to acquire the same lock to handle input or reset the system, causing a complete dual-core deadlock and severing USB Serial communication (handled by Core 0).

### 2. Solutions Implemented
We re-architected the cross-core communication for I/O and improved panic handling:

1.  **I/O Command Queue (Thread Safety)**
    *   Introduced a set of volatile flags (`req_scan_disks`, `req_load_disk_idx`, `req_reload_track0`) to act as a lock-free message passing system.
    *   Core 1 now *never* calls SD library functions directly. Instead, it asserts a request flag and waits.
    *   Core 0 reads these flags outside of its emulation tick and safely executes the SD operations. All SPI1 traffic is now perfectly serialized on Core 0.

2.  **Cortex-M0+ Hard Reset on Panic**
    *   Modified the Rust `panic_handler` in `apple2_core/src/lib.rs`.
    *   Instead of `loop {}`, it now directly writes to the ARM Application Interrupt and Reset Control Register (AIRCR) to trigger a `SYSRESETREQ` (System Reset Request).
    *   `core::ptr::write_volatile(0xE000ED0C as *mut u32, 0x05FA0004);`
    *   This ensures that any fatal error forcibly reboots the Pico, automatically restoring the USB connection rather than silently bricking the device.
