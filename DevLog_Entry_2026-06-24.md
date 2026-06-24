# 🍎 PicoApple2 DevLog - 2026-06-24

## 🚀 上電延遲改為「有上限輪詢」+ 燒錄/編譯流程備忘(實機燒錄通過 ✅)

### 1. 背景:兩個 `delay(2000)` 的盲等
`commit 19c9a51`(synchronize boot timing)為了讓兩核開機時序穩定,加了兩段固定盲等:
- **Core 0 `setup()`**: `Serial.begin(115200); delay(2000);` —— 等 USB CDC 列舉。
- **Core 1 `setup1()`**: 開頭 `delay(2000);` —— 等 Core 0 序列埠/`apple2_init()` 穩定再操作顯示器/SD。

`g_boot_ready` 閘門(Core 0 `loop()` 的 `if(!g_boot_ready) return;`)**本來就是事件驅動**,不在改動範圍。問題只在那兩個盲等,合計最差 ~4 秒。

### 2. 修正:bounded poll 取代盲等(`PicoApple2.ino`)
- 新增 `volatile bool g_core0_ready`。Core 0 在 `apple2_init()` + `spin_unlock` 後設 true。
- **Core 0 serial**: `while (!Serial && millis()-t0 < 2000) tight_loop_contents();` —— 接電腦時序列埠一好就走;獨立開機(無 host)最多 2 秒上限放行,**不會卡死**。
- **Core 1**: `while (!g_core0_ready && millis()-t0 < 2000) tight_loop_contents();` —— Core 0 一就緒(通常數 ms)就放行。
- **設計重點**: 兩段都是「輪詢 + 2 秒上限」,**最壞情況等同舊的 `delay(2000)`,零回歸風險**;正常情況省下大半等待。
- ⚠️ 陷阱:serial 那段**絕不能**寫成無上限的 `while(!Serial)` —— 本機會獨立開機(不接 USB),沒上限會卡死在開機。

### 3. 🛠️ 燒錄 / 編譯驗證流程備忘(下次照做,不要再 try)

**環境變數**:`build_env.bat` 已是 auto-generated 但**持久**,直接讀用,不必重跑 `scan_env.ps1`(且 `-ExecutionPolicy Bypass` 會被 Claude Code 安全策略擋下):
```
ARDUINO_CLI = C:\Program Files\Arduino IDE\resources\app\lib\backend\resources\arduino-cli.exe
PICOTOOL    = C:\Users\pondahai\AppData\Local\Arduino15\packages\rp2040\tools\pqt-picotool\4.1.0-1aec55e\picotool.exe
ARDUINO_LIB = c:\Users\pondahai\Dropbox\Arduino\libraries
FQBN        = rp2040:rp2040:rpipico
```

**只編譯驗證(不上傳、不開終端、不卡 pause)** —— `full_build.bat` 會上傳+`pause` 會卡住,別整支跑。手動三步:
1. `cd apple2_core && cargo build --target thumbv6m-none-eabi --release`(約 14s;只剩既有 4 個 warning 屬正常)
2. `copy target\thumbv6m-none-eabi\release\libapple2_core.a → src\libapple2_core.a`
3. `arduino-cli compile --fqbn rp2040:rp2040:rpipico --libraries <LIB> --build-property "compiler.c.elf.extra_flags=\"-L<ROOT>\src\" -lapple2_core" --output-dir . PicoApple2.ino`
   - 產出 `PicoApple2.ino.elf`。`ld.exe` 的 `.note.GNU-stack` warning 是工具鏈既有訊息,可忽略。

**燒錄(picotool load)** —— Pico 平時在 serial 模式(本機是 **COM14**,`arduino-cli board list` 抓 rp2040 那行的 COM),需先進 BOOTSEL:
- `picotool reboot -f -u` 偶爾回 exit 255/輸出被吞 **但其實有生效**;判斷依據是隨後 **該 COM port 消失** 且 `picotool info` 回 exit 0(顯示 `Program Information: none`)= 已在 BOOTSEL。
- 備援:對該 COM 開 **1200bps** 再關也能觸發 BOOTSEL(`New-Object System.IO.Ports.SerialPort 'COM14',1200; Open; Close`)。開到「指定的裝置不存在」其實代表已重列舉進 BOOTSEL。
- 進 BOOTSEL 後:`picotool load -x PicoApple2.ino.elf`(`-x` = 載入後直接執行)。100% + "The device was rebooted to start the application" = 成功。
- 燒錄前先殺掉佔用序列埠的監控(`serial_monitor.ps1` 進程 / "Pico Terminal" 視窗),否則埠被佔。

### 4. 驗證
- 編譯:Rust + Arduino 皆 exit 0。Flash 168804 B(8%)、RAM 101404 B(38%)。
- 燒錄:`picotool load` 100% 完成並自動重啟運行 ✅。

