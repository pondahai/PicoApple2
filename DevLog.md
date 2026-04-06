# Pico Apple II Emulator - Development Log

## 2026-04-03: JIT Rendering Overhaul & Zero-Noise Input (VBLANK Sync)

### 1. 交錯式光柵同步渲染 (Interlaced VBLANK Sync Rendering)
*   **背景**: 過去 40ms 限速器的停走式批次渲染導致 25 FPS，且無法模擬 Raster Effects。
*   **優化內容**:
    *   在 Rust 核心引入 `core::sync::atomic`，實作無鎖的 `apple2_get_beam_y()` 暴露電子束實體位置。
    *   Core 1 `loop1()` 改為追逐電子束 (Beam-chasing)，實作奇偶場交錯渲染 (Even/Odd Fields)。
*   **成果**: 達成 60 Fields/sec 的平滑流暢度，徹底解決畫面撕裂，並支援畫面中途改變影片模式的光柵特效。

### 2. VBLANK 矩陣掃描與硬體串擾修復
*   **背景**: 高頻 62.5MHz SPI DMA 在背景連續運作時，會對 GPIO 產生嚴重 EMI 與接地彈跳 (Ground Bounce)，導致搖桿斷訊 (Ghost release events)。
*   **優化內容**:
    *   將硬體矩陣與 GPIO 掃描 (`scan_matrix()`) 完全移入垂直空白區 (VBLANK, Y >= 192)。
    *   使用 `tft_dma.waitTransferDone()` 強制停止所有 SPI 通訊，確保掃描環境 100% 乾淨無雜訊。
    *   實作 40ms 的後緣防彈跳 (Trailing-edge Debounce) 濾波器。
*   **成果**: 搖桿恢復完美連貫，徹底根除「不連續桿」現象。

### 3. 雙軌制輸入緩衝 (Dual-track Input Buffering)
*   **背景**: 為了追求極致零延遲一度移除了鍵盤 FIFO，導致從終端機貼上長串 BASIC 代碼時發生嚴重漏字。
*   **優化內容**:
    *   還原 128-byte 鍵盤 FIFO 緩衝區 (`g_key_fifo`)，專屬於序列埠與鍵盤輸入，保證高速貼上操作 100% 不漏字。
    *   搖桿方向與搖桿按鈕維持實體直通 (Zero-buffer)，直接寫入記憶體不受 FIFO 影響。
*   **成果**: 達成「文字輸入不漏字、搖桿操作零延遲」的完美平衡。

### 4. 變速模擬功能 (Speed Multiplier)
*   **優化內容**: 
    *   引入 `g_speed_multipliers` 陣列，支援 x1.0, x1.2, x1.4, x1.5 四種速率。
    *   新增 `Fn + 5` (F5) 快捷鍵，可即時循環切換模擬速度。
    *   在畫面下方顯示當前倍率提示。
*   **成果**: 允許玩家在載入或特定遊戲情境下加速執行。

---

## 2026-04-02: SPI Performance Optimization (High Speed Restoration)

### 1. SPI 頻率提升
*   **背景**: 之前為了穩定性（特別是長杜邦線連接）將 SPI 頻率降低。在確認硬體接線穩定後，今天執行了高速恢復。
*   **優化內容**:
    *   **TFT (SPI0)**: 從 30MHz 提升至 **62.5MHz**。這在 250MHz 超頻下對應 `clk_sys / 4`，顯著降低了 Core 1 在 `waitTransferDone()` 上的等待時間，提升了渲染吞吐量。
    *   **SD (SPI1)**: 從 10MHz 恢復至 **20MHz**。加快了磁軌載入與 `flushDirtyTrack()` 的寫回速度。
*   **成果**: 
    *   Core 1 的渲染循環現在更加流暢，為未來的視訊效果留出了更多餘裕。
    *   磁碟讀寫延遲感降低。

---

