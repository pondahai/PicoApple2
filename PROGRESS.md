# 🍎 PicoApple2 - 專案進度表

## 🚀 當前版本：v1.2 (Background Write-back)
SD 回寫不再讓模擬器停頓：壓縮磁碟的整檔回壓改由 Core 1 分片執行，6502 與音訊全程不中斷，畫面停格也一併消除。同時拿掉已無必要的 2.5 秒回壓防抖，並加上板載 LED 作為 SD 寫入指示燈。

<details><summary>v1.1 (Stability Patch)</summary>

專案已修復了關鍵的啟動同步死鎖，並完整實作了 F1-F3 系統功能鍵與 GPIO 高頻掃描。

</details>

---

## ✅ 已完成功能
*   **🦀 Rust 模擬核心**: 完成 6502 模擬、Language Card 切換。
*   **📺 繪圖系統**: TFT DMA 掃描線雙緩衝 (62.5MHz SPI)。
*   **⚡ 效能優化**: RP2040 250MHz 穩定運行。
*   **🖥️ 綠色監視器**: `Fn+7` 切換 彩色 / 綠色單色顯示（TEXT / HIRES / LORES 全模式），狀態列顯示 COLOR/GREEN。
*   **🔄 系統重置**: 實作 F1 (Warm) / F2 (Cold) Reset，支援磁軌 0 重啟。
*   **💾 磁碟系統**: 支援 .DSK 掛載、20MHz SPI 高速模式、開機自動載入。
*   **📦 壓縮磁碟**: 支援 `.gz` / `.zip` 載入與寫回(gz 多成員真壓縮、zip stored;uzlib streaming)。
*   **⚙️ 背景回壓**: 回壓搬到 Core 1 分片執行，模擬器與音訊不再停頓（設計與量測詳見 DevLog 2026-09-10）。
*   **💡 SD 寫入指示燈**: 板載 LED (GPIO25)。閃一下 = 換軌寫回 work dsk;恆亮 ~0.7s = 整檔回壓。
*   **🎮 控制系統**: GPIO 按鍵高頻掃描（解除幀率限制）、WebSerial F1-F3 完整映射。

---

