// 主機端開機冒煙測試：用與 PicoApple2.ino 完全相同的 FFI 流程開機 DSK 映像，
// 觀察磁軌載入順序、CPU 卡點與畫面輸出，用於離線重現磁碟相容性問題。
// 執行方式（全域狀態共用，必須單執行緒）：
//   cargo test --release boot_smoke -- --nocapture --test-threads=1
// 可用環境變數 BOOT_DSK 指定要測的映像檔路徑。
#[cfg(test)]
mod tests {
    extern crate std;
    use std::collections::HashMap;
    use std::string::String;
    use std::vec::Vec;
    use std::{env, fs, println};

    fn text_row_addr(row: usize) -> usize {
        ((row & 0x07) << 7) | ((row & 0x18) * 5) | 0x0400
    }

    fn dump_text_screen() {
        let ram = crate::apple2_get_ram_ptr();
        for row in 0..24 {
            let addr = text_row_addr(row);
            let mut line = String::new();
            for col in 0..40 {
                let ch = unsafe { *ram.add(addr + col) } & 0x7F;
                line.push(if (0x20..0x7F).contains(&ch) { ch as char } else { '.' });
            }
            if line.chars().any(|c| c != '.' && c != ' ') {
                println!("  row {:2}: {}", row, line);
            }
        }
    }

    fn load_track(dsk: &[u8], track: u8) {
        let off = track as usize * 4096;
        crate::apple2_load_track(track, dsk[off..off + 4096].as_ptr(), 4096);
    }

    fn cpu_pc() -> u16 {
        let (mut pc, mut a, mut x, mut y, mut sp, mut st) = (0u16, 0u8, 0u8, 0u8, 0u8, 0u8);
        crate::apple2_get_cpu_state(&mut pc, &mut a, &mut x, &mut y, &mut sp, &mut st);
        pc
    }

