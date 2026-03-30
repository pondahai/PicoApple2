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