---

## 📦 壓縮磁碟支援:gz / zip 讀取 + 寫回

### 1. 背景與目標
先前 PicoApple2 只認 `.dsk`(35 軌 × 4096 = 140KB 固定格式)。磁碟引擎的核心是
**單軌隨機讀寫**:換軌時 `seek(track*4096)` 只讀 1 軌進 `track_buffer`,寫回也是
就地覆蓋該軌(`flushDirtyTrack`)。本輪目標:支援 `.gz` / `.zip`,且**讀取與寫回**
都要有。`.rar` 評估後放棄(無自由的 RAR 壓縮器、解壓庫太大,RP2040 不划算)。

### 2. 為什麼不能直接在壓縮檔上跑引擎
deflate(gz/zip 共用)是**連續資料流,不能隨機 seek 單軌**;且 140KB 影像 + 解壓
視窗超過剩餘 RAM,無法整檔進記憶體。因此採:

> **解壓到 SD 上的暫存工作檔 `/_WORK.DSK` → 既有單軌引擎不動,直接對工作檔讀寫
> → 換片時把工作檔回壓覆蓋原壓縮檔。**

引擎一行未改,只在「載入」與「換片/回壓」兩端各包一層。

### 3. Codec(`disk_archive.cpp` + vendored `src/uzlib/`)
uzlib 取自 rp2040 core 的 OTA 模組(zlib 授權)。

- **解壓(讀)**:streaming inflate,32KB `dict_ring` 滑動視窗 + 2KB flush 緩衝,
  140KB 輸出**從不全進 RAM**。gz 走 `uzlib_gzip_parse_header` + 多成員迴圈;
  zip 由**中央目錄**(權威,可處理 data descriptor)定位首個項目,method 8 raw
  inflate、method 0 直接複製。
- **gz 寫回**:**多成員 gzip**(每 16KB 一個 member)。uzlib 壓縮器需整段輸入駐留
  RAM,故切 16KB chunk 餵入(視窗 16KB),member 串接 = 標準多成員 gz,任何工具
  可解。RAM 峰值 ~80KB(chunk 16KB + hash 16KB + outbuf ~24KB)。
- **zip 寫回**:**stored(method 0)**。單一 deflate 流無法在 RP2040 RAM 內 streaming
  壓縮 140KB,故 v1 不壓縮(檔案不變小但完全合法)。壓縮 zip 列為後續(需 streaming
  deflate,如 windowBits 受限的 zlib)。
- 回壓寫到 `/_REPACK.TMP` 再 `SD.rename` 覆蓋原檔,中途失敗/斷電不毀原檔。

### 4. 整合(`PicoApple2.ino`)
- `archive_kind()` 依副檔名分類;`scanDiskFiles` 接受 `.dsk/.gz/.zip`,排除
  `_WORK.DSK` / `_REPACK.TMP`。
- `openDiskByPath()`:純 dsk 直接開;壓縮檔先 `archive_extract` 到 WORK 再開,記住
  `g_archive_src` / `g_archive_kind`。
- `flushDirtyTrack()` 寫成功且來源是壓縮檔 → 設 `g_archive_dirty`。
- `repackArchiveIfDirty()`:換片前呼叫,把 WORK 回壓覆蓋原壓縮檔。
- `LASTDISK.TXT` 存**原始路徑**(壓縮檔記原檔,非 WORK),開機重新解壓。
- 選單顯示用原檔名,不外洩 `/_WORK.DSK`。

### 5. 驗證
- **Codec 邏輯(host 實跑)**:`scripts/test_archive.c` 用 MSVC 編譯,完全鏡像
  `disk_archive.cpp` 驅動邏輯(File→FILE*),真資料 round-trip 並用系統
  `gzip -t` / `unzip -t` 交叉驗證。**全數通過**。
  - 過程抓到一個真 bug:多成員解壓時,每個 member 用全新 `uzlib_uncomp` 並把
    `source=NULL`,會**丟掉已緩衝在 `s_src.buf` 的下一 member 起始位元組**。改成
    重用同一 `d`、每 member 只呼叫 `uzlib_uncompress_init`(重設解碼狀態但保留
    source 指標)。單成員不觸發故先前看似正常 —— ARM 編譯檢查抓不到這種執行期
    bug,host 實跑才抓得到。
- **韌體**:`scripts/_compile_only.bat`(arduino-cli,沿用 `build_env.bat`/預編譯
  `libapple2_core.a`)→ `COMPILE_EXIT=0`。Flash 8%、靜態 RAM 38%;codec ~80KB
  working buffer 走 heap,落在保留的 ~160KB 內。