    fn boot_disk(path: &str, seconds: u64) {
        println!("==================================================");
        println!("BOOT: {}", path);
        let dsk = fs::read(path).expect("read dsk");
        assert_eq!(dsk.len(), 143_360, "unexpected dsk size");

        crate::apple2_init();
        load_track(&dsk, 0);

        let target: u64 = seconds * 1_020_500;
        let mut cycles: u64 = 0;
        let mut track_loads: Vec<(u8, u64)> = Vec::new();
        let mut pc_hist: HashMap<u16, u32> = HashMap::new();
        let mut sample = 0u32;
        let mut last_qtr: i32 = -1;
        let mut qtr_moves: Vec<(i32, u64)> = Vec::new();

        // BOOT_PRESS_BTN=秒數: 模擬在該時刻按住搖桿按鈕 0 半秒
        let press_at: Option<u64> = env::var("BOOT_PRESS_BTN").ok().and_then(|s| s.parse().ok());

        while cycles < target {
            cycles += crate::apple2_tick() as u64;
            if let Some(at) = press_at {
                // 自該時刻起每 3 秒按住按鈕 0 半秒，並在各次按壓間交替推動搖桿到極限位置（模擬校正流程）
                let t0 = at * 1_020_500;
                if cycles >= t0 {
                    let phase = (cycles - t0) % 3_061_500;
                    crate::apple2_set_button(0, phase < 510_000);
                    let step = (cycles - t0) / 3_061_500 % 4;
                    let (px, py) = match step { 0 => (0, 0), 1 => (255, 0), 2 => (255, 255), _ => (0, 255) };
                    crate::apple2_set_paddle(0, px);
                    crate::apple2_set_paddle(1, py);
                }
            }
            let t = crate::apple2_needs_disk_reload();
            if t >= 0 {
                load_track(&dsk, t as u8);
                track_loads.push((t as u8, cycles));
            }
            sample += 1;
            // 取樣 PC 作為熱點直方圖（粗略即可）
            if sample % 7 == 0 && cycles > target.saturating_sub(2_000_000) {
                *pc_hist.entry(cpu_pc()).or_insert(0) += 1;
            }
            // 追蹤磁頭 1/4 軌位置變化
            unsafe {
                if let Some(ref m) = *core::ptr::addr_of!(crate::MACHINE) {
                    let q = m.mem.disk2.current_qtr_track;
                    if q != last_qtr {
                        last_qtr = q;
                        if qtr_moves.len() < 400 {
                            qtr_moves.push((q, cycles));
                        }
                    }
                }
            }
        }

        println!(
            "track load sequence ({}): {:?}",
            track_loads.len(),
            track_loads
                .iter()
                .map(|(t, c)| std::format!("T{}@{:.1}s", t, *c as f64 / 1_020_500.0))
                .collect::<Vec<_>>()
        );
        println!(
            "video mode: 0x{:02X}  motor: {}  denib_err: 0x{:X}",
            crate::apple2_get_video_mode(),
            crate::apple2_get_disk_motor_status(),
            crate::apple2_get_denibblize_error()
        );
        let mut hot: Vec<(u16, u32)> = pc_hist.into_iter().collect();
        hot.sort_by(|a, b| b.1.cmp(&a.1));
        println!("hot PCs (last 2s): {:?}", hot.iter().take(8).map(|(pc, n)| std::format!("{:04X}x{}", pc, n)).collect::<Vec<_>>());
        // BOOT_FLOW=1: 錄製卡死迴圈的控制流（$0300-$05FF，略過延遲迴圈，連續重複去重）
        if env::var("BOOT_FLOW").is_ok() {
            let mut flow: Vec<u16> = Vec::new();
            let mut last_pc: u16 = 0;
            let mut started = false;
            let mut traced: u64 = 0;
            unsafe {
                if let Some(ref mut m) = *core::ptr::addr_of_mut!(crate::MACHINE) {
                    while traced < 8_000_000 && flow.len() < 1500 {
                        traced += m.step() as u64;
                        let pc = m.cpu.pc;
                        if !started {
                            if pc == 0x052C || pc == 0x052A {
                                started = true;
                            } else {
                                continue;
                            }
                        }
                        if pc == 0x0593 {
                            // 磁區比對點：高位元組 0xE7 標記 + 實際讀到的磁區，0xEE 標記 + 想要的磁區
                            let ram = crate::apple2_get_ram_ptr();
                            let want_idx = *ram.add(0x026E) as usize;
                            flow.push(0xE700 | *ram.add(0xE7) as u16);
                            flow.push(0xEE00 | *ram.add(0x027E + want_idx) as u16);
                        }
                        // 其他關鍵點：0x052C=外圈起點 0x0537=重試 0x0599=資料讀取後
                        if pc == 0x052C || pc == 0x0599 {
                            flow.push(pc);
                        }
                        last_pc = pc;
                        let _ = last_pc;
                        if m.mem.disk2.needs_reload {
                            let t = m.mem.disk2.current_track as u8;
                            load_track(&dsk, t);
                            flow.push(0xFFFF); // 標記 track reload
                        }
                    }
                }
            }
            println!(
                "flow trace ({}): {:?}",
                flow.len(),
                flow.iter().map(|p| std::format!("{:04X}", p)).collect::<Vec<_>>()
            );
        }

        // BOOT_TRACE=1: 卡死後逐指令追蹤自製 RWTS 的關鍵出入口
        if env::var("BOOT_TRACE").is_ok() {
            let mut fail_037e = 0u32;
            let mut ok_03dc = 0u32;
            let mut fields: Vec<(u8, u8, u8)> = Vec::new();
            let mut traced: u64 = 0;
            let mut full_hist: HashMap<u16, u64> = HashMap::new();
            unsafe {
                if let Some(ref mut m) = *core::ptr::addr_of_mut!(crate::MACHINE) {
                    while traced < 3_000_000 {
                        traced += m.step() as u64;
                        *full_hist.entry(m.cpu.pc).or_insert(0) += 1;
                        match m.cpu.pc {
                            0x037E => fail_037e += 1,
                            0x03DC => {
                                ok_03dc += 1;
                                if fields.len() < 24 {
                                    let ram = crate::apple2_get_ram_ptr();
                                    // $E9=vol $E8=trk $E7=sec
                                    fields.push((*ram.add(0xE9), *ram.add(0xE8), *ram.add(0xE7)));
                                }
                            }
                            _ => {}
                        }
                        if m.mem.disk2.needs_reload {
                            let t = m.mem.disk2.current_track as u8;
                            load_track(&dsk, t);
                            println!("  [trace] track reload T{}", t);
                        }
                    }
                }
            }
            println!(
                "trace(3s): addr-field fail(037E)={} ok(03DC)={} fields(vol,trk,sec)={:?}",
                fail_037e,
                ok_03dc,
                fields.iter().map(|(v, t, s)| std::format!("{:02X}/{:02X}/{:02X}", v, t, s)).collect::<Vec<_>>()
            );
            let mut hot2: Vec<(u16, u64)> = full_hist.into_iter().collect();
            hot2.sort_by(|a, b| b.1.cmp(&a.1));
            println!(
                "trace full PC hist top30: {:?}",
                hot2.iter().take(30).map(|(pc, n)| std::format!("{:04X}x{}", pc, n)).collect::<Vec<_>>()
            );
        }

        println!(
            "qtr-track moves ({}): {:?}",
            qtr_moves.len(),
            qtr_moves
                .iter()
                .rev()
                .take(60)
                .rev()
                .map(|(q, c)| std::format!("{}@{:.2}s", q, *c as f64 / 1_020_500.0))
                .collect::<Vec<_>>()
        );
        println!("text screen:");
        dump_text_screen();

        // BOOT_DUMP=起始HEX,長度HEX: dump 任意 RAM 範圍
        if let Ok(spec) = env::var("BOOT_DUMP") {
            let parts: Vec<usize> = spec.split(',').filter_map(|s| usize::from_str_radix(s, 16).ok()).collect();
            if parts.len() == 2 {
                let ram = crate::apple2_get_ram_ptr();
                let mut dump = String::new();
                for i in 0..parts[1] {
                    if i % 16 == 0 {
                        dump.push_str(&std::format!("\n  {:04X}:", parts[0] + i));
                    }
                    dump.push_str(&std::format!(" {:02X}", unsafe { *ram.add(parts[0] + i) }));
                }
                println!("RAM dump:{}", dump);
            }
        }

        // dump RAM 附近熱點，供反組譯分析卡住原因
        if let Some(&(hot_pc, _)) = hot.first() {
            let ram = crate::apple2_get_ram_ptr();
            let start = (hot_pc as usize).saturating_sub(0x80) & 0xFFF0;
            let mut dump = String::new();
            for i in 0..0x180 {
                if i % 16 == 0 {
                    dump.push_str(&std::format!("\n  {:04X}:", start + i));
                }
                dump.push_str(&std::format!(" {:02X}", unsafe { *ram.add(start + i) }));
            }
            println!("RAM around hot PC {:04X}:{}", hot_pc, dump);
        }
    }

