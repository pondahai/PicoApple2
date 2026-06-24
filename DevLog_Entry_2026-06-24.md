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
