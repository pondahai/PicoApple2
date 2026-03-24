#[cfg(test)]
mod tests {
    use crate::memory::{Apple2Memory, Memory};

    #[test]
    fn speaker_toggle_records_bus_cycle() {
        let mut mem = Apple2Memory::new();

        mem.begin_cpu_step(100);
        let _ = mem.read(0xC030);
        mem.end_cpu_step();

        assert!(mem.speaker);
        assert_eq!(mem.take_speaker_toggle_cycles(), vec![100]);
    }

    #[test]
    fn speaker_toggle_cycle_advances_with_each_bus_access() {
        let mut mem = Apple2Memory::new();

        mem.begin_cpu_step(42);
        let _ = mem.read(0x0000);
        let _ = mem.read(0xC030);
        mem.write(0x0001, 0xAA);
        mem.write(0xC030, 0x00);
        mem.end_cpu_step();

        assert!(!mem.speaker);
        assert_eq!(mem.take_speaker_toggle_cycles(), vec![43, 45]);
    }

    #[test]
    fn pushbutton_reads_set_bit7_when_pressed() {
        let mut mem = Apple2Memory::new();
        mem.set_joystick_state(127, 127, true, false);

        mem.begin_cpu_step(0);
        let pb0 = mem.read(0xC061);
        let pb1 = mem.read(0xC062);
        mem.end_cpu_step();

        assert_eq!(pb0, 0x80);
        assert_eq!(pb1, 0x00);
    }

    #[test]
    fn paddle_reads_stay_high_until_timeout_after_strobe() {
        let mut mem = Apple2Memory::new();
        mem.set_joystick_state(255, 0, false, false);

        mem.begin_cpu_step(100);
        let _ = mem.read(0xC070);
        let early = mem.read(0xC064);
        mem.end_cpu_step();

        mem.begin_cpu_step(3_000);
        let late = mem.read(0xC064);
        mem.end_cpu_step();

        assert_eq!(early, 0x80);
        assert_eq!(late, 0x00);
    }

    #[test]
    fn language_card_requires_double_read_on_same_canonical_switch_to_enable_writes() {
        let mut mem = Apple2Memory::new();

        mem.begin_cpu_step(0);
        let _ = mem.read(0xC081);
        assert!(!mem.lc_write_enable);
        let _ = mem.read(0xC085);
        mem.end_cpu_step();

        assert!(mem.lc_write_enable);
    }

    #[test]
    fn language_card_bank1_and_bank2_are_distinct_in_d000_window() {
        let mut mem = Apple2Memory::new();

        mem.begin_cpu_step(0);
        let _ = mem.read(0xC081);
        let _ = mem.read(0xC081);
        mem.write(0xD000, 0x11);

        let _ = mem.read(0xC089);
        let _ = mem.read(0xC089);
        mem.write(0xD000, 0x22);

        let _ = mem.read(0xC083);
        let bank1_val = mem.read(0xD000);
        let _ = mem.read(0xC08B);
        let bank2_val = mem.read(0xD000);
        mem.end_cpu_step();

        assert_eq!(bank1_val, 0x11);
        assert_eq!(bank2_val, 0x22);
    }

    #[test]
    fn language_card_e000_window_is_shared_between_banks() {
        let mut mem = Apple2Memory::new();

        mem.begin_cpu_step(0);
        let _ = mem.read(0xC081);
        let _ = mem.read(0xC081);
        mem.write(0xE000, 0x33);

        let _ = mem.read(0xC08B);
        let shared_val = mem.read(0xE000);
        mem.end_cpu_step();

        assert_eq!(shared_val, 0x33);
    }

    #[test]
    fn language_card_can_read_rom_while_writing_ram() {
        let mut mem = Apple2Memory::new();
        mem.rom[0] = 0xAA;

        mem.begin_cpu_step(0);
        let _ = mem.read(0xC081);
        let _ = mem.read(0xC081);
        mem.write(0xD000, 0x44);

        let _ = mem.read(0xC082);
        let rom_val = mem.read(0xD000);
        mem.end_cpu_step();

        assert_eq!(rom_val, 0xAA);
        assert_eq!(mem.lc_ram[0x1000], 0x44);
    }
}
