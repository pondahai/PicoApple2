# 🗺️ Pico Apple II Emulator Project Map (Updated 2026-03-26)

## 1. 核心結構 (Project Architecture)
目前的架構採用 **「零緩衝 JIT 渲染」** 與 **「即時狀態通訊協定」**：

| 檔案路徑 | 功能說明 | 核心語言 | 備註 |
| :--- | :--- | :--- | :--- |
| `apple2_core/` | 模擬器邏輯核心 (CPU, Mem, Disk) | **Rust** | 已實作 `set_paddle` API 與寫回校驗邏輯。 |
| `PicoApple2.ino` | RP2040 雙核排程、JIT 渲染器 | **C++** | **[最新]** 實作 STX 專屬協定，支援 WebSerial 真實按下/放開偵測。 |
| `Apple2Keyboard.html` | 專業虛擬控制台 (Pro Console) | **JS/HTML** | 整合 xterm.js 顯示與全鍵盤映射 (含 CTRL/F1-F3)。 |
| `scripts/scan_env.ps1` | 開發環境自動掃描工具 | PowerShell | 自動偵測 Arduino CLI、Picotool 與程式庫路徑。 |

## 2. 硬體架構與時序 (Hardware & Timing)
- **輸入模擬:**
    - **混合輸入:** 實體鍵盤矩陣與 WebSerial 狀態透過 `OR` 邏輯合併，支援多端並行控制。
    - **真實長按:** 透過 STX 協定實現了搖桿與蘋果鍵的精確長按偵測，不再依賴計時器。
- **動態時脈:** 1.023 MHz (Fixed) / 1.48 MHz (Turbo)。

## 3. 資源狀態 (Pico RP2040)
- **RAM 佔用:** 約 **50% (131KB)**。
- **編譯環境:** 零配置需求 (透過 `scan_env.ps1` 自動初始化)。

## 4. 待辦事項 (Backlog)
- [x] 實作 F3 磁碟切換選單。
- [x] 實作虛擬搖桿 (Joystick Emulation) 支援。
- [x] **[最新]** 實作 WebSerial 專業虛擬控制台，支援 Press/Release 即時狀態。
- [x] **[最新]** 修復核心並行衝突，分離實體與 Serial 的按鍵狀態空間。
- [x] **[已解決]** 實施「磁軌重整」邏輯，解決磁碟寫回後的 `CATALOG` 失效與「幽靈扇區」問題。

## 5. 已知問題與待修復 (Known Issues & FIXME)
- [ ] **FIXME (Disk Read):** `goonies.dsk` 無法正常載入。
    - **現象:** 疑似磁軌讀取時序或扇區偏移問題。
    - **備註:** 該檔案在其他模擬器可運行，排除 DSK 損壞可能。
- [ ] **FIXME (Disk Write):** DOS 3.3 對空白磁碟執行 `INIT HELLO` 時發生寫入錯誤。
    - **現象:** 寫入驗證失敗或檔案系統同步問題。
    - **備註:** 需要檢查磁軌格式化（Formatting）邏輯與 SD 卡寫回的原子性。
