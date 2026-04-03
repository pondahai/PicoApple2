# 🍎 PicoApple2

**PicoApple2** 是一個專為 Raspberry Pi Pico (RP2040) 設計的高效能 Apple II 模擬器。它結合了 **Rust** 撰寫的精確模擬核心與 **C++/Arduino** 實作的雙核渲染架構，能在微控制器上實現全速、全彩的 Apple II 體驗。

---

## ✨ 核心特色

*   **🦀 Rust 驅動核心**: 使用 Rust (`thumbv6m-none-eabi`) 實作 6502 CPU、記憶體映射與 Disk II 控制器，確保模擬精確度。
*   **⚡ 雙核加速架構**: 
    *   **Core 0**: 運行模擬邏輯、處理輸入與 SD 卡 I/O。
    *   **Core 1**: 專職 **JIT 視訊渲染**，直接從 RAM 即時生成 ILI9341 (SPI) 像素。
*   **🚀 極致效能**: 預設超頻至 **250MHz**，在磁碟運轉時自動放開限速，實現極速載入。
*   **💾 完整的磁碟支援**: 支援 `.DSK` 檔案讀寫，實作了物理驗證與磁軌同步技術，確保資料持久化安全。
*   **🎮 現代化控制**: 內建虛擬搖桿映射、功能鍵選單與 SD 卡檔案瀏覽器。

---

## ⚠️ 重要聲明 (Legal & ROM Requirement)

本專案**不提供**且**不包含**任何受版權保護的 Apple II ROM 檔案或軟體。

1.  **使用者責任**：玩家必須自行合法取得 Apple II 的 System ROM (`apple2_sys.rom`)、字體 ROM (`apple2_char.rom`) 與 Disk II 控制卡 ROM (`disk2_p5.rom`)。
2.  **檔案放置**：請將取得的 ROM 檔案放置於 `apple2_core/src/` 目錄下後再進行編譯。
3.  **合規使用**：本專案僅供學術研究與個人復刻體驗使用，請確保您遵守當地的版權法律規範。

---

## 🛠️ 硬體需求

*   **處理器**: Raspberry Pi Pico (RP2040)
*   **顯示器**: ILI9341 2.8" SPI TFT (320x240)
*   **儲存**: SD 卡模組 (SPI)
*   **輸入**: 鍵盤矩陣 (74HC165/74HC595 驅動)

---

## 🔌 GPIO 接線表 (Pin Mapping)

| 類別 | 功能 | Pico 引腳 (GPIO) | 說明 |
| :--- | :--- | :--- | :--- |
| **顯示器 (SPI0)** | SCK / MOSI / MISO | 18 / 19 / 16 | 連接至 ILI9341 |
| | CS / DC / RST / BL | 17 / 20 / 21 / 22 | |
| **SD 卡 (SPI1)** | SCK / MOSI / MISO / CS | 10 / 11 / 12 / 13 | 連接至 SD 卡模組 |
| **音效 (Audio)** | Sound Out | 7 | 1-bit PWM 輸出 (需接低通濾波器與放大器) |
| **鍵盤矩陣** | Data Out / Latch | 15 / 14 | 連接至 74HC595 / 74HC165 |
| | Clock / Data In | 26 / 27 | |
| **選單按鈕** | Up / Down | 9 / 5 | 輔助導航按鈕 (Pull-up) |
| | Left / Right | 8 / 6 | 搖桿 X 軸模擬 |
| | Button A / B | 2 / 3 | A:確認 (PB0), B:返回 (PB1) |
| | Menu (Start) | 4 | 進入磁碟選擇選單 (F3) |
| | ALT | 28 | 預留功能鍵 |

---

## 🔊 音效電路建議 (Audio Circuit Note)

由於 GPIO 7 輸出的是未經過處理的 1-bit 數位訊號，建議建構以下電路以獲得最佳音質：
1.  **低通濾波器 (LPF)**：使用簡單的 RC 電路（如 1kΩ 電阻與 100nF 電容）來濾除高頻數位雜訊。
2.  **音訊放大器**：建議連接至 **PAM8403** 或類似的 D 類放大器模組來驅動 4Ω/8Ω 喇叭。
3.  **隔離電容**：在進入放大器前建議串接一個 10uF 的電解電容以隔絕直流分量 (DC Offset)。

---

## 🚀 環境架設 (Environment Setup)

在開始編譯前，請確保您的開發環境已完成以下配置：

### 1. Arduino 開發環境
本專案建議使用 **Arduino IDE 2.x** 進行基礎設定，並透過自動化腳本進行編譯。

*   **安裝 RP2040 核心**: 在 Arduino IDE 的「開發板管理員」中搜尋並安裝 `Raspberry Pi Pico/RP2040` (by Earle F. Philhower, III)。**版本要求: 5.5.1+**。
*   **安裝必要程式庫**: 在「程式庫管理員」中搜尋並安裝以下元件：
    1.  **Adafruit GFX Library**
    2.  **Adafruit ILI9341**
