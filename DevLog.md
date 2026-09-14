# Pico Apple II Emulator - Development Log

## 2026-09-14: 底部說明列改為按鍵喚出 3 秒自動隱藏 + 網頁補 F7（實機驗證通過 ✅）

完整記錄見 `DevLog_Entry_2026-09-14.md`。摘要：

*   **版面**：狀態列 y=222 → y=210，下面 y=222 插入 F Key 速查列
    `F1:WRST F2:CRST F3:DISK F4:JOY F5:SPD F7:GRN`。字型 7px 寬，一列從 x=6 起算上限 **44 字**，
    這串剛好填滿 —— 所以不放 `FN+` 前綴，也**刻意不列 CapsLock**（鍵盤上有實體 CAPS 鍵可直接按，
    列出 `Fn+C` 反而誤導）。模擬畫面只到 y=191，兩列都在原本就空著的 48px 內。
*   **行為**：平時隱藏（畫面下方全黑）。按下**實體 Fn 的瞬間**、或**任一 F 鍵動作**時亮起，
    3 秒後自動收；期間再按會把倒數重算。實機是「按住 Fn 看一眼再按數字」，
    外接鍵盤沒有 Fn 就靠 F 鍵那條現身。`updateStatusLine()` 沿用原名改為「觸發」語意，
    繪製拆到 `paintStatusLines()`，所以既有的「狀態變動後重畫」呼叫點自動全變觸發點。
*   **⚠️ 自動隱藏的坑**：`statusLineTick()` 必須掛在 Core 1 的 VBLANK（緊接 `scan_matrix()`，
    SPI 此刻是靜的）；清除只擦 `drawRect(0,208,320,24,0)` 約 2ms，**不能用 `fillScreen`**（約 100ms）；
    **選單開啟時不擦** —— 選單下緣外框就在 y=228，擦了會打洞。
*   **網頁 F7**：內部 Fn+7 是就地翻旗標、不走 `g_f_key_event`，所以序列埠那條路原本沒有入口。
    新增 `g_f_key_event == 7`，入口 `ESC [18~`（真 ANSI F7 序列，原本沒用到），網頁 F7 送這串。
    **兩端都要更新** —— 舊韌體不認 `[18~` 會整串丟掉，症狀是 F7 全無反應而 F1~F3 正常。

### 待辦：外接鍵盤 F 鍵映射的兩項既存落差（本次未動）

1.  **F 鍵差一格**：韌體把 `[15~`（實體 **F5**）對到事件 4、`[17~`（實體 **F6**）對到 5。
    `Apple2Keyboard.html` 是照這個錯位寫的，所以**網頁端按 F4/F5 是對的**；
    但第三方序列終端（PuTTY 等）直連時，按 F4 沒反應、F5 切搖桿、F6 切速度，
    與畫面上的速查列對不起來。修法是韌體改成正規的 `[14~` / `[15~` 並同步網頁，**兩端得一起動**。
2.  **`'K'` 封包會漏字元**：網頁協定的 `'K'` 路徑只認 idx 112/113/114（F1~F3 的 JS keyCode），
    其餘一律 `pushKey(idx)`。若有客戶端送 keyCode 115（F4），會被當成 ASCII `'s'`
    憑空打出一個字母。之後在這條路加鍵時，順手把 **112~123 整段吃掉**比較安全。


## 2026-09-12: 新增 Fn+7 綠色監視器模式

Apple II 當年常配單色綠螢幕，補上這個顯示選項。純顯示層改動，模擬核心（Rust / Apple2Core）完全不知情。

*   **按鍵**：`Fn+7`。走 `Fn+C` 那條「就地翻旗標」的路，不佔用 `g_f_key_event`（該路徑是給 reset / 選單這類重動作與 300ms 去彈跳用的）。矩陣掃描本身已是邊緣觸發，不需額外防連發；Fn+7 原本就被吃掉不送鍵，不會漏出 '7' 給模擬器。
*   **狀態變數**：`g_mono_green`，預設彩色、不持久化。寫入與讀取同在 Core 1（`scan_matrix()` 就嵌在掃描線繪製中段），**不需要 spin lock**；最壞情況只是切換當幀上下半屏不同色，一幀就追上。
*   **三條渲染路徑**：
    *   TEXT / MIXED 下半：前景 `0xFFFF` → `MONO_FG`。
    *   HIRES：綠色模式直接「點亮即綠」，整段 artifact 上色（鄰位讀取 + 偶奇判斷）跳過 —— 這條路徑比彩色**更快**，不吃時序預算。
    *   LORES：查編譯期常數 `palette_green[16]`，把原色亮度映到四階綠。若只是全部塗同一個綠，整張 LORES 會糊成一片。
