# 🍎 PicoApple2

**PicoApple2** 是一個專為 Raspberry Pi Pico (RP2040) 設計的高效能 Apple II 模擬器。它結合了 **Rust** 撰寫的精確模擬核心與 **C++/Arduino** 實作的雙核渲染架構，能在微控制器上實現全速、全彩的 Apple II 體驗。

---

## ✨ 核心特色

*   **🦀 Rust 驅動核心**: 使用 Rust (`thumbv6m-none-eabi`) 實作 6502 CPU、記憶體映射與 Disk II 控制器，確保模擬精確度。
*   **⚡ 雙核加速架構**: 
    *   **Core 0**: 運行模擬邏輯、處理輸入與 SD 卡 I/O。
    *   **Core 1**: 專職 **JIT 視訊渲染**，直接從 RAM 即時生成 ILI9341 (SPI) 像素。
*   **🚀 極致效能**: 預設超頻至 **250MHz**，在磁碟運轉時自動放開限速，實現極速載入。
*   **💾 完整的磁碟支援**: 支援 `.DSK` 檔案讀寫，實作了物理驗證、磁軌同步與馬達慣性停轉等技術，確保資料持久化安全並相容於依賴精確時序的快速載入器 (fast loader)。
*   **📦 壓縮磁碟 (gz / zip)**: 可直接載入 `.gz` 與 `.zip` 壓縮磁碟映像（開機時串流解壓到工作檔），並支援**寫回**——換片時把改動回壓覆蓋原檔（gz 為真壓縮的多成員 gzip；zip 為 stored）。
*   **🔊 週期精確音訊**: 喇叭 (`$C030`) 翻轉以「模擬週期時間戳」記錄，並由 Core 1 的硬體計時器在精確的真實時刻重放，消除批次執行造成的音高與音質失真。
*   **🎮 現代化控制**: 內建虛擬搖桿映射、功能鍵選單與 SD 卡檔案瀏覽器（支援分頁捲動、無檔案數上限、ALT 硬體組合鍵）。

---

## ⚠️ 重要聲明 (Legal & ROM Requirement)

本專案**不提供**且**不包含**任何受版權保護的 Apple II ROM 檔案或軟體。

1.  **使用者責任**：玩家必須自行合法取得 Apple II 的 System ROM (`apple2_sys.rom`)、字體 ROM (`apple2_char.rom`) 與 Disk II 控制卡 ROM (`disk2_p5.rom`)。
2.  **檔案放置**：請將取得的 ROM 檔案放置於 `apple2_core/src/` 目錄下後再進行編譯。
3.  **合規使用**：本專案僅供學術研究與個人復刻體驗使用，請確保您遵守當地的版權法律規範。

---

## 🛠️ 硬體需求

*   **處理器**: Raspberry Pi Pico (RP2040)
*   **顯示器**: ILI9341 2.8" SPI TFT (320x240)
*   **儲存**: SD 卡模組 (SPI)
*   **輸入**: 鍵盤矩陣 (74HC165/74HC595 驅動)

---

## 🔌 GPIO 接線表 (Pin Mapping)

| 類別 | 功能 | Pico 引腳 (GPIO) | 說明 |
| :--- | :--- | :--- | :--- |
| **顯示器 (SPI0)** | SCK / MOSI / MISO | 18 / 19 / 16 | 連接至 ILI9341 |
| | CS / DC / RST / BL | 17 / 20 / 21 / 22 | |
| **SD 卡 (SPI1)** | SCK / MOSI / MISO / CS | 10 / 11 / 12 / 13 | 連接至 SD 卡模組 |
| **音效 (Audio)** | Sound Out | 7 | 1-bit PWM 輸出 (需接低通濾波器與放大器) |
| **鍵盤矩陣** | Data Out / Latch | 15 / 14 | 連接至 74HC595 / 74HC165 |
| | Clock / Data In | 26 / 27 | |
| **選單按鈕** | Up / Down | 9 / 5 | 輔助導航按鈕 (Pull-up) |
| | Left / Right | 8 / 6 | 搖桿 X 軸模擬 |
| | Button A / B | 2 / 3 | A:確認 (PB0), B:返回 (PB1) |
| | Menu (Start) | 4 | 進入磁碟選擇選單 (F3) |
| | ALT | 28 | 組合鍵修飾鍵（見下方「實體按鈕組合鍵」） |

