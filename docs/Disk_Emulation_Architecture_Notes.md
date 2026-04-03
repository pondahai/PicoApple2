# PicoApple2: Disk Emulation Architecture & Timing Notes

*This document serves as a cognitive anchor for future AI development sessions, summarizing critical insights regarding dual-core synchronization, 6502 cycle accuracy, and the limitations of `.DSK` image handling.*

## 1. The "Fast-Forward" Misconception vs. Cycle Accuracy

**Initial Theory:** We attempted to accelerate disk loading by disabling the 1MHz real-time throttle (`delayMicroseconds`) on Core 0 while the virtual disk motor was running.
**The Flaw:** We assumed that running the emulator at 250MHz would desynchronize the CPU from the disk data stream, causing read failures.
**The Reality:** The 6502 emulator operates in a closed, logical universe. The virtual disk's bit stream advances strictly based on *executed CPU cycles* (`cycles_accumulator += cycles`), not real-world time. Therefore, unthrottling Core 0 simply "fast-forwards" the entire universe synchronously. **Logical read failures are NOT caused by host execution speed.**

## 2. Why Unthrottling Actually Crashed the System (Host-Level Failures)

If logical timing is preserved during unthrottling, why did the emulator crash (Watchdog resets, `CP=1` or `CP=6` hangs)?

1.  **Dual-Core Starvation:** An unthrottled Core 0 executes instructions continuously, monopolizing the shared memory bus and the `res_lock` spinlock. Core 1 (responsible for TFT rendering and watchdog heartbeats) is starved of resources, leading to hardware-level deadlocks.
2.  **I/O Latency Bubbles:** Physical SD card reads (`loadSingleTrack`) take real-world milliseconds. When the CPU is running at extreme speeds, a 5ms SD read translates to a massive, unnatural halt in the emulator's internal timeline, which can break timing-sensitive 6502 routines.
3.  **The Underflow Bug:** Transitioning from an unthrottled state (motor on) back to a throttled state (motor off) caused the `expected` cycle time calculation to exceed the `actual` real-world elapsed time. Subtracting these caused an unsigned integer underflow, resulting in an infinite `delayMicroseconds` lockup.

*Resolution:* The emulator MUST maintain a strict 1.023MHz real-time throttle at all times to ensure host-system stability and equitable dual-core resource sharing.

## 3. The `.DSK` Format Dilemma: Why standard DOS boots, but "Goonies" hangs

If the system is perfectly cycle-accurate at 1.023MHz, why do some `.DSK` files (like standard DOS 3.3) boot perfectly, while cracked games (like *Goonies*) hang infinitely?

**The Nature of `.DSK`:**
`.DSK` files contain ONLY sector payload data (140KB). They strip away all physical track structures present on a real floppy disk, including Sync Bytes (`FF`), Address Headers (`D5 AA 96`), Data Headers, and Gap Bytes.

**The "Nibblization" Hack:**
Because the 6502 CPU expects to read raw physical signals from `$C08C`, our emulator (`apple2_core/src/nibble.rs`) must artificially reconstruct ("nibblize") the 256-byte pure data back into a rigid, standard DOS 3.3 physical track format (6656 bytes).

**The Root Cause of Failure (Fast Loaders vs. RWTS):**
*   **Standard DOS 3.3 (RWTS):** Uses forgiving, polling-based loops to read `$C08C`. It tolerates slight phase jitter in our artificially reconstructed nibble stream.
*   **Cracked Games (Fast Loaders):** Even when copy protection is removed, hackers retain **Fast Loaders** to speed up boot times. Fast Loaders use heavily optimized, cycle-counted *unrolled loops*. They expect the Disk II hardware shift register to present valid data at exact cycle intervals.
*   **Our Limitation:** Our current disk implementation (`disk2.rs`) uses a macroscopic, byte-level approximation (`if cycles_accumulator >= 32 { spit_out_byte }`). It lacks true bit-level shift register emulation. This introduces microsecond-level phase jitter.
*   **The Result:** When a Fast Loader encounters our jittery virtual disk, it reads a misaligned byte (e.g., catching data mid-shift), fails the sector checksum, and infinitely retries the read. **The 6502 hasn't crashed; it is stuck in a perfectly valid, infinite retry loop.**

## 4. Future Development Roadmap

To achieve high compatibility and support Fast Loaders/Custom DOS, future development must focus on:

1.  **Bit-Level Shift Register Emulation:** Rewrite `disk2.rs` to simulate the Disk II hardware at the bit level, accurately mirroring the Sequencer ROM states and "latch hold" behaviors, rather than relying on a simple 32-cycle byte trigger.
2.  **Flux-Level Image Support:** Implement support for `.NIB` or `.WOZ` disk images. These formats preserve the exact physical bitstream of the original disks, allowing us to bypass the flawed, artificial "nibblization" process entirely. This is the ultimate solution for perfect hardware emulation.