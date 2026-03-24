# 🗺️ Pico Apple II Emulator Project Map (Updated 2026-03-22)

## 1. 核心結構 (Project Architecture)
目前的架構採用 **「零緩衝 JIT 渲染」** 與 **「動態時脈控制」**：

| 檔案路徑 | 功能說明 | 核心語言 | 備註 |
| :--- | :--- | :--- | :--- |
| `apple2_core/` | 模擬器邏輯核心 (CPU, Mem, Disk) | **Rust** | 已實作 `set_paddle` API 與寫回校驗邏輯。 |
| `pico_apple2_emulator.ino` | RP2040 雙核排程、JIT 渲染器 | **C++** | **[最新]** 實作鍵盤轉虛擬搖桿 (Joystick Emulation)。 |
| `Apple2Core.h` | 跨語言 API 橋接 | C | 包含完整的鍵盤、搖桿、磁碟寫回 API。 |

## 2. 硬體架構與時序 (Hardware & Timing)
- **輸入模擬:**
    - **虛擬搖桿:** 鍵盤方向鍵映射至 Paddle 0/1，PgUp/PgDn 映射至 Buttons。
    - **自動歸零:** 實作了按鍵放開偵測，確保搖桿準確回到中位。
- **動態時脈:** 1.023 MHz (Fixed) / 1.48 MHz (Turbo)。

## 3. 資源狀態 (Pico RP2040)
- **RAM 佔用:** 約 **50% (131KB)**。
- **穩定性:** 數位音訊與 SD 原子化載入運作穩定。

## 4. 待辦事項 (Backlog)
- [x] 實作 F3 磁碟切換選單。
- [x] 實作虛擬搖桿 (Joystick Emulation) 支援。
- [x] **[已校準]** 校準 Hires 顏色解析邏輯 (Even/Odd 與 Shift Bit 對應已正確)。
- [x] **[已解決]** 實施「磁軌重整」邏輯，解決磁碟寫回後的 `CATALOG` 失效與「幽靈扇區」問題。