## 專案目標
將以 Rust 撰寫的 Apple II 模擬器核心 ([apple2emu](https://github.com/pondahai/apple2emu.git)) 移植到 Raspberry Pi Pico (RP2040) 上，使用 C++ (Arduino 框架) 負責硬體 I/O 與雙核排程。

## 成功關鍵技術 (1.09 MHz 全速 + 磁碟動態載入)

### 1. 零緩衝 JIT 渲染 (Just-In-Time Rendering)
*   **優化**: 移除像素緩衝區，Core 1 直接從 Apple II RAM 即時生成像素。
*   **成果**: RAM 佔用大幅下降至 **50% (131KB)**。

### 2. 磁碟寫入完美修復 (2026-03-24)
*   **技術 A: Q7 寫入鎖定 (Write Lock)**: 只要 Q7 暫存器開啟，就強制停用讀取電路，防止磁軌上的舊位元覆蓋 `data_latch`。
*   **技術 B: 髒位元優先解碼 (Dirty-aware Denibblization)**: 在記憶體受限的分頁架構下，利用 `dirty_mask` 辨識軟體真正寫入的扇區，排除初始化產生的偽信號。
*   **成果**: **SAVE 功能完美運作**，DSK 內容能真實持久化至 SD 卡。

### 3. 互動式終端機鍵盤 (Interactive Terminal Keyboard) (2026-03-25)
*   **優化**: 實作 `serial_monitor.ps1` 雙向通訊，捕捉電腦端鍵盤輸入並傳送至 Pico。
*   **技術**: 
    *   **ANSI 轉義序列解析**: 在 Core 0 實作狀態機，解析 `ESC [A` (上) 等序列，映射至模擬器方向鍵與 F1-F4。
    *   **按鍵捕捉偵錯模式 (Monitor Mode)**: 按下 `Ctrl+K` 進入偵診模式，Pico 會即時回傳接收到的 Hex 碼，用於校準不同終端機的按鍵映射。
*   **成果**: 開發者可完全透過電腦終端機操作模擬器（包含磁碟選單），無需實體鍵盤硬體。

### 4. 專業虛擬控制台 (WebSerial Pro Console) (2026-03-26)
*   **技術 A: STX 即時狀態協定**:
    *   **原理**: 為了解決傳統終端機無法傳送「放開按鍵 (Release)」的物理限制，實作了基於 `0x02` (STX) 的四位元組封包協定 `[STX][Type][Idx][State]`。
    *   **效果**: 搖桿與蘋果鍵 (PB0/PB1) 現在支援精確的長按行為，不再依賴不穩定的自動釋放計時器。
*   **技術 B: 核心間輸入狀態分離 (State Separation)**:
    *   **挑戰**: Core 1 的實體掃描頻率極高，會不斷覆寫 Core 0 從 Serial 接收到的虛擬按鍵狀態。
    *   **優化**: 分離 `joy_*` (實體) 與 `ser_joy_*` (虛擬) 變數空間。在最終設定 Apple II 暫存器時採用 `OR` 運算合併狀態，確保兩端輸入均能同時生效。
*   **技術 C: 智慧分流 (Smart Rerouting)**:
    *   **邏輯**: 實作了選單感知功能。當 F3 選單開啟時，WebSerial 傳入的「搖桿方向」會自動被重新路由為 `g_menu_cmd` (選單導航指令)，實現無縫的選單操作體驗。
*   **成果**: 開發出一個整合 `xterm.js` 的網頁控制台，取代了傳統的 CMD/PuTTY，提供了近乎實體鍵盤的零延遲操作感。

### 5. 磁碟核心硬體還原 (Disk II Hardware Accuracy) (2026-03-26)
*   **優化 A: 移除位元組過濾**:
    *   **問題**: 舊核心只接收 `(val & 0x80) != 0` 的位元組，導致非標準 nibbles 的磁碟（如 Goonies）讀取失敗。
    *   **修復**: 還原真實移位暫存器行為，磁頭持續更新 `data_latch`。
*   **優化 B: 寫入流物理同步**:
    *   **問題**: 舊核心在寫入時手動推進 `byte_index`，容易與 `tick()` 產生「雙倍步進」，破壞 `INIT` 格式化佈局。
    *   **修復**: 寫入時僅更新 Latch 並標記 Dirty，由 32-cycle 的 `tick()` 唯一驅動指標前進，確保位元流與物理旋轉同步。
*   **優化 C: 扇區搜尋容錯**:
    *   **修復**: 擴展 `denibblize` 掃描視窗至 60 bytes，提升對寫入後微小偏移的識別率。
*   **成果**: `goonies.dsk` 啟動成功，`INIT HELLO` 格式化寫回 SD 卡功能穩定。

### 6. 寫入安全性與彈性磁軌重構 (2026-03-26)
*   **技術 A: Read-Modify-Write (R-M-W)**:
    *   **目的**: 防止解碼失敗時誤刪扇區。
    *   **實作**: 存檔前先從 SD 讀取 4096 bytes，僅覆蓋成功解碼的扇區後再寫回。
*   **技術 B: 彈性磁軌 (Elastic Track)**:
    *   **優化**: 擴張物理磁軌至 6656 位元組（最大 RAM 空間）。
    *   **效果**: 解決了長寫入序列（如第 16 扇區）因為指標捲回而踩毀磁軌開頭（第 1 扇區標頭）的物理衝突。
*   **當前挑戰**:
    *   **ERROR #8 依舊存在**: SAVE 後的目錄區出現 I/O ERROR。
    *   **偵錯發現**: `Updated 15 sectors` 說明解碼器仍漏掉一個關鍵扇區。即使有 R-M-W，若該扇區是軟體「新寫入」的內容，漏掉它就代表寫入失敗。
    *   **下階段方向**: 考慮引入「位元級移位暫存器 (Bit-level Shift Register)」模擬，而不僅是位元組級，以徹底消除寫入時的相位抖動。

---

## 磁碟寫入研發避坑指引 (Crucial Lessons Learned)

### 🚨 坑 5: Q7/Q6 狀態機模擬過於簡化 (New!)
*   **現象**: 核心 tick 裡誤判寫入模式，導致 Latch 資料在被刻入磁軌前就被讀回來的舊資料「稀釋」。
*   **教訓**: 磁頭寫入電路 (Q7) 的物理權限高於讀取狀態。在模擬時，必須確保寫入模式下讀取操作是「無效」或「不更新暫存器」的。

### 🚨 坑 4: 寫入位移與幽靈扇區 (Ghost Sectors)
*   **教訓**: 由於時序微偏，寫入標頭可能偏移。解決方案是「髒位元優先解碼」，即在多個候選扇區中選取被軟體「改動最多」的那個。

### 🚨 坑 1: Arduino `FILE_WRITE` 陷阱
*   **教訓**: 必須使用 `"r+"` 模式進行原地覆蓋寫入，否則資料會被 append 到檔案末端。

---

## 🛠️ 硬體調試與偏差記錄 (Hardware Miswiring Workarounds)

由於實驗性硬體階段的接線失誤，程式碼中實作了以下「軟體補償」邏輯，在修復硬體前請勿改動：

### 1. 鍵盤 S / X 對調 (暫時性)
*   **狀態**: **維持對調** (2026-03-24)
*   **原因**: 鍵盤矩陣 Row 6 與 Row 7 的起始引腳在 PCB 上接反。
*   **影響**: 按下鍵盤上的 `S` 會觸發 `X` 的掃描碼，反之亦然。
*   **代碼位置**: `pico_apple2_emulator.ino` 中的 `keymap_base[6][0]` 與 `keymap_base[7][0]`。

### 2. Page Up (Joy Btn 0) / '?' 對調 (新加入)
*   **狀態**: **維持對調** (2026-03-24)
*   **原因**: 控制按鈕與鍵盤矩陣中的 `/` (Shift 為 `?`) 引腳物理位置接反。
*   **影響**: 按下 Page Up 鍵會送出 `?` 字元，按下 `?` 鍵會觸發模擬器的 Page Up 功能。
*   **代碼位置**: `keymap_base` 中的 `[5][7]` (原 PGUP) 與 `[7][4]` (原 `/`)。

### 3. Page Down (Joy Btn 1) / '=' (Shift 為 '+') 對調 (暫時性)
*   **狀態**: **維持對調** (2026-04-01)
*   **原因**: 控制按鈕與鍵盤矩陣中的 `=` 引腳物理位置接反。
*   **影響**: 按下 Page Down 鍵會送出 `=` 字元，按下 `=` 鍵會觸發模擬器的 Page Down 功能。
*   **代碼位置**: `keymap_base` 中的 `[4][5]` (原 `=`) 與 `[6][7]` (原 PGDN)。

---

## 模擬器控制熱鍵 (Fn Mapping)
| 熱鍵 | 功能 | 說明 |
| :--- | :--- | :--- |
| **Fn + 1** | Warm Reset (F1) | Ctrl-Reset 效果，不重載磁碟。 |
| **Fn + 2** | Cold Reset (F2) | 強制重啟並從磁軌 0 重新開機。 |
| **Fn + 3** | Disk Menu (F3) | 開啟 SD 卡 DSK 檔案選擇選單。 |
| **Fn + C** | Caps Lock | 切換大小寫鎖定（預設為 ON）。 |

---

### 當前進展
1.  **啟動同步與死鎖修復**:
    *   實作了 `g_boot_ready` 旗標，確保 Core 0 等待 Core 1 初始化 SD 卡後才開始執行 6502 核心。
    *   重新排列 `loop()` 邏輯，將 Serial 處理優先級提升至同步旗標之上，解決了初始化失敗導致的 USB 串列埠死鎖。
2.  **RESET 功能補全**:
    *   **F1 (Warm Reset)**: 呼叫 `apple2_warm_reset()`，模擬 Apple II 的 Ctrl-Reset。
    *   **F2 (Cold Reset)**: 呼叫 `apple2_reset()` 並強制重載第 0 軌，實現真正的冷啟動。
3.  **SD 卡相容性優化**:
    *   將 SD SPI 頻率從預設降低至 **10MHz**，顯著提升了使用長杜邦線連接時的掛載成功率。
    *   在 `scanDiskFiles` 中加入了 Serial 偵錯輸出，能即時回報磁碟掃描狀態。
4.  **GPIO 按鍵響應優化**:
    *   將 GPIO 讀取移出受限的渲染循環，現在按鍵掃描頻率不再受 40ms 幀率限制，解決了「按鍵沒反應」的體感問題。