*   **顏色**：`MONO_FG = 0x37E6`（≈ #33FF33，P1 磷光偏亮綠）。純 `0x07E0` 太刺眼，且會和狀態列現用的 `0x07E0` 撞色。
*   **刻意不改**：磁碟選單與「LOADING MENU」是主機端 UI，不是監視器畫面，維持白/黃字。`fillScreen(palette[0])` 兩模式同為黑。


## 2026-09-10: SD 回壓改由 Core 1 背景分片執行（實機驗證通過 ✅）

### 1. 出發點：回寫時整台機器停住
原本 SD 相關的存取全部跑在 Core 0 的 `loop()` 裡，跟 `apple2_tick()` 同一條執行緒——SD 一動，6502 就完全停住。既有的「延遲設計」（馬達停轉才 flush、停轉 2.5 秒才回壓）只是把停頓**延後與合併**，並沒有讓它變成非阻塞。

三種停頓性質不同，處理方式也不同：

| 停頓 | 長度 | 客端是否在等結果 | 處置 |
| :--- | :--- | :--- | :--- |
| 馬達停轉 flush（4KB 寫） | ~10ms | 否 | 本次未動 |
| 換軌前 flush + 讀 4KB | ~10–30ms | 是（讀的部分） | 本質阻塞，不能背景化 |
| 整檔回壓（gzip 重壓 140KB） | **~425ms** | **否** | **本次目標** |

回壓是唯一秒級量級、且客端完全不在等結果的一段，收益最大、語意最單純，所以先做它。

### 2. 第一階段：mailbox（`f16168d`）
Core 0 投信、Core 1 執行。狀態機三態：

- `RP_IDLE` — Core 0 自由使用 SD
- `RP_PENDING` — 已投信、Core 1 尚未取件。Core 0 此時要用 SD 可**撤回**，不必等
- `RP_RUNNING` — Core 1 已開工

**互斥的關鍵不變式**：`IDLE→PENDING` 只有 Core 0 會做，`PENDING→RUNNING` 只有 Core 1 會做。因此只要 Core 0 進入 SD 臨界區前先呼叫 `sdClaimForCore0()` 把狀態壓回 `IDLE`，Core 1 就不可能在它使用 SD 期間開工。狀態轉換用 `res_lock` 保護，避免「Core 0 以為撤回成功、Core 1 同時開工」的雙頭馬車。

⚠️ **`setup1()` 本來就在 Core 1 上做 `SD.begin()` 和 `openLastDisk()`**，所以 SD 存取原本就分裂在兩核，只是靠「開機期 Core 0 還沒開始跑」的時序默契在保平安。這次改動反而是把它收攏成單一 owner。

⚠️ `g_archive_dirty` 改 `volatile`——現在兩核都會寫它（Core 0 撤回還原、Core 1 回壓失敗還原）。

**實機量測（決定性）**：回壓期間 Core 0 完全不受影響。
```
[SPD] 1012 kcyc/s batches=971 throttled=100% rp=2   <- 回壓執行中
[ARC] BG repack /castle_wolfenstein.dsk.gz: OK (425 ms)
[SPD] 1012 kcyc/s batches=968 throttled=100% rp=0   <- 回壓結束
```
`throttled=100%` 表示 Core 0 每一批都還在被 `delayMicroseconds` 主動壓慢，它有餘裕、沒在飆。若真有 2 倍速暴衝，那一秒會多出約 430 kcyc（1012→~1440），不可能看不出來。