*   **FQBN**: `rp2040:rp2040:rpipico`

### 2. Rust 開發環境 (核心模擬器)
*   **安裝 Rust**: [rustup.rs](https://rustup.rs/)
*   **安裝編譯目標**: 
    ```bash
    rustup target add thumbv6m-none-eabi
    ```

---

## 🔨 編譯與上傳 (Build & Upload)

本專案已實作 **動態環境掃描系統**，會自動從您的 Arduino IDE 設定中抓取工具鏈路徑。

### 第一次執行 (環境檢查)
在開始編譯前，請先執行 `check_env.bat`。此腳本會：
1. 掃描 `~/.arduinoIDE/arduino-cli.yaml` 取得路徑。
2. 自動尋找 `arduino-cli.exe` 與 `picotool.exe` 的實際位置。
3. 產生 `build_env.bat` 環境設定檔。
4. 在視窗中顯示掃描結果供您確認。

```bash
.\check_env.bat
```

### 快速編譯與上傳 (自動化腳本)
確認環境正確後，您可以使用以下腳本進行開發：

*   **一鍵全編譯** (`full_build.bat`):
    自動執行環境掃描 -> 編譯 Rust 核心 -> 同步靜態庫 -> 編譯 `PicoApple2.ino` -> 自動透過 1200bps 重置並上傳至 Pico。
    ```bash
    .\full_build.bat
    ```
*   **僅更新 Rust 核心** (`build_rust.bat`):
    當您修改了 `apple2_core/` 下的 Rust 代碼時，執行此腳本會編譯並將產出同步至 Arduino 程式庫目錄，之後您可以直接在 Arduino IDE 中點擊上傳。

---

## 🖥️ 專業虛擬終端機 (Pro Console)

本專案內建一個基於 **WebSerial** 的高效能虛擬控制台，讓您可以透過電腦鍵盤完美模擬 Apple II 的所有操作。

### 如何啟動
在專案根目錄下執行 `terminal.bat`，系統將自動以 Chrome/Edge 瀏覽器開啟 `Apple2Keyboard.html`。

### 核心功能
*   **全鍵盤映射**: 捕捉電腦按鍵並轉換為 Apple II ASCII 碼，支援 **CTRL 組合鍵**、Enter、Backspace 與 Esc。
*   **即時狀態協定 (STX)**: 不同於傳統終端機，本控制台實作了專屬的按下/放開狀態同步，實現真正的「長按」行為。
*   **專業級終端顯示**: 內建 `xterm.js` 渲染引擎，同步顯示 Pico 輸出的所有 Log 與 Debug 訊息。
*   **記憶連線**: 只要連線過一次，下次啟動僅需點擊 `RECONNECT` 即可快速上線。

### 操作對應表
| 功能 | 電腦鍵盤按鍵 | 說明 |
| :--- | :--- | :--- |
| **打字輸入** | `A-Z`, `0-9`, `符號` | 自動轉換為 Apple II ASCII (預設大寫) |
| **組合鍵** | `CTRL + A-Z` | 傳送標準控制碼 (如 Ctrl-C 中斷) |
| **搖桿軸** | `方向鍵 (Arrows)` | 精確模擬 Paddle 0/1 的狀態 (支援長按) |
| **蘋果按鈕** | `Page Up / Down` | 對應 PB0 (Open-Apple) 與 PB1 (Closed-Apple) |
| **系統重置** | `F1` / `F2` | F1: Warm Reset (Ctrl-Reset), F2: Cold Reset |
| **磁碟選單** | `F3` | 開啟 SD 卡選單，之後可直接用方向鍵導航 |

---

## 📂 專案結構 (Project Structure)


*   `PicoApple2.ino`: 主程式進入點（原 `pico_apple2_emulator.ino`）。
*   `apple2_core/`: Rust 撰寫的 Apple II 模擬器核心。
*   `scripts/scan_env.ps1`: 動態環境掃描核心腳本。
*   `check_env.bat`: 環境驗證工具。
*   `Apple2Core.h`: C/Rust FFI 接口定義。
*   `src/`: 存放編譯後的 `libapple2_core.a` 靜態庫。


| 功能 | 組合鍵 / 按鍵 | 說明 |
| :--- | :--- | :--- |
| **Warm Reset** | Fn + 1 | F1 - 軟重啟 |
| **Cold Reset** | Fn + 2 | F2 - 冷重啟並重新載入磁軌 0 |
| **磁碟選單** | Fn + 3 | F3 - 開啟 .DSK 檔案選擇選單 |
| **模式切換** | Fn + 4 | F4 - 切換「方向鍵」為 搖桿模式 / 鍵盤模式 |
| **大寫鎖定** | Fn + C | Caps Lock 切換 |
| **虛擬搖桿** | 方向鍵 | 映射至 Paddle 0/1 (僅在 F4 搖桿模式下) |
| **按鈕 0/1** | PageUp / PageDown | 對應真實 Apple II 的蘋果鍵 / 搖桿按鈕 |

---

## 📜 授權

本專案採用 MIT 授權。
