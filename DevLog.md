# Pico Apple II Emulator - Development Log

## 2026-06-24: 上電延遲改為「有上限輪詢」+ 燒錄/編譯流程備忘（實機燒錄通過 ✅）

*   **改動**: 把 Core 0 serial 與 Core 1 開頭兩段 `delay(2000)` 盲等改成 **bounded poll**（輪詢 + 2 秒上限）。新增 `g_core0_ready`：Core 0 完成 `apple2_init()` 後置 true，Core 1 輪詢它放行。最壞情況等同舊行為（零回歸），正常情況省下大半開機等待。serial 那段務必保留上限——無上限的 `while(!Serial)` 會讓獨立開機卡死。
*   **流程備忘（重要，省 token）**: 詳見 `DevLog_Entry_2026-06-24.md` 第 3 節。重點：①`build_env.bat` 變數持久可直接讀（別重跑 `scan_env.ps1`，`-ExecutionPolicy Bypass` 會被擋）；②只驗證編譯就手動跑 cargo→copy .a→arduino-cli compile，**別整支跑 `full_build.bat`**（會上傳+`pause` 卡住）；③燒錄前先進 BOOTSEL——`picotool reboot -f -u` 回 255 也可能已生效（看 COM port 是否消失 + `picotool info` exit 0），備援是對 COM14 開 1200bps；Pico 本機 serial 在 **COM14**。
*   **成果**: Rust + Arduino 編譯 exit 0（Flash 8% / RAM 38%）；`picotool load -x` 燒錄並自動重啟運行 ✅。

## 2026-06-20: 無碟 beep 變低音根因 + SD 熱插拔支援（實機驗證通過 ✅）

### 背景：另一條與 warm-reset 無關的低音 bug
*   先前（6/12）修過 warm-reset 漏排空 audio ring 的破音。本次是**獨立的另一條鏈**：沒插 SD 卡時 beep 音高偏低、插卡就正常。兩者共用「現在到底有沒有可用的碟」這個沒人維護的狀態。

### 1. 根因：無碟時 `needs_reload` 卡死 → tick 吞吐崩潰
*   **診斷方法**: 既有 `bell_timing`/`nosd_beep` 都量在「週期域」，而本 bug 不在週期域（無碟/有碟翻轉在週期域間隔相同），是**真實時間吞吐**問題。新增 `nosd_throughput` 測試，直接量「每次 `apple2_tick()` 呼叫推進幾個週期」。
*   **發現**: 無碟開機時 Disk II ROM 步進磁頭（曾到 track 1 再回 0）→ `disk2.step_motor` 設 `needs_reload=true`。該旗標只在 `apple2_load_track` 被呼叫時清除；無 `diskFile` 時韌體 `loadSingleTrack` 第一行就 return，**永不清** → `needs_reload` 從第 ~225 次 tick 呼叫起永遠卡 true。而 `apple2_tick` 批次迴圈 `if needs_reload { break }` 於是每次只跑一條指令。
*   **實測**: bug 原貌平均 **2.8 週期/呼叫**、100% 崩塌；修復後 **825 週期/呼叫** → 吞吐差 **~295 倍**。模擬遠慢於真實時間，cycle-accurate 音訊重放被拖長 → 低音。
*   **修正**: 把提前 break 條件 gate 成 `is_disk_loaded && needs_reload`（與 `apple2_needs_disk_reload` 一致）——無碟就沒軌可載，跑滿整批才正確。有碟路徑不變（`bell_timing` 仍 935Hz 無回歸）。實機確認無碟/有碟 beep 都正常。

### 2. SD 熱插拔（`apple2_eject` + 軟體輪詢狀態機）
*   **動機**: 上面只解「開機無碟」。**運行中拔卡會讓 bug 復活**（`is_disk_loaded` 仍 true、換軌時 `needs_reload` 又卡死）。
*   **核心**: 新增 `apple2_eject()` 清 `is_disk_loaded`+`needs_reload`，讓拔卡回到無媒體狀態、break 持續被 gate 掉。`is_disk_loaded` 成為速度修復與熱插拔狀態機共用的唯一真相。`eject_keeps_throughput` 測試：hot-removal eject 前 2.5、eject 後 824.7 週期/呼叫。
*   **韌體**: `g_sd_mounted` 旗標；拔卡走惰性（`loadSingleTrack`/`flushDirtyTrack` 存取失敗 → `markSdRemoved`：close + `SD.end` + `apple2_eject`）；插卡走輪詢。本板卡座**無硬體 card-detect**（microSD pin2 DAT3/CD 已接 GPIO13 當 CS），只能軟體偵測。