### 3. 「畫面暴衝」的真正成因：渲染追幀，不是 CPU 加速
第一階段之後使用者回報「程式突然暴衝一小段」。實測排除了 CPU 加速（見上表），真正原因是 Core 1 被 `archive_compress` 佔住 425ms ≈ **25 個影格**：`next_draw_y` 凍結、6502 照跑，Core 1 回來後一口氣把積欠的掃描線補完，畫面直接跳到 425ms 後的現況。

⚠️ **這不是 SPI 匯流排衝突。** TFT 走 `spi0`、SD 走 `SPI1`，硬體上互不阻擋。而且那 425ms 絕大部分不是在等 SD——壓完約 50KB，用 20MHz SPI 寫出去大概 25ms，剩下約 400ms 是 uzlib 在 CPU 上算 LZ77 + Huffman。**若照「SPI 被佔住」的理解去修（例如把 SD 傳輸改 DMA），完全沒用**，省掉的只有那 25ms。要切割的是 CPU 時間。

### 4. 第二階段：分片 + 中止（`2fb95c4`）
- `disk_archive` 新增分片 API（`archive_compress_begin` / `_step` / `_abort`）。輸出格式與整檔版完全相同，只是把迴圈外提成狀態機，`File` handle 與緩衝跨呼叫存活。gz 每步壓一個 member，zip 每步搬 4KB。整檔版 `archive_compress` 保留給換片的同步路徑。
- **`GZ_CHUNK` 16K → 4K**。單靠分片不夠：16K member 單步約 47ms 仍是 3 個影格。4K 讓單步降到一個影格以內。代價是 member 變多（約 630 bytes 額外標頭）與字典窗口變小導致壓縮率略降；順帶好處是壓縮期間的常駐配置從約 57KB 降到約 26KB。multi-member gzip 自描述，舊的 16K 檔案照樣解得開。
- `serviceRepack()` 改成每次 `loop1()` 只做一步。

⚠️ **分片本身有反效果，必須配中止機制**：切片會把 `RP_RUNNING` 的牆鐘時間從 425ms 拉長到約 620ms（中間讓給渲染）。若 Core 0 仍得等整份跑完，第二階段反而比第一階段更糟。因為暫存檔要到最後一步才 `rename`、原始壓縮檔全程完好，**任一步中止都安全**，所以 `sdClaimForCore0()` 改成請 Core 1 在分片邊界收手，Core 0 最多等一步（~12ms）——比第一階段的「最多等 425ms」還好。

⚠️ **清 abort 與轉 IDLE 必須在同一個 `res_lock` 內**。否則 Core 1 剛清掉 abort、還沒轉 IDLE 時 Core 0 進來設 abort，旗標會殘留到下一份工作，害它一開工就被誤中止。

**實機驗證**：`OK (624 ms, 37 steps)` / `(657 ms, 37 steps)` / `(603 ms, 37 steps)`。37 步符合預期（140KB ÷ 4KB = 35 個 member + 收尾）。使用者主觀回報：**長時間停頓與畫面跳都消失，變成細碎頓挫，影響已經不大** ✅。

注意 `624 ÷ 37 = 16.9ms` **不是單步凍結時間**——牆鐘時間包含中間讓給渲染的部分。實際單步凍結未直接量測。

### 5. 板載 LED 當 SD 寫入指示燈（`1bffd75`）
`PIN_SD_LED = 25`。指示「寫入」而非所有存取：讀軌本來就有畫面右側的磁軌條可看，寫回才是無從確認的那一半。

用 latch 而非跟著存取亮滅——單次 flush 只有 ~10ms，直接跟會短到看不見，所以點亮後至少維持 60ms。回壓每 ~12ms 一個分片、每步補一次脈衝，於是整段維持恆亮，兩種寫入自然區分：

| 燈號 | 意義 |
| :--- | :--- |
| 閃一下（~60ms） | 換軌把 dirty track 寫回 `_WORK.DSK` |
| 恆亮 ~0.7 秒 | 整檔回壓到 `.gz` / `.zip` |

⚠️ `sdLedService()` 必須放在 `loop()` **所有 early return 之前**，否則進選單/暫停時燈會卡在亮著。