- **實機驗證(2026-06-24 已過)**:`picotool load -x` 燒入後,序列埠看到
  `[ARC] Decompressing /Outpost.zip -> /_WORK.DSK`,隨後 6502 PC 進入 RAM `$0B3x`
  執行(C1_T 數百萬 cycle、暫存器持續變動)→ **zip 解壓→載入→開機進遊戲全程成功**。
  注意:用 PowerShell SerialPort 讀取時需 `DtrEnable=$true` 才會觸發板子重啟並吐出
  開機 log(否則 CDC 不送早期輸出)。
- **尚待實機驗證**:(a) `.gz` 讀取(目前 SD 上只有 zip);(b) DOS `SAVE` 後 F3 換片
  觸發回壓,取出 SD 確認原 `.gz`/`.zip` 已更新且可被 PC 解開。

## 📜 F3 選單分頁/捲動

### 動機
舊選單 `disk_files[20]` 寫死、`drawDiskMenu` 從 index 0 一路畫,清單從 y=65 到框底
y=228 每列 12px **只容 ~13 列**;超過就畫到框外、選到也看不到高亮。

### 修正(`PicoApple2.ino`)— 無檔案數上限(按需讀目錄)
最初做成 `disk_files[64]` 固定陣列,但那是「總數」硬上限(第 65 筆以後直接丟棄)。
改為**不在 RAM 全存清單**,只快取「目前可視頁」:

- 核心分工前提:**SD 存取全在 core0(`loop()`)**,core1(`loop1()`)只繪 TFT。故目錄
  讀取必須在 core0,結果經 RAM 視窗 + 旗標交握交給 core1 繪製,維持 SD 單核存取不變。
- core0:`scanDiskFiles()` 只**數總數**(`disk_file_count`,無上限);`fillDiskWindow(base)`
  把從 base 起最多 13 筆檔名讀進 `disk_window[13]`;`findDiskNameByIndex(n)` 載入時依
  index 掃目錄取檔名。三者共用 `isMenuDiskEntry()` 過濾。
- core1↔core0 交握:`req_menu_fill`(請填某頁)/ `menu_fill_done`(已就緒)/ `disk_window_base`
  (視窗對應起始 index)。換頁才請 core0 重填;同頁移動高亮直接重畫。
- `clampMenuScroll()` 依 `selected_file_idx` 調整 `menu_scroll`(含上下繞回跳頁)。
- `drawDiskMenu()` 從 `disk_window[]` 畫,右上頁碼 `(sel/total)`,上/下還有項目時右緣
  畫 `^` / `V`(Apple II 字元集 $5E/$56 皆有)。
- RAM 幾乎不增(13 個 String);此「掃哪個目錄」結構也是日後資料夾瀏覽的基礎。
- 實機:編譯過、`picotool load -x` 燒入、Outpost(zip)仍正常載入開機;選單捲動/頁碼
  視覺行為待在 TFT 上以 F3 + 上下鍵確認。
- 燒錄注意:本版韌體未對外露出 picotool reset interface,`reboot -f -u` 會回報
  "Unable to locate reset interface",須改用 **1200bps touch**(full_build 既有作法)進
  BOOTSEL 再 `load`。

## ⌨️ ALT 組合鍵:RIGHT=ENTER、DOWN=SPACE

延續既有 `BTN_ALT`(GPIO 28)組合鍵機制(原本 ALT+A=循環速度、ALT+B=切換方向/搖桿
模式),新增兩個常用送鍵:

- **ALT + RIGHT(GPIO 6)→ ENTER(0x0D)**
- **ALT + DOWN(GPIO 5)→ SPACE(0x20)**

實作(`scan_matrix()`,跑在 core1):
- 對 `raw_right`/`raw_down` 加**邊緣偵測**(記錄抑制前原始值),按一下送一次,不連發。
- ALT 按住時,RIGHT/DOWN 一併**抑制**(`raw_right=raw_down=false`),不會同時觸發搖桿/
  方向移動;與既有 A/B 抑制一致。
- 用 `pushHardwareKey()`(與實體矩陣鍵盤同路徑,`res_lock` 保護)。
- `!g_show_menu` 守衛:選單中不送,避免關閉選單後殘留按鍵跑進 Apple II。
- 實機:編譯/燒錄/開機皆過。

## 已知限制 (壓縮磁碟 v1)
- **回壓只在「F3 換片」時觸發**(含重選同一片)。純斷電不會回壓 → 改動留在 WORK,
  原壓縮檔不更新(WORK 不刪,資料不失)。
- **zip 寫回不壓縮**(stored)。
- zip 讀取取首個項目(多檔 zip 以第一個為準)。
- 輔助腳本:`scripts/archive_proto.py`(framing 原型驗證)、`scripts/test_archive.c`
  (codec host 測試)、`scripts/_build_test.bat`、`scripts/_compile_only.bat`。
