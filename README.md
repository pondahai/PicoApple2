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

1.  **使用者責任**：玩家必須自行合法取得 Apple II 的 System ROM (`apple2_sys.rom`)、字體 ROM (`apple2_char.rom`) 與 Disk II 控制卡 ROM (`disk2.rom`)。
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
| | Button A / B | 2 / 3 | A:確認, B:返回 |

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
本專案使用 `arduino-cli` 進行自動化編譯，但底層仍需安裝 RP2040 核心：
*   **安裝 Arduino IDE**: 建議安裝 2.x 版本。
*   **安裝 RP2040 核心**: 在 Arduino IDE 的「開發板管理員」中搜尋並安裝 `Raspberry Pi Pico/RP2040` (by Earle F. Philhower, III)。本專案建議版本為 **5.5.1+**。
*   **FQBN**: `rp2040:rp2040:rpipico`

### 2. Rust 開發環境 (核心模擬器)
*   **安裝 Rust**: [rustup.rs](https://rustup.rs/)
*   **安裝編譯目標**: 
    ```bash
    rustup target add thumbv6m-none-eabi
    ```

---

## 🔨 編譯與上傳 (Build & Upload)

### 第一步：編譯核心 (Rust)
進入 `apple2_core` 目錄並編譯生成靜態庫：
```bash
cd apple2_core
cargo build --target thumbv6m-none-eabi --release
```

### 第二步：同步與編譯韌體 (Arduino)
使用專案根目錄下的 `full_build.bat` 腳本，它會自動將 Rust 生成的 `.a` 檔同步至 `src/` 並進行最終編譯與上傳：
```bash
.\full_build.bat
```

---

## 🎹 控制說明

| 功能 | 組合鍵 / 按鍵 |
| :--- | :--- |
| **Warm Reset** | Fn + 1 |
| **Cold Reset** | Fn + 2 |
| **磁碟選單** | Fn + 3 (選擇 .DSK 檔案) |
| **虛擬搖桿** | 方向鍵 (映射至 Paddle 0/1) |
| **按鈕 0/1** | Page Up / Page Down |

---

## 📜 授權

本專案採用 MIT 授權。