⚠️ 成本可忽略是因為 `rpipico` 的 LED 是 GPIO25 直驅。**若日後換 Pico W 必須改寫**——那顆 LED 掛在 CYW43 上，每次切換要走 SPI 到無線晶片。

同時移除 `[SPD]` 每秒實速監看（任務完成）；`[ARC]` 保留。

### 6. 拿掉回壓防抖（`6c779ff`）
`ARCHIVE_REPACK_DELAY_MS` 2500 → 0。防抖當初的理由是「回壓會凍住整台機器 0.4 秒，觸發越少越好」，改成背景分片後這個理由消失。

**寫回其實有三層延遲，只有第三層是人為的：**

| 層 | 延遲 | 來源 | 可否改 |
| :--- | :--- | :--- | :--- |
| 1. 磁區寫入 → work dsk | 到換軌或馬達停轉才落地 | 軌為單位的 nibble 引擎 | 不該（改成每磁區落地會讓一次 SAVE 對同一軌重複寫 4KB 十幾次） |
| 2. 馬達停轉延遲 | **1 秒**（`MOTOR_OFF_DELAY_CYCLES`） | **真實 Disk II 硬體行為** | **不可改**（`disk2.rs` 有專門測試驗證，且軟體依賴此時序） |
| 3. 回壓防抖 | 2500ms → **0** | 軟體防抖 | 已拿掉 |

所以最壞情況仍是寫入後約 1 秒進 work dsk、約 1.7 秒進 `.gz`。**那 1 秒是硬體語意，不是我們的延遲。**

**實機驗證後的意外發現**：拿掉防抖後並未出現空轉。連續磁碟活動期間馬達根本不會停轉，而 `postRepackIfDirty()` 的觸發條件是 `!motor_on`，所以**合併是馬達狀態天然做掉的**：
```
Flush Track 26 / 27 / 25 / 27 / 26 / 27 / 25 / 27   <- 8 次換軌寫入
[ARC] BG repack: OK (676 ms, 37 steps)              <- 活動結束後才回壓 1 次
```
50 秒內 5 次回壓、**零 `ABORTED`**。換言之那個 2.5 秒防抖從一開始就大部分是多餘的，真正在做合併的是第 2 層的 1 秒硬體延遲。

### 7. 尚未驗證 / 待辦
- **`ABORTED` 路徑從未被實測走過**。中止機制的正確性目前只有邏輯論證（暫存檔未 rename、dirty 保留、清旗標與轉 IDLE 同鎖），不是已驗證的程式碼。要壓測可在 LED 恆亮的那 0.7 秒內按 F3 進選單（選單路徑會呼叫 `sdClaimForCore0()`）。
- **`GZ_CHUNK` 16K→4K 的壓縮率代價沒有數字**。需把 SD 插到電腦上比對 `.gz` 檔案大小。
- **單步凍結時間未直接量測**（見 §4）。目前靠主觀回報判定達標。
- 拿掉防抖後回壓耗時從 603–657ms 升到 672–739ms（約 +10%），`steps` 不變表示工作量沒變，推測是與 Core 0 的 flush 在 SD 匯流排上交疊。未追查。
- **第三階段（未做）**：把換軌 flush 也搬進信箱。那是高頻停頓（每次換軌 ~10–30ms），但讀軌本質上是同步的（客端在等資料），只有寫的部分能背景化。

## 2026-08-27: 模擬畫面上移貼齊 y=0 + build 路徑鏈再度斷裂（實機驗證通過 ✅）

### 1. 版面：模擬畫面從 y=24 移到 y=0（實機燒錄驗證通過 ✅）
*   **改動**：`loop1()` 的掃描線送窗由 `startFrame(20, 24 + y, 299, 24 + y)` 改為 `startFrame(20, y, 299, y)`。x 維持 20（280px 仍左右置中），畫面範圍變成 **x 20..299 / y 0..191**，整塊貼齊面板頂端。
*   **狀態列不動**：仍在 y=222（`updateStatusLine`），畫面底 191 與狀態列之間留下 192..221 的空白帶。
*   **連帶調整（必要，不是順手改）**：
    *   磁軌指示條跟著對齊 → `drawRect(305, 0, 8, 192, ...)`，滑塊 `handle_y = track * 184 / 34`，上限由 208 改為 **184**（184+8=192 剛好貼齊畫面底）。
    *   **磁碟馬達 LED 從 y=10 搬到 y=196**。原位置 (305,10) 現在被上移後的磁軌條佔走，兩者會疊在一起；196..203 落在畫面底與狀態列之間的空白帶，兩邊都不碰。