---

## 🔊 音效電路建議 (Audio Circuit Note)

由於 GPIO 7 輸出的是未經過處理的 1-bit 數位訊號，建議建構以下電路以獲得最佳音質：
1.  **低通濾波器 (LPF)**：使用簡單的 RC 電路（如 1kΩ 電阻與 100nF 電容）來濾除高頻數位雜訊。
2.  **音訊放大器**：建議連接至 **PAM8403** 或類似的 D 類放大器模組來驅動 4Ω/8Ω 喇叭。
3.  **隔離電容**：在進入放大器前建議串接一個 10uF 的電解電容以隔絕直流分量 (DC Offset)。

---

## 🚀 環境架設 (Environment Setup)

在開始編譯前，請確保您的開發環境已完成以下配置：

### 1. Arduino 開發環境
本專案建議使用 **Arduino IDE 2.x** 進行基礎設定，並透過自動化腳本進行編譯。

*   **安裝 RP2040 核心**: 在 Arduino IDE 的「開發板管理員」中搜尋並安裝 `Raspberry Pi Pico/RP2040` (by Earle F. Philhower, III)。**版本要求: 5.5.1+**。
*   **安裝必要程式庫**: 在「程式庫管理員」中搜尋並安裝以下元件：
    1.  **Adafruit GFX Library**
    2.  **Adafruit ILI9341**
*   **FQBN**: `rp2040:rp2040:rpipico`