### 3. 踩坑（重要）：`SD.begin` 不能拿來當「插卡偵測」
*   **症狀**: 加了輪詢後，**無卡時模擬整個變超慢**，一插卡瞬間恢復正常並開始讀碟。
*   **根因**: `SD.begin()` 在無卡時卡在 SdFat 的 ACMD41 約 **2 秒**逾時。每 500ms 輪詢 → Core 0 幾乎一直困在 begin 裡。
*   **修正**: 改用低階 `sdCardProbe()`——SPI1 以 400kHz 送 CMD0，有卡數 byte 內回非 0xFF、無卡持續 0xFF，**~1ms 判定**；確認有卡才走昂貴的 `SD.begin`。並對 SD_MISO 開內部上拉，讓無卡浮接穩定讀 0xFF、避免誤判。
*   **教訓**: 偵測手段的失敗成本要低。`SD.begin` 兼具偵測與掛載，但 2 秒失敗成本不適合高頻輪詢——偵測與掛載要分開。Core 0 上任何長阻塞都會直接拖垮模擬實時性。
*   **成果**: 實機三情境（無卡正常速度 / 插卡瞬間掛載讀碟 / 運行中拔卡退片且 beep 不崩）全數通過 ✅。

## 2026-06-18: 顯示管線重疊 + 修掉潛藏的 SPI 排空 Bug（消除動態梳狀，實機驗證通過 ✅）

### 背景：受 SPI/LCD 頻寬限制的隔行渲染
*   電子束追逐渲染為了在 62.5MHz SPI 下跟上 60Hz，採奇偶場交錯（每場只畫 96/192 行）。因 ILI9341 是保持型面板，兩場疊加**空間解析度是滿的**，代價是每條線實際 30Hz 更新 → **動態畫面出現梳狀 (combing)**。

### 1. 管線重疊（回收被串列化的運算時間）
*   **問題**: `loop1()` 原本是 `waitTransferDone → 算整條線 → startFrame → 啟動DMA`，掃描線**等 DMA 結束才開始算**，雙緩衝 `scanline_buffers[2]` 形同虛設，每行時間 ≈ 運算 + DMA 相加。
*   **修正**: 把運算搬到 `waitTransferDone` **之前**，讓本行運算與上一行的 DMA 並行（兩者讀寫不同 buffer，`current_buf_idx` 在送出後才翻，天然不衝突）。每行時間降到約 `max(運算, DMA) + 設窗開銷`。
*   **成果**: 餘裕足以讓繪製跟上電子束，**動態梳狀消失**。

### 2. 踩坑（重要）：DMA 完成 ≠ SPI 傳輸完成
*   **症狀**: 重疊後出現①每行末端殘留像素、會慢慢淡去；②**跑久了整片全白、時序錯亂**。
*   **根因**: `waitTransferDone()` 只 `dma_channel_wait_for_finish_blocking`，那只代表資料填進 SPI TX FIFO，**PL022 移位暫存器可能還在打最後 1~2 byte**。舊碼那段 ~50µs 的整行運算**意外**讓 FIFO 排空，遮住了問題；重疊後 `waitTransferDone` 緊貼 `startFrame`、中間無延遲 → 下一個命令（CASET）在前一行資料還在線上時就送出 → command/data 對撞，累積性失步 → CASET/色彩模式被汙染 → 全白。
*   **修正**: `waitTransferDone()` 等完 DMA 後，再 `while (spi_is_busy(_spi)) tight_loop_contents();` 等 PL022 `BSY` 清零（移位完成且 FIFO 排空）才返回。代價僅數百奈秒，重疊好處完整保留。
*   **教訓**: RP2040 上凡是「DMA 餵 SPI → 之後要拉 CS 或送命令」的轉換點，**都必須額外等 `spi_is_busy` 為假**，不能只等 DMA。原本能跑只是被旁邊的耗時運算巧合遮住。

## 2026-06-12: 聲音模擬修正——CPU 分支週期 Bug 與 Cycle-Accurate 重放（實機驗證通過 ✅）

### 1. CPU 分支週期重複計算（影響全機時序，不只聲音）
*   **診斷方法**: 新增 `bell_timing` 測試——開機到 BASIC 後送 Ctrl-G 觸發 ROM BELL，記錄每次 $C030 翻轉的模擬週期時間戳。
*   **發現**: 半週期 624 cycles（818 Hz），但真實硬體是 ~546（935 Hz）。差值 72 恰等於 WAIT 迴圈每半週期的 taken 分支數 → taken 分支被算成 4 cycles（match arm 回傳 3 又被 `branch()` 加 1），真實 6502 是 3。
*   **影響**: 所有數週期的程式慢 ~13%——音高低近兩個半音、delay 迴圈拖長、遊戲節奏偏慢。
*   **修正**: 分支 opcode base 改回 2，taken +1 / 跨頁 +2 由 `branch()` 統一提供。修正後 BELL = 546 cycles / 935 Hz / 102ms，教科書數值。

### 2. 音訊架構：時間戳重放 (Timestamp Replay)
*   **問題**: 同步 GPIO 翻轉受批次配速（300 指令全速跑 + delayMicroseconds 補差）影響，翻轉在批次窗內被壓縮 → 週期抖動。
*   **設計**: 核心把翻轉的模擬週期推入 SPSC 環形緩衝（`AUDIO_RING` + `apple2_audio_peek/drop` FFI）；韌體用硬體 alarm 鏈在精確真實時刻重放。固定 4ms 延遲窗吸收批次抖動（模擬恆跑在真實時間之前）；「週期↔真實時間」錨點僅在大漂移（暫停/選單/SD/變速）時重校準。
*   **踩坑（重要）**: alarm pool 預設建在 Core 0，但 Core 0 在整個模擬批次期間 `spin_lock_blocking` 關中斷 ~1ms → ISR 被擋、堆積翻轉批次尾連發，聽感**比修正前更破**。**alarm pool 必須建在 Core 1**（`setup1()`），其關中斷窗口僅微秒級；跨核排程用 `alarm_pool_add_alarm_in_us` 是安全的。
*   **結果**: 實機試聽通過——頻率正確且音質清澈（修正前：清澈但偏低 13%；中間版：頻率對但破聲；最終：兩者皆正確）。

