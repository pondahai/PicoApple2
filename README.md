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

## 🚀 快速開始

### 編譯核心 (Rust)
```bash
cd apple2_core
cargo build --target thumbv6m-none-eabi --release
```

### 編譯韌體 (Arduino)
使用專案根目錄下的 `full_build.bat` 腳本，它會自動同步核心庫並編譯上傳：
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