### 2. Rust 開發環境 (核心模擬器)
*   **安裝 Rust**: [rustup.rs](https://rustup.rs/)
*   **安裝編譯目標**: 
    ```bash
    rustup target add thumbv6m-none-eabi
    ```

---

## 🔨 編譯與上傳 (Build & Upload)

本專案已實作 **動態環境掃描系統**，會自動從您的 Arduino IDE 設定中抓取工具鏈路徑。

### 第一次執行 (環境檢查)
在開始編譯前，請先執行 `check_env.bat`。此腳本會：
1. 掃描 `~/.arduinoIDE/arduino-cli.yaml` 取得路徑。
2. 自動尋找 `arduino-cli.exe` 與 `picotool.exe` 的實際位置。
3. 產生 `build_env.bat` 環境設定檔。
4. 在視窗中顯示掃描結果供您確認。

```bash
.\check_env.bat
```

### 快速編譯與上傳 (自動化腳本)
確認環境正確後，您可以使用以下腳本進行開發：

*   **一鍵全編譯** (`full_build.bat`):
    自動執行環境掃描 -> 編譯 Rust 核心 -> 同步靜態庫 -> 編譯 `PicoApple2.ino` -> 自動透過 1200bps 重置並上傳至 Pico。
    ```bash
    .\full_build.bat
    ```
*   **僅更新 Rust 核心** (`build_rust.bat`):
    當您修改了 `apple2_core/` 下的 Rust 代碼時，執行此腳本會編譯並將產出同步至 Arduino 程式庫目錄，之後您可以直接在 Arduino IDE 中點擊上傳。

---

## 💾 搭配開機載入器 (rp2040-retro-loader)

[rp2040-retro-loader](https://github.com/pondahai/rp2040-retro-loader) 是同一台掌機的開機載入器：冷開機進選單、從 SD 卡挑一個 `.uf2` 燒進 flash、交棒執行。要讓 PicoApple2 能被它載入，必須用**偏移編譯模式**重新編譯。

```bash
.\build_offset.bat
```

產出 `build_offset\PicoApple2_standalone.uf2`。這個檔案兩種用法都吃得下：放進 SD 卡根目錄給載入器，或直接用 USB 燒進去。**平常開發不受影響**——`full_build.bat` 完全沒動，偏移模式是另一支獨立的腳本。

這支腳本**不需要你在 Arduino sketchbook 裡安裝 `Apple2Core` 程式庫**，它會自己生成一份（見下面「不依賴 sketchbook」）。

### 為什麼要重新編譯

載入器住在 flash 最前面 16KB（ROM 只認那裡），所以專題本體必須讓開：

```
0x10000000  載入器 或 跳板（16KB，兩者擇一）
0x10004000  PicoApple2 本體（最前面就是向量表）
```

不能只是把檔案往後搬。RP2040 是 XIP，所有位址在編譯時就寫死在機器碼裡了，搬 bytes 只會讓每個指標都指向錯的地方。

### 這個專案改造時的三件事

改造流程照 loader repo 的 README §3.4 檢查清單走。對 PicoApple2 來說：

**① 換 linker script — 但這裡不是 pico-sdk 專案**

清單假設的是 pico-sdk + CMake（`pico_set_linker_script`）。PicoApple2 走的是 arduino-cli + arduino-pico，機制完全不同：linker script 是 build 期由 `simplesub.py` 從 `lib/rp2040/memmap_default.ld` 生成到 `{build.path}/memmap_default.ld`，而 link 指令寫死讀那個檔名。

所以正確的換法不是加 `-Wl,--script`（會跟寫死的那個打架），而是覆蓋 platform.txt 的 prelink hook，把它的 `--input` 指到我們的偏移版樣板。`build_offset.bat` 的第 5 步就是在做這件事。

偏移版樣板由 `loader_offset/gen_app_ld.py` 從 arduino-pico 自己那份生成，**不要手改**。換 arduino-pico 版本就重跑（`build_offset.bat` 每次都會重跑一次）。

**② ⚠️ arduino-pico 的向量表不在 image 最前面 —— infones 沒遇過的坑**

這是改造這個專案時最關鍵的一件事。arduino-pico 預設編譯出來的實測佈局是：

| 位址 | 段 | 大小 |
|---|---|---|
| `0x10000000` | `.boot2` | 256 |
| `0x10000100` | `.ota` | `0x27f4` |
| `0x100028f4` | `.partition` | `0x70c` |
| `0x10003000` | `.text`（向量表從這裡才開始） | |

也就是說 arduino-pico 一律先經過一段 OTA 前導程式才進本體，ROM/boot2 跳的是 `0x10000100` 而不是向量表。

而載入器與跳板是**直接讀 `APP_BASE` 的向量表（SP + Reset）然後跳**。如果只把 `ORIGIN` 改成 `0x10004000` 而留著這兩段，向量表會落在 `0x10007000`，載入器跳到 `0x10004000` 只會拿到 OTA blob 的頭幾個 byte 當堆疊指標——開機直接死，**而且症狀跟「根本沒燒進去」一模一樣**。

本專案不用 OTA、不用 LittleFS（磁碟映像走 SD 卡），所以 `.boot2` / `.ota` / `.partition` 三段一起丟掉。丟掉之後 `0x10004000` 第一個 byte 就是向量表，跟 infones 的偏移版同形。

丟輸出段還不夠：`ota.o` 與 `boot2.o` 是 link 指令寫死拉進來的，輸入段不明確 discard 的話 ld 會把它們當 orphan 隨手安置，很可能就塞在向量表前面。`gen_app_ld.py` 因此補了一段 `/DISCARD/`。

**③ 寫死的 flash 位址 —— 這一項不適用**

清單裡最難查的第 ③ 項（專題自己在 flash 劃地盤存 ROM／存檔／資源，位址不會跟著位移）在這裡是空的。PicoApple2 的磁碟映像與存檔全部走 SD 卡，flash 上只有 image 本身。grep 過 `.c` / `.cpp` / `.h` / `.ino` / `.rs` 全部沒有寫死的 `0x10xxxxxx`、沒有 `flash_range_*`、沒有 EEPROM / LittleFS。

這也是 infones（`NES_FILE_ADDR` 撞上存檔槽）跟這裡最大的差別。

**④ build 期佈局檢查**

`loader_offset/check_flash_layout.py`，`build_offset.bat` 第 6 步自動跑。因為第 ③ 項不適用，它守的不是「資料區重疊」而是上面第 ② 項：向量表是否正好在 `0x10004000`、SP/Reset 是否通得過載入器的 `app_present()`、UF2 是否連續、image 尾端有沒有越過 flash 可用上限。

### 不依賴 sketchbook

`build_offset.bat` 的第 3 步會生成一份自足的 `Apple2Core` 程式庫給 arduino-cli 用：

```
build_offset\arduino_libs\Apple2Core\
    library.properties                    生成（precompiled=true + ldflags）
    src\Apple2Core.h                      從 repo 根目錄的 Apple2Core.h 複製
    src\cortex-m0plus\libapple2_core.a    從這次剛編好的 Rust 產出複製
```

`--libraries` 只指向這裡。sketch 用到的其他程式庫（SPI / SD / SDFS / SdFat）都隨 rp2040 平台附帶，所以**整個 build 不碰 Arduino sketchbook 一根寒毛**。

**為什麼要這樣做。** 原本的依賴鏈是「`arduino-cli.yaml` 的 `user:` → `scan_env.ps1` 掃出路徑 → 那底下要有手工維護的 `Apple2Core` → 它的 `src/` 底下要有最新的 `.h` 與 `.a`」。這條鏈斷過三次，而且每次的症狀都不像環境問題：

1. sketchbook 搬到另一顆磁碟後 `arduino-cli.yaml` 還指著舊路徑 → `Apple2Core.h: No such file or directory`
2. 搬家時 `src/` 沒跟著搬，只剩 `library.properties` → 同樣的錯誤訊息
3. `src/cortex-m0plus/` 底下的 `.a` 過期，連結器照用不誤 → 修正沒進韌體，而且完全沒有警告（`PROGRESS.md` 有記這一次，查了很久）

生成的話這三個都不可能發生：`.h` 只有 repo 根目錄一個來源，`.a` 永遠是這次剛編的，路徑固定。

**順帶修掉一個長期的坑。** repo 根目錄的 `library.properties` 原本寫 `dot_a_linkage=true`，那是壞的——它會叫 arduino-cli 去找一個「由本程式庫自己的原始碼編出來的」`Apple2Core.a`，而這個程式庫沒有原始碼。真正能動的 `precompiled=true` + `ldflags=-lapple2_core` 當年只改在某台機器的 sketchbook 裡，從來沒回流到 repo。現在兩邊都是能動的版本，而且以 `make_arduino_lib.py` 為準。

### Rust 核心不需要任何改動

`apple2_core` 只產出 `libapple2_core.a`（staticlib，沒有自己的 linker script、沒有 `.cargo/config.toml`、沒有寫死的位址），所有位址都由 arduino-pico 的 linker script 決定。偏移編譯對 Rust 那邊是透明的，**`build_offset.bat` 跟 `full_build.bat` 用的是同一份 `.a`**。

### 實測數字（arduino-pico 5.6.1 / rpipico 2MB）

| | 預設編譯 | 偏移編譯 |
|---|---|---|
| image 起點 | `0x10000000` | `0x10004000` |
| 向量表 | `0x10003000` | `0x10004000` |
| image 大小 | 177,540 bytes | 177,284 bytes |
| image 尾端 | — | `0x10030e00` |
| 上限（EEPROM 區起點） | `0x101ff000` | `0x101ff000` |
| 餘裕 | — | 1,892,864 bytes |

空間非常寬裕，偏移 16KB 對這個專案沒有壓力（DOOM 那邊才是緊的）。

### 驗證狀態

**已實機驗證（2026-08-15）**：

*   **跳板路線**：`PicoApple2_standalone.uf2` 用 `picotool load -v -x` 燒進掌機，開機進入 Apple II 畫面，**字形正常**（字元 ROM 的顯示路徑一併確認）。
*   **載入器路線**：`loader.uf2` 燒進掌機、standalone 版放 SD 卡根目錄，冷開機 → 載入器選單 → 選檔 → 燒錄 → 交棒 → Apple II 畫面。

也就是說底層假設全部成立：偏移編譯、丟掉 `.boot2` / `.ota` / `.partition`、向量表落在 `0x10004000`、交棒，以及同一份 UF2 兩種燒法都吃得下。

**已實機驗證（2026-08-16）**：

*   **`build_offset.bat` 本身**：在乾淨環境下完整跑完 7 個步驟，產出 782 塊的合併 UF2，拆開驗證 `APP_BASE` 以下 64 塊、以上 718 塊、位址斷點 0 個。
*   **新版跳板**：載入器 repo 更新後跳板從 22 塊變 12 塊，重編的 standalone 燒進去開機正常。
*   **磁碟讀寫**：偏移模式下 `.dsk` 的讀與寫都正常。這是先前最大的一塊空白，現在確認 flash 佈局的改動確實碰不到它（磁碟全走 SD 卡）。

> 08-15 那次實機驗證用的韌體，是在腳本尚未自足時用等效指令手動編出來的——當時 sketchbook 路徑已經壞了，腳本自己跑不完。韌體本身是對的（燒進去會動），但「腳本能不能自己跑完」直到 08-16 才真正驗證。

**尚未驗證**：

*   燒錄中途失敗、UF2 損毀、SD 卡中途拔出等錯誤路徑。
*   壓縮磁碟（`.gz` / `.zip`）的載入與回壓在偏移模式下沒有單獨測過。

### 燒錄與注意事項

```bash
picotool load -v -x build_offset\PicoApple2_standalone.uf2
```

*   `-v` 會逐塊驗證。開發時建議用它而不是拖曳——位址不連續的 UF2 拖曳會安靜截斷（loader README §3.5 坑 3；`merge_uf2.py` 已經用 `0xFF` 補過空隙，但 `-v` 仍然比較好查）。
*   `picotool load -x` 與 `picotool reboot` 都是**軟重置**，載入器會直接穿透到 app、不顯示選單。要看選單就燒完不加 `-x`，然後**拔電冷開機**。
*   `build_offset\PicoApple2.ino.uf2`（沒有 `_standalone`）是只有本體的版本，前 16KB 是空的，**不能單獨燒錄**。

---

## 🖥️ 專業虛擬終端機 (Pro Console)

本專案內建一個基於 **WebSerial** 的高效能虛擬控制台，讓您可以透過電腦鍵盤完美模擬 Apple II 的所有操作。

### 如何啟動
在專案根目錄下執行 `terminal.bat`，系統將自動以 Chrome/Edge 瀏覽器開啟 `Apple2Keyboard.html`。

### 核心功能
*   **全鍵盤映射**: 捕捉電腦按鍵並轉換為 Apple II ASCII 碼，支援 **CTRL 組合鍵**、Enter、Backspace 與 Esc。
*   **即時狀態協定 (STX)**: 不同於傳統終端機，本控制台實作了專屬的按下/放開狀態同步，實現真正的「長按」行為。
*   **打字 / 貼上雙通道輸入**: 打字與連發走「即時通道」(`'K'` 封包，硬體 latch 式 newest-wins，不會被連發塞住)；貼上整段文字走「緩衝通道」(`'P'` 封包，FIFO 保序不漏字)。兩者在瀏覽器源頭就分流，確定性無誤判。可用 `Ctrl+V` 或 `MANUALLY PASTE` 按鈕貼上，並以「Paste Settings」滑桿微調字元間延遲。
*   **專業級終端顯示**: 內建 `xterm.js` 渲染引擎，同步顯示 Pico 輸出的所有 Log 與 Debug 訊息。
*   **記憶連線**: 只要連線過一次，下次啟動僅需點擊 `RECONNECT` 即可快速上線。

---

## 🎮 完整操作指南 (Interaction & Controls)

本專案支援 **實體鍵盤矩陣** 與 **虛擬終端機** 兩種輸入方式。大部分的操作在兩者間是共通的，主要差異在於**系統功能鍵**的觸發方式（終端機直接使用 `F1~F5` 鍵，而實體鍵盤使用 `Fn + 數字` 組合鍵）。

### 1. 一般按鍵與搖桿映射 (共通)
| 功能 | 實體鍵盤 / 終端機按鍵 | 說明 |
| :--- | :--- | :--- |
| **打字輸入** | `A-Z`, `0-9`, `符號` | 自動轉換為 Apple II ASCII |
| **組合鍵** | `CTRL + A-Z` | 傳送標準控制碼 (如 Ctrl-C 中斷) |
| **大寫鎖定** | `Fn + C` / (終端機無) | 實體鍵盤切換大小寫鎖定 (預設為大寫) |
| **虛擬搖桿** | `方向鍵 (Arrows)` | 精確模擬 Paddle 0/1 的狀態 (支援長按，僅在 F4 搖桿模式下) |
| **蘋果按鈕** | `Page Up / Down` | 對應 PB0 (Open-Apple) 與 PB1 (Closed-Apple) |

### 2. 系統功能鍵 (System & Function Keys)
| 功能 | 終端機 (直接按) | 實體鍵盤 (組合鍵) | 說明 |
| :--- | :--- | :--- | :--- |
| **Warm Reset** | `F1` | `Fn + 1` | 軟重啟 (Ctrl-Reset) |
| **Cold Reset** | `F2` | `Fn + 2` | 冷重啟並重新載入磁軌 0 |
| **磁碟選單** | `F3` | `Fn + 3` | 開啟 SD 卡磁碟選單 (`.DSK` / `.GZ` / `.ZIP`) |
| **模式切換** | `F4` | `Fn + 4` | 切換「方向鍵」為 搖桿模式 / 鍵盤模式 |
| **速度切換** | `F5` | `Fn + 5` | 循環切換模擬速度 (x1.0, x1.2, x1.4, x1.5) |

### 3. 實體按鈕組合鍵 (ALT Combos)
按住 `ALT` (GPIO 28) 再點按下列鍵（皆為邊緣偵測，按一下觸發一次；ALT 按住期間該鍵的原功能會被抑制）：

| 組合 | 功能 |
| :--- | :--- |
| `ALT + A` (Btn A) | 循環模擬速度 (x1.0 / x1.2 / x1.4 / x1.5) |
| `ALT + B` (Btn B) | 切換 方向鍵 ↔ 搖桿 模式 |
| `ALT + RIGHT` | 送出 **ENTER** (Return) |
| `ALT + DOWN` | 送出 **SPACE** |
| `ALT + LEFT` | 送出 **`J`** (大寫) |
| `ALT + UP` | 送出 **`K`** (大寫) |

### 4. 磁碟選單操作 (Disk Menu)
當進入磁碟選單後（F3 / Fn+3 / Menu 按鈕），操作方式如下：
* **清單內容**：列出 SD 根目錄的 `.DSK` / `.GZ` / `.ZIP` 檔；**無檔案數上限**，超過一頁時自動分頁（右上顯示 `(目前/總數)`，上下還有項目時右緣顯示 `^` / `V`）。
* **導航**：使用 `方向鍵 (Up/Down)` 選擇檔案，跨頁自動捲動。
* **確認**：按下 `Enter` 載入所選磁碟（熱抽換，模擬不重啟；壓縮檔會先解壓再載入。如需從新碟開機請另按 F2 / Fn+2 Cold Reset）。
* **取消**：按下 `Esc` 關閉選單並返回。

> **壓縮磁碟寫回**：若載入的是 `.gz` / `.zip`，對磁碟的寫入(如 DOS `SAVE`)會先存進工作檔，並在**磁碟馬達停轉約數秒後自動回壓**覆蓋原壓縮檔(防抖：把連續寫入合併成一次重壓)；**換片時**(F3 選單再選一張)也會回壓當保險。回壓當下會短暫凍結模擬。純斷電(寫入後馬達尚未停轉滿延遲)則不回壓，改動仍保留在工作檔。

---

## 📂 專案結構 (Project Structure)

*   `PicoApple2.ino`: 主程式進入點（原 `pico_apple2_emulator.ino`）。
*   `apple2_core/`: Rust 撰寫的 Apple II 模擬器核心。
*   `disk_archive.h/.cpp`: gz / zip 磁碟映像的讀取與寫回(壓縮)膠合層。
*   `src/uzlib/`: vendored uzlib(deflate/inflate;取自 rp2040 core OTA 模組，zlib 授權)。
*   `scripts/scan_env.ps1`: 動態環境掃描核心腳本。
*   `scripts/test_archive.c` / `archive_proto.py`: 壓縮 codec 的 host 端驗證工具。
*   `check_env.bat`: 環境驗證工具。
*   `Apple2Core.h`: C/Rust FFI 接口定義。
*   `src/`: 存放編譯後的 `libapple2_core.a` 靜態庫。
*   `build_offset.bat`: 偏移編譯（搭配 rp2040-retro-loader），見上面「搭配開機載入器」一章。
*   `loader_offset/make_arduino_lib.py`: build 期生成自足的 `Apple2Core` 程式庫，讓編譯不依賴 Arduino sketchbook。
*   `loader_offset/gen_app_ld.py`: 從 arduino-pico 的 `memmap_default.ld` 生成偏移版 linker script。
*   `loader_offset/memmap_app_arduino.ld`: **生成的，不要手改**（改 arduino-pico 版本就重跑上面那支）。
*   `loader_offset/check_flash_layout.py`: 偏移編譯的 build 期佈局檢查。

---

## 📜 授權

本專案採用 MIT 授權。