*   選單畫面用自己的全螢幕座標系（外框 10..309 / 10..229），不受此次改動影響。

### 2. build 環境：sketchbook 從 Dropbox 搬到 Google Drive，路徑鏈再度斷裂（已修 ✅）
*   **症狀**：`full_build.bat` 報 `Apple2Core.h: No such file or directory`。與 `make_arduino_lib.py` docstring 裡記錄的斷裂 #1 完全同一種。
*   **根因**：使用者已棄用 Dropbox，內容搬到 `G:\我的雲端硬碟\dropbox_dahai_pon`，但 `~/.arduinoIDE/arduino-cli.yaml` 的 `user:` 仍指著 `c:\Users\Dell\Dropbox\Arduino`（整個路徑已不存在）。`scan_env.ps1` 照抄那個值 → `--libraries` 指向空氣。G: 上那份庫的 `.a` 也停在 2026-06-20，就算改指過去也會踩「靜默連結舊核心」的老坑。
*   **修正 1 — `full_build.bat` 不再依賴 sketchbook**：步驟 2 改呼叫 `loader_offset/make_arduino_lib.py` 就地生成 Apple2Core 程式庫（`.h` 來自 repo 根目錄＝單一事實來源、`.a` 來自剛編好的產出），步驟 3 的 `--libraries` 只指這份、並加 `--clean`。原本的 direct-link（`-L src -lapple2_core` 走 `extra_flags`）拿掉——precompiled library 機制已經負責把 archive 放進 link 指令的正確位置。**這正是 `build_offset.bat` 早就在用的做法，兩支腳本現在一致了。**
*   **修正 2 — 產出移進 `build/`**：`--output-dir` 由 `.` 改為 `%PROJECT_ROOT%build`（`.gitignore` 已含 `build/`），步驟 4 的 `picotool load -x` 路徑同步更新。repo 根目錄不再散落 `.uf2/.elf/.bin/.map`。
*   **修正 3 — 生成庫放 `build\arduino_libs`，不要放 `build_offset\arduino_libs`**：後者是 offset 版的輸出目錄，兩支腳本共用會互相覆蓋。
*   **修正 4 — `scan_env.ps1` 路徑不存在時出聲**：改成 `Test-Path` 後 `Write-Warning`，不再沉默地產出死路徑。**注意該檔必須維持純 ASCII** —— Windows PowerShell 5.1 以系統 codepage(cp950) 讀取無 BOM 的 `.ps1`，中文會被解成亂碼而觸發 parser error（我加中文註解時當場踩到）。同理 `.bat` 的註解也維持 ASCII，理由見 `build_offset.bat` 抬頭。
*   **兩種產出別搞混**：`full_build.bat` link 在 `0x10000000`，產出 `build\PicoApple2.ino.uf2`，直接燒錄用；**rp2040-retro-loader 用的是 `build_offset.bat`**（link 在 `0x10004000`，前 16KB 留給 loader/trampoline），產出 `build_offset\PicoApple2_standalone.uf2`。
*   **驗證**：完整跑過修改後的步驟 1–3（只截掉上傳）——Rust exit 0、Arduino `--clean` exit 0，Flash **177980 bytes (8%)**、RAM **103072 bytes (39%)**，`build\PicoApple2.ino.uf2` 393216 bytes。後續已燒錄實機並確認畫面正常 ✅。

### 3. 兩個版本都重編（皆編譯通過 ✅；已燒錄實機測試成功 ✅）

| 腳本 | link 位址 | 產出 | Flash |
| :--- | :--- | :--- | :--- |
| `full_build.bat` | `0x10000000` | `build\PicoApple2.ino.uf2` (393,216 B) | **177,980** B (8%) |
| `build_offset.bat` | `0x10004000` | `build_offset\PicoApple2_standalone.uf2` (401,408 B) | **177,724** B (8%) |

