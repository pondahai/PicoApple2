# 🍎 PicoApple2 DevLog - 2026-06-23

## 🎲 Floating Bus(浮空匯流排)實作:補回硬體亂數源

### 1. 背景
Apple II 沒有亂數產生硬體。許多遊戲(如《德軍總部 / Castle Wolfenstein》)的亂數來源是 **floating bus**:讀取一個「沒有任何晶片驅動資料匯流排」的 `$C0xx` 位址時,CPU 會讀到**影像電路在同一個 cycle 前半從 RAM 抓取的那個畫面位元組**(共用資料匯流排 + 殘留電荷的物理副作用)。其值隨光束位置變動,對程式而言就是免費的亂數。

本核心先前對所有未驅動的 `$C0xx` 讀取一律回傳常數 `0`,等於亂數恆為 `0`。在 apple2emu(姊妹桌面核心)中已確認這會造成《德軍總部》:
- 爆炸聲(白雜訊)塌成單音 —— 雜訊靠「亂數打亂喇叭 toggle 間隔」產生,亂數恆定 → 規律 toggle → 單音。
- 防彈背心失效 —— 中彈/穿透判定的擲骰永遠落同一邊。

本輪把同一修正移植到 PicoApple2 核心。

### 2. 修正內容(`apple2_core/src/memory.rs`)
- 新增 `video_scanner_address(cycle)`:移植自 AppleWin `VideoGetScannerAddress`(Jim Sather《Understanding the Apple IIe》第 5 章模型)。NTSC 65 horizontal clocks/掃描線 × 262 線;依目前 text/hires/page2/mixed 狀態,用 H/V counter 位元組合算出影像掃描器此刻指向的 RAM 位址(含 HBL/VBL 期間)。
- 新增 `floating_bus()`:由 `cpu_step_cycle_base + cpu_step_cycle_cursor` 取得目前 cycle,回傳 `RAM_48K[掃描位址]`(全域 static RAM;含邊界保險檢查)。
- 將原本回傳寫死 `0` 的未驅動 I/O 全部改走 floating bus:`$C050`–`$C057`(影像開關)、`$C030`(喇叭,讀取同時 click 並回傳浮空值——雜訊程式正是在此計時迴圈取亂數)、`$C080`–`$C08F`(LC 開關)、萬用 fallback `_`。
- **所有原本的副作用(切顯示模式 / toggle 喇叭 / LC bank 與 write-enable 解鎖)全部保留,只把回傳值由 `0` 改為 floating bus。**

### 3. 觀念釐清
- **回傳值**可用單一 `floating_bus()` 統一處理,不必逐位址對應。
- 但 floating bus 值本身**必須**算掃描器位址——它是特定畫面 RAM 那一格,不是隨機數;有些 II+ 程式靠它的 bit pattern 偵測垂直消隱(VBL),回錯值會壞掉。
- soft switch 的**副作用仍須逐位址解碼**,不能用 floating 取代。
- 記憶體圖中只有 `$C000–$CFFF` I/O 區會 floating;`$0000–$BFFF` 全 RAM、`$D000–$FFFF` 全 ROM/LC,皆有驅動。

### 4. 驗證
- 新增測試模組 `apple2_core/src/floating_bus_test.rs`(並於 `lib.rs` 以 `#[cfg(test)] mod floating_bus_test;` 註冊)。
  - 註:本核心的 `memory_test.rs` / `cpu_test.rs` 等檔**未被 `lib.rs` 註冊**,且已與現行 API 脫節(例如 `mem.rom` 現為 `&'static [u8]` 不可寫)無法編譯,故 floating bus 測試獨立放在會被編譯執行的新模組。
  - 測試 `undriven_io_reads_track_the_floating_bus`:把 RAM 填上位址低位元組標記,連讀 `$C055` 200 次,確認回傳值**確實隨 cycle 變動且取自 RAM**(常數 stub 會令其失敗)。
- `cargo test`:**6 passed / 6 ignored**(新測試通過;6 個 ignored 為既有硬體開機測試)。
- `cargo build --release --target thumbv6m-none-eabi`:嵌入式韌體 static lib **編譯通過**。

### 5. 參考
- AppleWin `Video.cpp` → `VideoGetScannerAddress`(位址公式來源)
- Jim Sather, *Understanding the Apple IIe*, 第 5 章
- 姊妹核心 apple2emu 的詳細原理筆記 `FLOATING_BUS.md`
