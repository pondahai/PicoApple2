# 🍎 PicoApple2 - 專案進度表

## 🚀 當前版本：v1.1 (Stability Patch)
專案已修復了關鍵的啟動同步死鎖，並完整實作了 F1-F3 系統功能鍵與 GPIO 高頻掃描。

---

## ✅ 已完成功能
*   **🦀 Rust 模擬核心**: 完成 6502 模擬、Language Card 切換。
*   **📺 繪圖系統**: TFT DMA 掃描線雙緩衝 (62.5MHz SPI)。
*   **⚡ 效能優化**: RP2040 250MHz 穩定運行。
*   **🔄 系統重置**: 實作 F1 (Warm) / F2 (Cold) Reset，支援磁軌 0 重啟。
*   **💾 磁碟系統**: 支援 .DSK 掛載、20MHz SPI 高速模式、開機自動載入。
*   **🎮 控制系統**: GPIO 按鍵高頻掃描（解除幀率限制）、WebSerial F1-F3 完整映射。

---

## ⚠️ 開發瓶頸 (Blockers) & 實驗記錄
*   **🛠️ 磁碟相容性當機 (Goonies)**: 32-cycle 核心在處理非標準位元組時可能陷入搜尋死鎖，需導入防禦性邊界檢查。
*   **🛠️ 磁碟寫入不穩定 (ERROR #8)**: 磁頭寫入相位抖動。
*   **⚠️ 位元級重構 (Reverted)**: 4-cycle Bit-level 實驗因系統不穩與啟動掛起已**正式撤回**，目前回歸 32-cycle Byte-level 穩定核心。

---

## 🎮 實體控制系統
*   **完整引腳對應**: 補全所有實體按鈕映射（UP/DOWN/LEFT/RIGHT/A/B/MENU/ALT）。
*   **智慧導航**: 選單模式下自動切換方向鍵邏輯，支援限速滾動。
*   **虛擬搖桿**: 鍵盤實體映射鍵完整模擬 Apple II 雙軸搖桿。

## 🌐 專業虛擬控制台 (Pro Console)
*   **WebSerial 介面**: 開發基於 Chrome/Edge 的網頁控制台，取代傳統 ANSI 終端機。
*   **ANSI 解析修復**: 完整支援終端機 F1-F3 系統功能鍵。
*   **即時狀態協定 (STX)**: 實施 Press/Release 雙態同步，解決雲端/遠端按鍵重複問題。

---

## 📅 近期計畫 (Next Steps)
1.  **🔍 核心防禦性加固**: 修正 `apple2_core` 中的位元搜尋迴圈，防止 `goonies.dsk` 等異常格式導致系統鎖死。
2.  **🔍 相位偏移校準**: 在 32-cycle 基礎上實施 `cycles_accumulator` 餘量補償，解決 `SAVE` 與 `INIT HELLO` 驗證失敗問題。
3.  **🔊 音效強化**: 導入多層級 Apple II 撥放音效演算法優化。

---
*最後更新日期：2026-03-28 (Stability Strategy Update)*