*   offset 版 7 步全過，其中步驟 6 的 flash 佈局檢查：`image 0x10004000..0x10031000 (184320 bytes)`、`向量表 SP=0x20042000 Reset=0x100040e3`，`app_present()` 條件都通過。步驟 7 合併跳板：`784 blocks (跳板 12 + body 720 + 填充 52)`。
*   `build_offset\PicoApple2.ino.uf2`（368,640 B）是**只有 body、前 16KB 空的中間產物，不能單獨燒**。要用的是 `PicoApple2_standalone.uf2`：丟 SD 卡根目錄給 loader，或直接 `picotool load -v -x`。
*   兩版 Flash 差 256 bytes 純粹是 linker script 不同（offset 版丟掉 `.boot2`/`.ota`/`.partition`），RAM 兩版相同 103,072 B (39%)。

*   **實機結果：兩個版本都燒錄測試成功 ✅**。`build\PicoApple2.ino.uf2`（直燒版）與 `build_offset\PicoApple2_standalone.uf2`（rp2040-retro-loader 版）各自上機驗證，畫面都正常：模擬區貼齊面板頂端、狀態列留在 y=222，磁軌條與馬達 LED 未互相覆蓋。新的 build 流程（生成式 Apple2Core 程式庫 + 產出進 `build/`）兩條路線同時成立，offset 版的跳板合併與 flash 佈局也經實機確認。

### 4. 收尾：把 `arduino-cli.yaml` 的 sketchbook 路徑指到 G:（順帶挖出 build_env.bat 的編碼坑）

*   **改動**：`~/.arduinoIDE/arduino-cli.yaml` 的 `directories.user` 由 `c:\Users\Dell\Dropbox\Arduino` 改為 `G:\我的雲端硬碟\dropbox_dahai_pon\Arduino`（改前已備份為 `arduino-cli.yaml.bak-20260827`；Arduino IDE 當時未執行，否則它結束時會把設定寫回去覆蓋）。PicoApple2 本身已不需要這個路徑，但其他 sketch 需要。
*   **改完立刻冒出第二個坑**：`scan_env.ps1` 是用 `Out-File -Encoding ascii` 產生 `build_env.bat` 的。新路徑含中文，ascii 把每個中文字轉成 `?`，於是 `ARDUINO_USER_LIB_PATH=G:\??????\dropbox_dahai_pon\...` —— **又是一條沉默的死路徑**，跟這篇第 2 節修的是同一種病。
*   **為什麼不能用 8.3 短檔名繞開**：試過了，Google Drive 的虛擬檔案系統不產生短檔名（`ShortPath` 原樣回傳含中文的長路徑），所以那條路徑沒有任何純 ASCII 的寫法。
*   **修正**：`Out-File -Encoding ascii` → **`-Encoding oem`**。`cmd.exe` 是用主控台 codepage 逐行解析 `.bat` 的，oem 正好對應系統預設（本機 cp950）。實測在 cp950 主控台下 `if exist "%ARDUINO_USER_LIB_PATH%\Apple2Core"` 回 YES。
*   **已知限制（寫在 `scan_env.ps1` 註解裡）**：這假設呼叫端的主控台在系統預設 codepage。被強制 `chcp 65001` 的主控台（`build_offset.bat` 就會）會解錯這一行 —— 目前無害，因為兩支 build 腳本都不再讀這個變數；仍在讀它的是 `build_rust.bat` / `precompile_sd.bat` / `test_sd.bat` / `scripts/_compile_only.bat`。
*   順手移除 `full_build.bat` 裡已成死碼的 `set "CUSTOM_LIB_PATH=%ARDUINO_USER_LIB_PATH%"`（步驟 3 改用 `GEN_LIB_DIR` 後就沒人用了）。
*   **驗證**：在 cp950 主控台重跑修改後的步驟 1–3 —— `[OK] Libraries -> G:\我的雲端硬碟\...`（警告消失＝路徑真的解析得到）、Rust exit 0、Arduino `--clean` exit 0、Flash 177,980 B (8%)、`build\PicoApple2.ino.uf2` 393,216 B。