    /// 效能基準：MASTER.DSK 開機後測量主機端模擬吞吐量（emulated MHz）。
    /// 跑法：cargo test --release bench_throughput -- --nocapture --test-threads=1 --ignored
    #[test]
    #[ignore]
    fn bench_throughput() {
        let dsk = fs::read(r"C:\Users\Dell\Downloads\AppleWin1.30.18.0\MASTER.DSK").expect("read dsk");
        crate::apple2_init();
        load_track(&dsk, 0);

        // 暖機：跑完 DOS 開機（含磁碟活動）
        let mut cycles: u64 = 0;
        while cycles < 12 * 1_020_500 {
            cycles += crate::apple2_tick() as u64;
            let t = crate::apple2_needs_disk_reload();
            if t >= 0 {
                load_track(&dsk, t as u8);
            }
        }

        // 正式量測：純 CPU 迴圈（BASIC 提示符下的 KEYIN 輪詢）
        const TARGET: u64 = 200_000_000;
        let start = std::time::Instant::now();
        let mut bench_cycles: u64 = 0;
        while bench_cycles < TARGET {
            bench_cycles += crate::apple2_tick() as u64;
        }
        let dt = start.elapsed();
        let mhz = bench_cycles as f64 / dt.as_secs_f64() / 1_000_000.0;
        println!(
            "bench: {} cycles in {:.3}s -> {:.1} emulated MHz ({:.0}x realtime)",
            bench_cycles,
            dt.as_secs_f64(),
            mhz,
            mhz / 1.0205
        );
    }

