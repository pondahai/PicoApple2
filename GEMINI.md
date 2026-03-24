# 🍎 PicoApple2 - Project Context

This project is a high-performance Apple II emulator designed for the Raspberry Pi Pico (RP2040). It leverages a dual-core architecture and a hybrid Rust/C++ codebase to achieve full-speed emulation with minimal RAM usage.

## 🏗️ Architecture Overview

The project is split into two main layers:

1.  **Emulator Core (`apple2_core/`):**
    *   **Language:** Rust (`thumbv6m-none-eabi` target).
    *   **Responsibility:** Implements the 6502 CPU, memory mapping, Disk II controller logic, and peripheral state.
    *   **Key Feature:** "Zero-buffer JIT Rendering" — instead of maintaining a full framebuffer, the core provides direct access to Apple II RAM, which the frontend renders on-the-fly.

2.  **Frontend & Hardware I/O (`pico_apple2_emulator.ino`):**
    *   **Language:** C++ (Arduino framework with Pico SDK).
    *   **Responsibility:** Handles dual-core scheduling, ILI9341 display driver (SPI), SD card filesystem (FatFS/SD.h), keyboard matrix scanning, and 1-bit speaker output.
    *   **Core Distribution:**
        *   **Core 0:** Runs the main emulation loop (`apple2_tick`), handles input, and manages SD card I/O.
        *   **Core 1:** Dedicated to JIT video rendering, pulling data from the Rust core and pushing pixels to the ILI9341 display.

## 🛠️ Building and Running

### Prerequisites
*   **Rust:** `rustup target add thumbv6m-none-eabi`
*   **Arduino CLI:** Included in the root (`arduino-cli.exe`).
*   **Pico Core:** `rp2040:rp2040` (v5.5.1+) installed in Arduino.
*   **FQBN:** `rp2040:rp2040:rpipico`

### Build Workflow

1.  **Compile Rust Core:**
    ```bash
    cd apple2_core
    cargo build --target thumbv6m-none-eabi --release
    ```
2.  **Sync Static Library:**
    Copy `apple2_core/target/thumbv6m-none-eabi/release/libapple2_core.a` to the project's `src/` directory.
3.  **Compile Arduino Sketch:**
    Use `arduino-cli` with explicit linker flags to link the Rust static library:
    ```bash
    .\arduino-cli.exe compile --fqbn rp2040:rp2040:rpipico ^
      --build-property "compiler.c.elf.extra_flags=\"-LC:\Users\Dell\Documents\pico_apple2_emulator\src\" -lapple2_core" ^
      pico_apple2_emulator.ino
    ```
    *Note: Replace the `-L` path with your absolute project `src` path if it differs.*

### Build Scripts
*   **`full_build.bat`:** The primary build script. It automates Rust compilation, library syncing, Arduino compilation, and flashing.
*   **`build_rust.bat`:** Compiles only the Rust core and syncs the library/headers to the Arduino environment.

### Deployment
1.  Connect the Pico in Bootloader mode (or via Serial if already running).
2.  Run `full_build.bat`.
3.  The script will automatically detect the COM port and flash the firmware.

## 🎹 Interaction & Controls

| Feature | Mapping |
| :--- | :--- |
| **Virtual Joystick** | Keyboard Arrow keys map to Apple II Paddle 0/1. |
| **Joystick Buttons** | Page Up / Page Down. |
| **Fn + 1** | Warm Reset (Ctrl-Reset). |
| **Fn + 2** | Cold Reset (Reboot). |
| **Fn + 3** | Disk Menu (F3) — Load `.DSK` files from SD card. |

## 📁 Key Files & Directories

*   `apple2_core/`: Rust source code for the emulator logic.
*   `pico_apple2_emulator.ino`: Main Arduino sketch and hardware setup.
*   `Apple2Core.h`: The C bridge defining the FFI interface between Rust and C++.
*   `drivers/`: Custom C++ hardware drivers.
*   `src/`: Local storage for the compiled `libapple2_core.a`.
*   `DevLog.md`: Detailed technical history and "Lessons Learned" regarding disk timing and color calibration.
*   `PROJECT_MAP.md`: High-level roadmap and status of features.

## ⚠️ Development Conventions

*   **FFI Safety:** Any changes to the Rust core API must be reflected in `Apple2Core.h`.
*   **Timing Critical:** The Apple II runs at 1.023 MHz. The `apple2_tick()` function handles cycle counting; avoid blocking operations on Core 0.
*   **Disk Write-Back:** When implementing disk writes, ensure `"r+"` mode is used for SD card files to prevent file corruption. Emulation should be paused during SD writes.
*   **Memory Management:** The RP2040 is RAM-constrained. Avoid allocating large buffers; prefer direct-from-RAM rendering.
