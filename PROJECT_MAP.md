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
- [x] **FIXME (Disk Read) - RESOLVED (2026-06-11 實機驗證通過):** `goonies.dsk` 無法正常載入。
    - **已確認事實:** 該映像檔為網路常見來源，在其他模擬器可正常遊玩 → 排除防拷映像假說，問題在本核心；且自專案開始從未載入成功（DevLog 03-26「啟動成功」為筆誤，已勘誤）。
    - **2026-06-11 修正（已過主機端模擬）:** (1) 馬達關閉增加 1 秒延遲停轉（真實類比板行為，loader 常依賴）；(2) 讀取側相位捨入補償改為僅在該次馬達啟動期間用過寫入模式 (Q7) 時生效，避免把純讀取的 32-cycle 節奏拉長到 ~48 cycles 破壞 fast loader；(3) 換軌時保留旋轉相位（`byte_index` 不再歸零）。
    - **主機端模擬結果 (2026-06-11):** `src/boot_test.rs` 全系統開機模擬確認：遊戲載入 T0-T23 後停在搖桿校正提示（文字頁 2，等待按鈕 0），按鈕通過後快速載入 T24-T27 關卡資料並進入純 HIRES 遊戲畫面。根因是 loader 每讀一個磁區就關開馬達，舊核心立即停轉導致每磁區多等 1.5 秒起轉延遲。模擬指令：`BOOT_DSK=<dsk路徑> BOOT_PRESS_BTN=30 BOOT_SECS=60 cargo test --release boot_smoke -- --nocapture --test-threads=1`。
    - **操作注意:** 遊戲開機後停在「HOLD JOYSTICK...」校正畫面（顯示於文字頁 2，渲染端已支援），需按搖桿按鈕 0 數次完成校正才會載入關卡，這是正常流程不是當機。
    - **驗證紀錄 (2026-06-11):** 實機（Pico）燒錄後 goonies.dsk 開機、校正、關卡載入全數通過；主機端模擬 MASTER.DSK 開機回歸正常。注意：當日曾因兩個建置陷阱導致修正未進韌體——(a) 本機 main 落後 origin/main（修正只在遠端）；(b) Arduino 連結器優先使用 `Dropbox\Arduino\libraries\Apple2Core\src\cortex-m0plus\` 的過期預編譯 `.a`。重編時務必先 `git pull` 並同步新 `.a` 到該庫的 `src/` 與 `src/cortex-m0plus/`。
    - **待回歸（實機）:** DOS 3.3 `LOAD`、`SAVE`、`INIT HELLO` 寫入路徑（相位補償已改為僅寫入模式後生效，理論上不受影響，建議抽空實測）。
