// Floating bus 回歸測試。
//
// 注意:本核心的 memory_test.rs / cpu_test.rs 等檔案未被 lib.rs `mod` 進去
// (且已與現行 API 脫節無法編譯),故 floating bus 測試獨立放在這個會被
// 編譯與執行的模組中。
#[cfg(test)]
mod tests {
    use crate::memory::{Apple2Memory, Memory};

    #[test]
    fn undriven_io_reads_track_the_floating_bus() {
        let mut mem = Apple2Memory::new();

        // 把每個 RAM 格標上自己的位址低位元組,於是 floating bus 讀取會回傳
        // 影像掃描器這個 cycle 指向的位址的低位元組。
        unsafe {
            let ram = &mut *core::ptr::addr_of_mut!(crate::RAM_48K);
            for (i, cell) in ram.iter_mut().enumerate() {
                *cell = i as u8;
            }
        }

        // 隨匯流排 cycle 推進,逐次讀取未驅動軟開關 ($C055 = Page 2 select)。
        // 常數 0 的舊行為每次回傳相同值;floating bus 必須隨掃描器移動而變動。
        mem.begin_cpu_step(0);
        let first = mem.read(0xC055);
        let mut varied = false;
        let mut any_nonzero = first != 0;
        for _ in 0..199 {
            let v = mem.read(0xC055);
            if v != first { varied = true; }
            if v != 0 { any_nonzero = true; }
        }
        mem.end_cpu_step();

        // 回傳值必須隨 cycle 變動(常數 stub 會讓此失敗)。
        assert!(varied, "floating bus 回傳常數 ({:#04x});亂數源已死", first);
        // 且必須讀到真正的 RAM,不是寫死的哨兵值。
        assert!(any_nonzero, "floating bus 未讀到真正的 RAM");
    }

    // $C010 在 II/II+ 上沒有資料驅動器(回傳 AKD 是 //e 的行為)。讀取只清 strobe,
    // 值來自 floating bus。曾經回傳 keyboard_latch,讓以此為亂數/計時源的程式取到定值。
    #[test]
    fn kbdstrobe_read_clears_strobe_and_returns_floating_bus() {
        let mut mem = Apple2Memory::new();

        unsafe {
            let ram = &mut *core::ptr::addr_of_mut!(crate::RAM_48K);
            for (i, cell) in ram.iter_mut().enumerate() {
                *cell = i as u8;
            }
        }

        mem.begin_cpu_step(0);

        // 放一個帶 strobe 的按鍵:'A' | 0x80。
        mem.keyboard_latch = 0xC1;
        assert_eq!(mem.read(0xC000), 0xC1, "$C000 必須回傳 latch 原值");

        let v = mem.read(0xC010);
        assert_eq!(mem.keyboard_latch, 0x41, "讀 $C010 必須清掉 strobe 位元");
        assert_ne!(v, 0xC1, "$C010 回傳了 latch,而非 floating bus");

        // 值必須隨掃描器 cycle 變動,證明真的走 floating bus。
        let mut varied = false;
        for _ in 0..199 {
            if mem.read(0xC010) != v { varied = true; }
        }
        mem.end_cpu_step();
        assert!(varied, "$C010 回傳常數 ({:#04x});未接上 floating bus", v);
    }
}
