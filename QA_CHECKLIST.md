# 🍎 PicoApple2 - QA 測試與功能迴歸檢查表 (Regression Checklist)

這份檢查表用於在每次重大架構改動（如：渲染引擎重構、雙核通訊修改、硬體 I/O 最佳化）之後，進行全面的系統性測試，以避免「改 A 壞 B」的迴歸問題。

## 🛠️ 1. 編譯與佈署 (Build & Deploy)
- [ ] **Rust 核心編譯**：執行 `build_rust.bat` 成功，無 Error，警告 (Warning) 在預期範圍內。
- [ ] **靜態庫同步**：`libapple2_core.a` 與 `Apple2Core.h` 成功複製到 Arduino Library 目錄或專案 `src/` 目錄。
- [ ] **Arduino 編譯與燒錄**：執行 `full_build.bat` 成功，`picotool` 順利找到裝置並完成燒錄，裝置自動重啟。

## 📺 2. 視訊與渲染 (Video & Rendering)
- [ ] **開機畫面 (Text Mode)**：畫面正中央出現 `Apple //][` 標誌，下方出現閃爍的游標。字體清晰無破圖。
- [ ] **高解析度圖形 (Hi-Res Mode)**：載入支援 Hi-Res 的遊戲（如：Karateka 或 Lode Runner），確認 NTSC 假色演算法（紫、綠、藍、橘）顯色正確。
- [ ] **混合模式 (Mixed Mode)**：確認在 Hi-Res 遊戲畫面下方，能正確保留並渲染 4 行文字模式（Text Mode）的資訊列。
- [ ] **畫面流暢度與光柵同步**：
  - 觀察快速移動的物體，確認**沒有嚴重的畫面撕裂 (Tearing)**。
  - 確認交錯渲染 (Interlaced Rendering) 運作正常（60 Fields/sec），畫面更新不應有遲滯感。
- [ ] **UI 渲染**：確認右下角磁碟機馬達狀態列（小紅點與磁軌進度條）顯示正常，且不干擾主畫面。

## ⌨️ 3. 輸入與控制 (Input & Controls)
- [ ] **實體鍵盤直通 (Zero-delay Hardware Input)**：
  - 在 BASIC 提示字元下連續打字，確認手感脆彈，沒有「隔一層紗」的緩衝延遲感（驗證 `pushHardwareKey` 直通記憶體）。
  - 按下 Caps Lock / Shift 等組合鍵行為正確。
- [ ] **虛擬終端貼上測試 (Serial FIFO Stability)**：
  - 從電腦端終端機貼上一段長串的 BASIC 程式碼（如 50 行以上），確認 **100% 不漏字**（驗證 128-byte `g_key_fifo` 運作正常）。
- [ ] **模擬搖桿 (Joystick via Keyboard)**：
  - 確認方向鍵（上、下、左、右）映射至 Paddle 0/1 正確。
  - **⚠️ 長按壓力測試**：死死按住任一方向鍵，確認遊戲內角色移動**完全連續，沒有任何斷續感或卡頓**（驗證 VBLANK 同步掃描杜絕了 SPI DMA 雜訊）。
  - 確認搖桿按鈕 0/1 觸發正常。
- [ ] **系統快捷鍵**：
  - [ ] `Fn + 1`：觸發 Warm Reset。
  - [ ] `Fn + 2`：觸發 Cold Reset。
  - [ ] `Fn + 3`：成功呼叫出「磁碟選單 (Disk Menu)」。
  - [ ] `Fn + 4`：切換「搖桿模式 / 鍵盤模式」，確認切換時畫面有提示。

## 💾 4. 磁碟與 SD 卡 I/O (Disk & SD Card)
- [ ] **熱插拔與初始化**：開機時若未插卡應有錯誤提示。
- [ ] **磁碟選單 (F3 Menu) 強制優先權測試**：
  - 開啟選單後，畫面渲染停止，僅顯示選單 UI。
  - **選單導航 (F4 模式獨立)**：將模擬器切換至 **「鍵盤模式 (F4: ARROWS: KEYBOARD)」** 後進入 F3 選單，確認**方向鍵依然能正常捲動選單**（驗證選單導航不受模式限制）。
  - 按下確認鍵能成功載入 `.DSK` 檔案。
  - 按下 `ESC` 鍵能取消並退出選單。
- [ ] **讀取測試**：載入大型遊戲，確認載入過程中系統不會 Crash 或 Watchdog Timeout。
- [ ] **寫回測試 (Write-back)**：
  - 在支援存檔的遊戲（或儲存 BASIC 程式）中執行寫入操作。
  - 確認 `flushDirtyTrack()` 成功將資料寫回 SD 卡的 `.DSK` 檔案（檔案需以 `r+` 模式開啟）。
  - 重啟模擬器，確認存檔資料仍然存在。

## 🎵 5. 音效 (Audio)
- [ ] **喇叭輸出**：載入有音效的遊戲，確認 `PIN_JACK_SND` 有聲音輸出。
- [ ] **效能影響**：確認密集發聲時（如：連續按錯鍵的 Beep 聲），不會因為 CPU 週期計算錯誤而拖慢整體模擬器速度。

## 🧠 6. 系統穩定性與並發 (System Stability & Concurrency)
- [ ] **雙核同步鎖 (Spinlocks)**：
  - 確保 Core 0 (CPU 模擬) 與 Core 1 (視訊/輸入) 之間的變數交換（如：電子束位置 `BEAM_Y`、影片模式 `VIDEO_MODE`）使用 Atomic 變數或精準的 Spinlock。
  - 確認無死鎖 (Deadlock) 或單一核心長時間等待鎖定而導致 Watchdog Timeout 重啟。
- [ ] **長時間運行**：放置執行 DEMO 畫面超過 15 分鐘，確認不會因為記憶體洩漏或計時器溢位而當機。

---
*📝 註記：當進行任何會影響 `loop()` 或 `loop1()` 執行頻率、硬體中斷、DMA 傳輸、或 `apple2_tick()` 週期精準度的修改時，務必從頭到尾走過一次此檢查表。*