# 🍎 PicoApple2 開發日誌 (2026-03-28)

## 核心重構回顧：位元級模擬 (Bit-level) 的實驗與撤回

### 1. 實驗結論 (Post-mortem)
*   **現象**: 試圖將 `tick()` 精度提升至 4-cycle 並導入 `read_shift_register` 狀態機，結果導致核心在實體 Pico 上啟動死鎖（Startup Hang），且對於 `goonies.dsk` 的讀取相容性並無實質提升。
*   **決策**: **正式撤回位元級重構**。目前核心代碼已回退至穩定的 **32-cycle 位元組級 (Byte-level)** 實作，確保系統基礎運行穩定。
*   **代碼殘留說明**: 為了保留實驗證據，`apple2_core/src/disk2_test.rs` 中的位元級測試案例暫不刪除，但已知其與目前 `Disk2` 結構不相符。

### 2. 核心診斷：Goonies 磁碟當機事件
*   **診斷**: 核心在處理非標準 Sync 位元時，目前的 32-cycle 實作可能陷入 `while` 搜尋死循環，或者發生 FFI 指標溢位，導致雙核排程崩潰。
*   **新策略**: 導入「防禦性核心檢查」。在 `byte_index` 推進與 `while` 迴圈中加入強制邊界判定與超時機制。

### 3. ERROR #8 寫入校準方案 (Phase Compensation)
*   **分析**: 寫入偏移源於 CPU 指令時間與 32-cycle 磁軌邊界的不對齊（相位抖動）。
*   **方案**: 
    *   **相位捨入補償**: 在 `read_io` 觸發時，根據 `cycles_accumulator` 的剩餘值進行時序微調，而非單純捨棄。
    *   **Q6/Q7 原子性強化**: 確保磁頭狀態切換與 `tick()` 的步進具有更強的物理同步性。

---

## 當前狀態 (Current Status)
*   **核心**: 32-cycle Byte-level (Stable).
*   **顯示**: DMA + Hardware SPI (Fluent).
*   **待辦**: 修正核心掛起邏輯，修復寫入相位偏移。
