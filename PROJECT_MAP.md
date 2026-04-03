# 🗺️ Pico Apple II Emulator Project Map (Updated 2026-04-03)

## 1. 核心結構 (Project Architecture)
目前的架構採用 **「交錯式光柵同步渲染 (Interlaced VBLANK Sync)」**、**「無鎖原子狀態 (Lock-free Atomics)」** 與 **「即時狀態通訊協定」**：

| 檔案路徑 | 功能說明 | 核心語言 | 備註 |
| :--- | :--- | :--- | :--- |
| `apple2_core/` | 模擬器邏輯核心 (CPU, Mem, Disk) | **Rust** | 支援 bit-level 磁碟移位暫存器模擬。 |
| `PicoApple2.ino` | RP2040 雙核排程、JIT 渲染器 | **C++** | 整合 Hardware SPI + DMA 傳輸。 |
| `TFT_DMA.cpp/h` | 底層顯示驅動 | **C++** | 自定義非阻塞傳輸與 UI 繪製邏輯。 |
| `Apple2Keyboard.html` | 專業虛擬控制台 (Pro Console) | **JS/HTML** | 透過 STX 協定實現 Press/Release 同步。 |

## 2. 硬體架構與時序 (Hardware & Timing)
- **輸入模擬:**
    - **混合輸入:** 實體鍵盤矩陣與 WebSerial 狀態透過 `OR` 邏輯合併。
    - **精確導航:** 選單模式下自動切換方向鍵為選單導航 (`g_menu_cmd`)。
- **動態時脈:** 1.023 MHz (Fixed) / 1.48 MHz (Turbo)。
- **顯示效能:** SPI 頻率鎖定 62.5MHz，DMA 負責傳輸，Core 1 負載大幅下降。

## 3. 資源狀態 (Pico RP2040)
- **RAM 佔用:** 約 **50% (131KB)**。
- **編譯環境:** 支援 Arduino CLI 與全自動環境掃描。

## 4. 待辦事項 (Backlog)
- [x] 實作 F3 磁碟切換選單與穩定按鍵導航。
- [x] 實作 TFT DMA 掃描線雙緩衝。
- [x] 修復 Rust 核心位元級重構後的啟動當機。
- [x] 實施「磁軌重整」與「Safe Write-back」。

## 5. 已知問題與待修復 (Known Issues & FIXME)
- [x] **FIXME (Disk Write) - RESOLVED:** DOS 3.3 對磁碟執行 `SAVE` 與寫入錯誤。
    - **解決方案:** 取消定時器 `tick()` 在 `Q6=0` 期間的指針自增跳號，確保由 `STA $C08D` 驅動磁碟寫入陣列時達成 100% 長度與時序同步。
- [ ] **FIXME (Disk Read):** `goonies.dsk` 無法正常載入（位元級模擬需進一步校準）。