    /// 音訊時序分析：開機到 BASIC 提示符後送 Ctrl-G 觸發 ROM BELL（規格 1kHz / 0.1s），
    /// 檢查喇叭翻轉在「模擬週期域」的間隔是否穩定且接近 1kHz。
    /// 跑法：cargo test --release bell_timing -- --nocapture --test-threads=1 --ignored
    #[test]
    #[ignore]
    fn bell_timing() {
        let dsk = fs::read(r"C:\Users\Dell\Downloads\AppleWin1.30.18.0\MASTER.DSK").expect("read dsk");
        crate::apple2_init();
        load_track(&dsk, 0);

        // 開機到 ] 提示符
        let mut cycles: u64 = 0;
        while cycles < 14 * 1_020_500 {
            cycles += crate::apple2_tick() as u64;
            let t = crate::apple2_needs_disk_reload();
            if t >= 0 {
                load_track(&dsk, t as u8);
            }
        }

        // 清空殘留翻轉，送 Ctrl-G（觸發 ROM BELL），跑 0.5 秒
        let mut drain = 0u32;
        while crate::apple2_audio_peek(&mut drain) {
            crate::apple2_audio_drop();
        }
        crate::apple2_handle_key(0x07);
        let mut bell_cycles: u64 = 0;
        while bell_cycles < 510_250 {
            bell_cycles += crate::apple2_tick() as u64;
        }

        // 經由正式的 FFI 環形緩衝取出時間戳（與韌體重放端相同路徑）
        let mut stamps: Vec<u64> = Vec::new();
        let mut c = 0u32;
        while crate::apple2_audio_peek(&mut c) {
            crate::apple2_audio_drop();
            stamps.push(c as u64);
        }
        let count = stamps.len();
        assert!(count > 10, "BELL 應產生大量喇叭翻轉，實際 {}", count);

        let mut intervals: Vec<u64> = stamps.windows(2).map(|w| w[1] - w[0]).collect();
        let first = intervals.first().copied().unwrap_or(0);
        let min = intervals.iter().min().copied().unwrap_or(0);
        let max = intervals.iter().max().copied().unwrap_or(0);
        let mean = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
        intervals.sort_unstable();
        let median = intervals[intervals.len() / 2];
        let freq = 1_020_500.0 / (mean * 2.0);
        let duration_ms = (stamps[count - 1] - stamps[0]) as f64 / 1020.5;

        println!(
            "BELL: {} toggles over {:.1} ms; half-period cycles: first={} min={} median={} max={} mean={:.1} -> {:.0} Hz square wave",
            count, duration_ms, first, min, median, max, mean, freq
        );
        // 規格：~1kHz、0.1 秒。模擬週期域的間隔應穩定（抖動僅來自 WAIT 引數粒度）
        assert!((800.0..1300.0).contains(&freq), "BELL 頻率偏離規格: {:.0} Hz", freq);
        assert!((70.0..160.0).contains(&duration_ms), "BELL 長度偏離規格: {:.1} ms", duration_ms);
    }

    #[test]
    fn boot_smoke() {
        if let Ok(p) = env::var("BOOT_DSK") {
            let secs = env::var("BOOT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30);
            boot_disk(&p, secs);
            return;
        }
        boot_disk(r"C:\Users\Dell\Downloads\AppleWin1.30.18.0\MASTER.DSK", 15);
    }
}