## 2026-07-01: 部分遊戲搖桿「右/下」失效——滿舵脈衝太短，補上飽和區（實機驗證通過 ✅）

### 症狀
*   某些遊戲搖桿的**右**與**下**沒作用，左/上正常；切換韌體 JOYSTICK/KEYBOARD 模式都一樣；其他遊戲與 F3 選單方向都正常。代表案例：**Championship Lode Runner (CLR)**。

### 診斷（實測，非推理——中途一次錯誤猜測的教訓）
*   **先排除的兩條路（都用 host 端實測坐實）**：①paddle 讀取時序正確——用真實 6502 迴圈量測，emulator 對 paddle 值 0~255 全部原封讀回，無溢位；②輸入映射左右/上下對稱、實體輸入確實有到（選單/他遊戲正常）。第一次「255→256 溢位」的猜測**是錯的**，已完整還原、不要再走那條路。**問題與輸入值無關，在讀取端脈衝長度。**
*   **關鍵**：切換韌體搖桿模式對「讀類比搖桿的遊戲」根本沒換路徑——`apple2_set_paddle` 吃的是實體按鍵狀態、兩模式共用，所以兩模式一起壞是必然，不能拿來證明遊戲壞掉。
*   **抓真凶**：boot_test 開機 CLR → `BOOT_DUMP` dump 主 RAM → grep 特徵碼 `AD 70 C0`/`AD 64 C0`/`AD 65 C0` 找到讀桿常式 `$8B80` → 反組譯：它每圈 **54 cycle**（逐指令核對，週期正確）把脈衝寬度數到 `$65`(X)/`$66`(Y)，門檻 `CMP #$37`=55：計數 <15 或 ≥55 才算有效方向。
*   **把該常式原始位元組塞進真實 CPU+Apple2Memory 直接跑**，量各方向計數：左=0、上=1（過關）；**右=52、下=52 → 差門檻 55 臨門一腳 → 失效**；置中=26/27（中立）。

### 根因與修正
*   滿舵要數到 ≥55 需脈衝 ≥55×54=**2970 cycle**，但 memory.rs 舊式線性 `8+value*11` 在 v=255 只給 **2813 cycle**（→52 圈）。真實 Apple II 滿舵脈衝約 **3300+ cycle**（進入 PREAD 量程外「飽和區」；PREAD 含 DEY 照樣讀 255），模擬器把滿舵截太短。
*   **修正**（memory.rs `0xC064..=0xC067` 讀取分支）：`if el < (8 + v*11 + v.saturating_sub(192)*6)`。只把 v>192 高值段補進飽和區，**v≤192 與置中 128 byte-for-byte 不變**，不影響其他比例式讀桿遊戲。修後 CLR 右/下數到 **59**（過門檻）、置中仍中立。
*   **迴歸測試**：boot_test.rs `clr_joystick_all_four_directions_register`（用 CLR 真實常式斷言四方向+置中）。並加兩個 host 除錯旋鈕：`BOOT_TEXT_EVERY`（每N秒 dump 文字畫面翻多頁）、`BOOT_TYPE_GAP`（按鍵間隔慢速翻頁）。備忘：`memory_test.rs`/`cpu_test.rs` 未被 lib.rs `mod` 進去＝死檔不編譯，加測試要放 `boot_test.rs`。
*   **成果**：主機端測試 7 passed、thumbv6m target 編譯 exit 0；重跑 full_build.bat 燒錄後實機四方向恢復 ✅。

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
| **Fn + 4** | Joy/Key (F4) | 切換方向鍵為 搖桿模式 / 鍵盤模式。 |
| **Fn + 5** | Speed (F5) | 循環切換模擬速度 (x1.0 / x1.2 / x1.4 / x1.5)。 |
| **Fn + 7** | Color/Green (F7) | 切換 彩色 / 綠色監視器。 |
| **Fn + C** | Caps Lock | 切換大小寫鎖定（預設為 ON）。實體 CAPS 鍵亦可，兩者等效。 |

> 按下 Fn（或任一 F 鍵）會把螢幕底部的狀態列 + F Key 速查列叫出來，3 秒後自動隱藏。
> 詳見 `DevLog_Entry_2026-09-14.md`。

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
