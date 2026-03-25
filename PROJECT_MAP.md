# 🗺️ Pico Apple II Emulator Project Map (Updated 2026-03-25)

## 1. 核心結構 (Project Architecture)
目前的架構採用 **「零緩衝 JIT 渲染」** 與 **「動態時脈控制」**：

| 檔案路徑 | 功能說明 | 核心語言 | 備註 |
| :--- | :--- | :--- | :--- |
| `apple2_core/` | 模擬器邏輯核心 (CPU, Mem, Disk) | **Rust** | 已實作 `set_paddle` API 與寫回校驗邏輯。 |
| `PicoApple2.ino` | RP2040 雙核排程、JIT 渲染器 | **C++** | **[最新]** 實作 ANSI 轉義序列解析器，支援互動式終端機鍵盤。 |
| `Apple2Core.h` | 跨語言 API 橋接 | C | 包含完整的鍵盤、搖桿、磁碟寫回 API。 |
| `scripts/scan_env.ps1` | 開發環境自動掃描工具 | PowerShell | 自動偵測 Arduino CLI、Picotool 與程式庫路徑。 |

## 2. 硬體架構與時序 (Hardware & Timing)
- **輸入模擬:**
    - **虛擬搖桿:** 實體鍵盤與終端機方向鍵映射至 Paddle 0/1，PgUp/PgDn 映射至 Buttons。
    - **互動模式:** 實作了偵錯模式 (`Ctrl+K`) 以捕捉終端機 Raw Hex 碼。
- **動態時脈:** 1.023 MHz (Fixed) / 1.48 MHz (Turbo)。

## 3. 資源狀態 (Pico RP2040)
- **RAM 佔用:** 約 **50% (131KB)**。
- **編譯環境:** 零配置需求 (透過 `scan_env.ps1` 自動初始化)。

## 4. 待辦事項 (Backlog)
- [x] 實作 F3 磁碟切換選單。
- [x] 實作虛擬搖桿 (Joystick Emulation) 支援。
- [x] **[最新]** 實作互動式終端機鍵盤（支援方向鍵、F1-F4、選單控制）。
- [x] **[最新]** 實作開發工具鏈自動掃描與環境零配置方案。
- [x] **[已解決]** 實施「磁軌重整」邏輯，解決磁碟寫回後的 `CATALOG` 失效與「幽靈扇區」問題。
