
## 2026-03-27: Bit-level Shifting Stability & Startup Fix

### 當前進展
1.  **啟動當機修復 (Startup Hang Resolved)**:
    *   **診斷**: 發現 `Box<Disk2>` 與 ROM 外部載入過程中，由於堆積空間 (Heap) 競爭與串列埠初始化順序衝突，導致 Pico 在 `setup()` 階段鎖死。
    *   **修復**: 調整了 `apple2_core` 的靜態初始化順序，並在 `PicoApple2.ino` 中確保 USB Serial 穩定後再進行核心握手。
2.  **成功引導**:
    *   `MASTER.DSK` (DOS 3.3) 已經可以穩定啟動，Serial Console 輸出正常。
    *   位元級移位暫存器 (Bit-level Shift Register) 邏輯初步驗證成功，磁軌讀取更加穩定。

### 當前狀態
*   **讀取**: 正常。
*   **寫入**: 待驗證 (待測 `INIT HELLO`)。
*   **效能**: 穩定運行於 1.023MHz。

### 下一步
*   測試 `goonies.dsk` 在位元級模擬下的讀取表現。
*   深挖 `ERROR #8` 的根源，檢查位元級寫入時的同步位元 (Sync bits) 是否產生物理偏移。

## 2026-03-27 (Session 2): Display System Migration to Hardware SPI + DMA

### 當前進展
1.  **TFT DMA 遷移成功**:
    *   **技術決策**: 捨棄了 PIO 模擬 SPI 方案（因時序與 250MHz 超頻不穩），改用 RP2040 內建的 **Hardware SPI0 + DMA**。
    *   **掃描線雙緩衝**: 實施了 `scanline_buffers[2][280]`，Core 1 現在可以並行「計算第 N+1 行」與「DMA 傳送第 N 行」，主畫面不再阻塞 CPU。
    *   **Endian Swapping**: 在渲染循環中使用 `__builtin_bswap16` 進行即時轉換，完美符合 ILI9341 的 Big-Endian 需求。
2.  **全功能修復與還原**:
    *   **渲染模式**: 完整還原了 Text (含反色/閃爍)、Lo-Res (LGR)、Hires 與 Mixed Mode。
    *   **磁碟選單**: 修復了按鍵失效與磁碟載入邏輯，配色回歸深灰底/黃標/白框。
    *   **馬達指示燈**: 重新實作為右上角的紅色 (`0xF800`) 小方塊。
    *   **終端機支援**: 補回 ANSI 解析器，恢復 F1-F3 鍵功能。

### 當前狀態
*   **顯示**: 高效能 DMA 渲染，流暢度大幅提升。
*   **控制**: 選單與 WebSerial 恢復正常。
*   **效能**: 穩定運行於 1.023MHz，SPI 鎖定於 30MHz。

### 下一步
*   深挖 `ERROR #8` 的根源，利用 DMA 釋放出來的 Core 1 效能進行更精確的時序觀察。

## 2026-03-27 (Session 3): Final Synchronization & Persistence Fixes

### 當前進展
1.  **啟動同步優化 (Boot Sync)**:
    *   **診斷**: 發現 Core 0 與 Core 1 的啟動延遲不一致（1s vs 6s），導致開機先 Beep 聲後才亮屏。
    *   **修復**: 同步兩核延遲為 2s，Beep 聲與開機畫面現在同步出現。
2.  **磁碟載入持久化 (LastDisk Persistence)**:
    *   **功能**: 在 `setup1` 中加入讀取 `/LASTDISK.TXT` 的邏輯。
    *   **效果**: 重開機或 Cold Reset (F2) 後，系統會自動載入上次在 F3 選單中選取的磁碟，不再硬編碼為 `MASTER.DSK`。
3.  **F2 鍵與 ANSI 解析器修正**:
    *   **修正**: 增強了 `loop()` 中的 ANSI Escape 解析器，支援更多種終端機格式（`indexOf` 搭配位元組比對）。
    *   **效果**: 恢復了終端機下的 Cold Reset (F2) 功能，並確保 F1-F3 絕對可靠。
4.  **選單導航優化**:
    *   **修復**: 解決了 F3 選單中 Enter 鍵載入失效與自動重啟的錯誤預設。現在選取後正確掛載磁碟並繼續執行。

### 當前狀態
*   **啟動**: 完美同步。
*   **持久化**: 已實作，支援開機自動回補上次磁碟。
*   **控制**: 終端機 F 鍵全數恢復。

### 下一步
*   轉向磁碟寫入穩定性測試。