## ⚠️ 開發瓶頸 (Blockers) & 實驗記錄
*   **🛠️ 磁碟相容性當機 (Goonies)**: 32-cycle 核心在處理非標準位元組時可能陷入搜尋死鎖，需導入防禦性邊界檢查。
*   **🛠️ 磁碟寫入不穩定 (ERROR #8)**: 磁頭寫入相位抖動。
*   **⚠️ 位元級重構 (Reverted)**: 4-cycle Bit-level 實驗因系統不穩與啟動掛起已**正式撤回**，目前回歸 32-cycle Byte-level 穩定核心。

---

## 🎮 實體控制系統
*   **完整引腳對應**: 補全所有實體按鈕映射（UP/DOWN/LEFT/RIGHT/A/B/MENU/ALT）。
*   **ALT 組合鍵**: ALT+A=速度、ALT+B=方向/搖桿切換、ALT+RIGHT=ENTER、ALT+DOWN=SPACE（邊緣偵測 + 按住抑制原功能）。
*   **智慧導航**: 選單模式下自動切換方向鍵邏輯，支援限速滾動。
*   **磁碟選單分頁**: 清單無檔案數上限，按需讀目錄、自動捲動分頁（頁碼 + `^`/`V` 指示）。
*   **虛擬搖桿**: 鍵盤實體映射鍵完整模擬 Apple II 雙軸搖桿。

## 🌐 專業虛擬控制台 (Pro Console)
*   **WebSerial 介面**: 開發基於 Chrome/Edge 的網頁控制台，取代傳統 ANSI 終端機。
*   **ANSI 解析修復**: 完整支援終端機 F1-F3 系統功能鍵。
*   **即時狀態協定 (STX)**: 實施 Press/Release 雙態同步，解決雲端/遠端按鍵重複問題。

---

## 📅 近期計畫 (Next Steps)
0.  **✅ 壓縮檔 repack 機制重新設計 (2026-09-10 完成，實機驗證通過)**: 原本的「馬達停轉防抖 2.5s → 同步整檔重壓」會把整台機器凍住約 0.4 秒。已改為：
    - **Core 1 背景執行**：Core 0 只投信 (`postRepackIfDirty`) 就繼續跑 `apple2_tick()`。實測回壓期間 Core 0 仍是 1012 kcyc/s、throttled 100%，完全不受影響。
    - **分片執行**：`disk_archive` 新增 `archive_compress_begin/_step/_abort`，每次 `loop1()` 只壓一個 gz member；`GZ_CHUNK` 16K→4K 讓單步降到一個影格以內。解決了「Core 1 被佔住 425ms → 畫面停格 25 幀 → 看起來像程式暴衝」。
    - **中止機制**：暫存檔要到最後一步才 `rename`，任一步中止都安全，所以 Core 0 需要 SD 時可請 Core 1 在分片邊界收手，最多等 ~12ms。
    - **拿掉 2.5s 防抖**（改為 0）：背景化後防抖已無必要。實測發現連續磁碟活動期間馬達根本不會停轉，**合併是馬達狀態天然做掉的**（8 次換軌寫入只換來 1 次回壓），那個防抖從一開始就大部分多餘。
    - **未變的部分**：`LASTDISK.TXT` 仍記原壓縮檔路徑、開機重新解壓到 `/_WORK.DSK`、回壓仍是整檔重壓（暫存 → `SD.rename`）、F3 換片仍走同步回壓路徑。
    - **⚠️ 尚未驗證**：`ABORTED` 路徑從未實測走過（正確性目前只有邏輯論證）;`GZ_CHUNK` 16K→4K 的壓縮率代價沒有數字;單步凍結時間未直接量測。詳見 DevLog 2026-09-10 §7。
0.  **📁 F3 選單資料夾結構 (未實作)**: 目前選單只掃 SD **根目錄**，不進子資料夾。計畫加入瀏覽：
    - 操作：**右鍵/Enter 進入資料夾、左鍵回上一層**；檔案上 Enter = 載入。
    - 基礎已備妥：選單已改為「按需讀目錄」(core0 `scanDiskFiles`/`fillDiskWindow`/`findDiskNameByIndex`)，把掃描目標從固定 `/` 改成可變的 `g_menu_dir` 即可；目錄項需以 `isDirectory()` 標記並在清單中區分顯示 (如 `[DIR]`/`name/`)。
    - 要接的輸入(選單模式下左右鍵目前未用)：實體搖桿 `mat_joy_left/right`、序列協定 `J idx 2/3`、ANSI 方向鍵 `ESC[C/D` → 各自轉成新的 `g_menu_cmd`。
    - 注意：載入路徑改用 `g_menu_dir + name`；`WORK_DSK`/`REPACK_TMP` 維持在根目錄；`entry.name()` 為基本檔名需自行組完整路徑。
1.  **🔍 核心防禦性加固**: 修正 `apple2_core` 中的位元搜尋迴圈，防止 `goonies.dsk` 等異常格式導致系統鎖死。
2.  **🔍 相位偏移校準**: 在 32-cycle 基礎上實施 `cycles_accumulator` 餘量補償，解決 `SAVE` 與 `INIT HELLO` 驗證失敗問題。
3.  **🔊 音效強化**: 導入多層級 Apple II 撥放音效演算法優化。
4.  **🖥️ 髒行偵測 (Dirty-line Detection)**: 顯示管線重疊已消除一般動態梳狀，但**全螢幕捲動**仍受 192 行 SPI 吞吐上限約束（該幀所有行皆變動 → 退化為畫滿）。計畫對螢幕來源 RAM 留 shadow，逐行比對 40 bytes，未變動的行整條跳過不畫（保持型面板自然留存），把省下的頻寬拿去把變動行畫成**逐行 (progressive)** → 變動區零梳狀。需處理邊界：文字反白閃爍翻轉、模式切換/page2 翻頁須強制標髒；全螢幕捲動為最壞情況。詳見 DevLog.md 2026-06-18。

---
*最後更新日期：2026-09-10 (SD 回壓背景分片化 + 板載 LED 寫入指示燈)*