### 3. 效能審查（同日稍早，5bf14e1）
*   移除每指令 u64 `% / ÷`（M0+ 無除法器，軟體除法數百週期）→ 增量光柵計數器；`apple2_get_beam_y()` 改讀 atomic（兼修跨核 u64 撕裂）；RAM/ROM 熱路徑去除邊界檢查；fat LTO + codegen-units=1。主機端基準 +16%（323→375 emulated MHz），M0+ 收益更大。

## 2026-06-11: Goonies.dsk 終於載入成功（馬達慣性停轉 + 文字頁 2）

### 1. 根因診斷：主機端全系統開機模擬 (boot_test.rs)
*   **方法**: 新增 `apple2_core/src/boot_test.rs`，以與韌體完全相同的 FFI 流程在電腦上開機 DSK，輸出磁軌載入序列、PC 熱點直方圖、1/4 軌磁頭軌跡、文字畫面與 RAM dump，並可注入搖桿按鈕。
*   **發現**: 遊戲 loader（自製 RWTS，標記完全標準）每讀**一個磁區**就 `$C088`/`$C089` 關開馬達。舊核心立即停轉 → loader 的轉速偵測（連續讀 `$C08C` 比較）判定磁碟停止 → 每磁區罰等 ~1.5 秒起轉延遲 → 載入看似永遠完成不了。
*   **修正驗證**: e335cdf 的 1 秒慣性停轉延遲使關卡載入恢復正常速度，模擬中遊戲成功進入純 HIRES 畫面；MASTER.DSK 回歸正常。

### 2. 文字頁 2 渲染支援
*   **發現**: 遊戲開機後停在「HOLD JOYSTICK...」搖桿校正畫面，文字寫在**文字頁 2 ($0800)**；渲染端過去寫死 $0400，玩家只看到頁 1 的 loader 殘碼亂畫面，誤判為載入失敗。
*   **修正**: `get_text_row_addr()` 增加 page2 參數（TEXT 與 LORES 模式同步支援）。

### 3. 建置陷阱（重要教訓）
*   **陷阱 A**: 修正 commit 只存在 origin/main，本機 main 落後 → 本機重編的韌體不含修正。**重編前先 `git pull`**。
*   **陷阱 B**: Arduino precompiled library 機制優先連結 `Dropbox\Arduino\libraries\Apple2Core\src\cortex-m0plus\libapple2_core.a`（當時為 3/21 舊檔）→ 原始碼再新也連到舊核心。**新 `.a` 必須同步到該庫 `src/` 與 `src/cortex-m0plus/` 兩處**，並可在 `.bin` 中搜尋新版常數（如 0x000F9C18 LE）驗證。
*   **成果**: 實機燒錄後 goonies.dsk 開機、搖桿校正、關卡載入全數通過 ✅（專案開始以來首次）。

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

### 4. 變速模擬功能 (Speed Multiplier) 與 Bug 修復
*   **優化內容**: 
    *   引入 `g_speed_multipliers` 陣列，支援 x1.0, x1.2, x1.4, x1.5 四種速率。
    *   新增 `Fn + 5` (F5) 快捷鍵，可即時循環切換模擬速度。
    *   在畫面正下方中央顯示當前倍率提示 (重構了 `updateStatusLine()` 統一管理 UI)。
*   **關鍵 Bug 修復 (Troubleshooting)**:
    1.  **無效的變速 (Volatile Scope)**: 最初實作時 `g_speed_idx` 在 Core 1 被修改，但 Core 0 讀取時因缺乏 `volatile` 關鍵字導致暫存器快取未更新，速度無法切換。已將其宣告為 `volatile int g_speed_idx` 強制重新讀取主記憶體。
    2.  **WebSerial 漏攔截**: 網頁控制台的 `Apple2Keyboard.html` 漏掉了 F5 的 ANSI 序列轉換 (`[17~`)，導致網頁端按下 F5 無反應。已補上完整的捕捉邏輯。
    3.  **實體矩陣漏映射**: 在 `scan_matrix()` 中補齊了 `Fn + 5` (`k == '5' || k == '%'`) 的硬體按鍵判斷。
*   **成果**: 允許玩家在載入或特定遊戲情境下，透過實體鍵盤或網頁端無縫切換加速執行。

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
    *   **(勘誤 2026-06-11)**: 「`goonies.dsk` 啟動成功」為筆誤。經確認 `goonies.dsk` 自專案開始從未成功載入過；該映像檔在其他模擬器可正常遊玩，問題在本核心的位元組級磁碟模型。

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
