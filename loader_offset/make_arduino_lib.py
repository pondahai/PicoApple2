#!/usr/bin/env python3
"""build 期生成一份自足的 Apple2Core Arduino 程式庫。

用法:
    python loader_offset/make_arduino_lib.py <輸出目錄> <libapple2_core.a 的路徑>

輸出:
    <輸出目錄>/Apple2Core/library.properties
    <輸出目錄>/Apple2Core/src/Apple2Core.h
    <輸出目錄>/Apple2Core/src/cortex-m0plus/libapple2_core.a

然後把 <輸出目錄> 交給 arduino-cli 的 --libraries。

--------------------------------------------------------------------------
為什麼要生成,而不是叫使用者去裝一個程式庫
--------------------------------------------------------------------------
原本的流程依賴一條很長的鏈:
    arduino-cli.yaml 的 user: 設定
      -> scan_env.ps1 掃出 ARDUINO_USER_LIB_PATH
        -> 那底下要有一個手工維護的 Apple2Core 程式庫
          -> 那個程式庫的 src/ 底下要有最新的 .h 與 .a

這條鏈斷過至少三次,而且每次的症狀都不像是「環境問題」:

  1. sketchbook 從 Dropbox 搬到 Google Drive 之後,yaml 還指著舊路徑,
     整個路徑不存在 -> "Apple2Core.h: No such file or directory"
  2. 搬家時 src/ 沒跟著搬,只剩 library.properties -> 同樣的錯誤訊息
  3. src/cortex-m0plus/ 底下的 .a 過期,連結器照用不誤 -> 修正沒進韌體,
     而且完全沒有警告(PROGRESS.md 有記這一次,查了很久)

生成的話這三個都不可能發生:.h 直接來自 repo 根目錄(單一事實來源),
.a 直接來自這次剛編好的產出,路徑固定在 build_offset/ 底下。

--------------------------------------------------------------------------
為什麼是 precompiled=true 而不是 dot_a_linkage=true
--------------------------------------------------------------------------
repo 根目錄那份 library.properties 寫的是 dot_a_linkage=true —— 那是壞的。
它會讓 arduino-cli 去找一個「由這個程式庫自己的原始碼編出來的」Apple2Core.a,
而這個程式庫沒有任何原始碼,於是 link 階段報「找不到 Apple2Core.a」。

precompiled=true + ldflags=-lapple2_core 才是能動的組合(這是使用者機器上
那份實際在用的設定)。arduino-cli 會自動加上 -L<src/cortex-m0plus>,
而 ldflags 的內容會被放進 compiler.libraries.ldflags —— 也就是 link 指令的
--start-group 裡面、sketch 的 object 之後。位置很重要:archive 排在需要它的
object 之前的話,ld 掃到它時還沒有人要那些符號,整包會被跳過,症狀是一整排
undefined reference to `apple2_*`。
"""

import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent

# arduino-pico 的 build.mcu,precompiled 程式庫的 .a 要放在 src/<這個>/ 底下
MCU_DIR = "cortex-m0plus"

LIBRARY_PROPERTIES = """\
name=Apple2Core
version=0.1.0
author=Rust
maintainer=Rust
sentence=Apple II Core for Pico
paragraph=Precompiled Rust static library for Apple II emulation.
category=Other
url=none
architectures=rp2040
precompiled=true
ldflags=-lapple2_core
includes=Apple2Core.h
"""


def die(msg):
    print("make_arduino_lib: " + msg, file=sys.stderr)
    sys.exit(1)


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(2)

    out_root = Path(sys.argv[1]).resolve()
    static_lib = Path(sys.argv[2]).resolve()

    header = REPO / "Apple2Core.h"
    if not header.is_file():
        die(f"找不到 {header} —— 這是 C/Rust FFI 介面的單一事實來源")
    if not static_lib.is_file():
        die(f"找不到 {static_lib} —— 請先編譯 Rust 核心")

    lib = out_root / "Apple2Core"
    mcu = lib / "src" / MCU_DIR
    # 整個重建,避免留下上一次的殘骸(尤其是過期的 .a)
    if lib.exists():
        shutil.rmtree(lib)
    mcu.mkdir(parents=True)

    (lib / "library.properties").write_text(LIBRARY_PROPERTIES, encoding="ascii")
    shutil.copy2(header, lib / "src" / "Apple2Core.h")
    shutil.copy2(static_lib, mcu / "libapple2_core.a")

    print(f"make_arduino_lib: 產出 {lib}")
    print(f"make_arduino_lib:   Apple2Core.h      <- {header}")
    print(f"make_arduino_lib:   libapple2_core.a  <- {static_lib} "
          f"({static_lib.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
