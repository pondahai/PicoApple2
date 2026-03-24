#[cfg(test)]
mod tests {
    use crate::cpu::CPU;
    use crate::memory::Memory;

    struct TestMemory {
        ram: [u8; 65536],
        reads: Vec<u16>,
    }

    impl TestMemory {
        fn new() -> Self {
            Self {
                ram: [0; 65536],
                reads: Vec::new(),
            }
        }
    }

    impl Memory for TestMemory {
        fn read(&mut self, addr: u16) -> u8 {
            self.reads.push(addr);
            self.ram[addr as usize]
        }
        fn write(&mut self, addr: u16, data: u8) {
            self.ram[addr as usize] = data;
        }
    }

    #[test]
    fn test_adc_binary() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        // ADC #$10 (0xA9 0x10)
        cpu.a = 0x20;
        cpu.status.c = false;
        mem.ram[0] = 0x69;
        mem.ram[1] = 0x10;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x30);
        assert_eq!(cpu.status.c, false);
        assert_eq!(cpu.status.z, false);
    }

    #[test]
    fn test_adc_carry() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xFF;
        cpu.status.c = false;
        mem.ram[0] = 0x69;
        mem.ram[1] = 0x01;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.status.c, true);
        assert_eq!(cpu.status.z, true);
    }

    #[test]
    fn test_adc_decimal_mode() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0x15;
        cpu.status.c = false;
        cpu.status.d = true;
        mem.ram[0] = 0x69;
        mem.ram[1] = 0x27;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x42);
        assert!(!cpu.status.c);
    }

    #[test]
    fn test_adc_decimal_mode_sets_carry() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0x58;
        cpu.status.c = false;
        cpu.status.d = true;
        mem.ram[0] = 0x69;
        mem.ram[1] = 0x75;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x33);
        assert!(cpu.status.c);
    }

    #[test]
    fn test_sbc_binary() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        // SBC #$01 (0xE9 0x01)
        cpu.a = 0x10;
        cpu.status.c = true; // No borrow
        mem.ram[0] = 0xE9;
        mem.ram[1] = 0x01;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x0F);
        assert_eq!(cpu.status.c, true); // No borrow occurred
    }

    #[test]
    fn test_sbc_unofficial_immediate_alias_eb() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0x10;
        cpu.status.c = true;
        mem.ram[0] = 0xEB;
        mem.ram[1] = 0x01;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x0F);
        assert!(cpu.status.c);
    }

    #[test]
    fn test_sbc_decimal_mode() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0x50;
        cpu.status.c = true;
        cpu.status.d = true;
        mem.ram[0] = 0xE9;
        mem.ram[1] = 0x15;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x35);
        assert!(cpu.status.c);
    }

    #[test]
    fn test_sbc_decimal_mode_with_borrow() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0x10;
        cpu.status.c = true;
        cpu.status.d = true;
        mem.ram[0] = 0xE9;
        mem.ram[1] = 0x11;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x99);
        assert!(!cpu.status.c);
    }

    #[test]
    fn test_brk_pc_offset() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        // BRK at $1000, signature byte $AA at $1001
        cpu.pc = 0x1000;
        mem.ram[0x1000] = 0x00;
        mem.ram[0x1001] = 0xAA;

        // Interrupt vector at $FFFE
        mem.ram[0xFFFE] = 0xAD;
        mem.ram[0xFFFF] = 0xDE;

        cpu.sp = 0xFF;
        cpu.step(&mut mem);

        // PC should be at the vector
        assert_eq!(cpu.pc, 0xDEAD);

        // Stack should have pushed PC+2 ($1002)
        // SP starts at 0xFF. Pushes hi ($10) to 0x01FF, then lo ($02) to 0x01FE.
        let hi = mem.ram[0x01FF];
        let lo = mem.ram[0x01FE];
        let pushed_pc = ((hi as u16) << 8) | lo as u16;
        assert_eq!(pushed_pc, 0x1002);

        // Status should have B flag set on stack
        let pushed_status = mem.ram[0x01FD];
        assert_eq!(pushed_status & 0x10, 0x10);
        assert_eq!(pushed_status & 0x20, 0x20); // Unused flag
    }

    #[test]
    fn test_anc_sets_carry_from_negative() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xF0;
        mem.ram[0] = 0x0B;
        mem.ram[1] = 0x80;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x80);
        assert!(cpu.status.n);
        assert!(cpu.status.c);
    }

    #[test]
    fn test_alr_shifts_and_sets_carry() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xFF;
        mem.ram[0] = 0x4B;
        mem.ram[1] = 0x03;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x01);
        assert!(cpu.status.c);
        assert!(!cpu.status.n);
    }

    #[test]
    fn test_arr_updates_carry_and_overflow() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xFF;
        cpu.status.c = false;
        mem.ram[0] = 0x6B;
        mem.ram[1] = 0x60;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x30);
        assert!(!cpu.status.c);
        assert!(cpu.status.v);
    }

    #[test]
    fn test_axs_stores_result_in_x() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xCC;
        cpu.x = 0xAA;
        mem.ram[0] = 0xCB;
        mem.ram[1] = 0x88;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.x, 0x00);
        assert!(cpu.status.c);
        assert!(cpu.status.z);
    }

    #[test]
    fn test_kil_jams_cpu_until_reset() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        mem.ram[0] = 0x02;
        cpu.pc = 0;

        assert_eq!(cpu.step(&mut mem), 1);
        assert!(cpu.jammed);
        assert_eq!(cpu.pc, 0);

        mem.ram[0] = 0xEA;
        assert_eq!(cpu.step(&mut mem), 1);
        assert_eq!(cpu.pc, 0);
    }

    #[test]
    fn test_las_loads_a_x_and_sp() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.sp = 0xF0;
        mem.ram[0] = 0xBB;
        mem.ram[1] = 0x34;
        mem.ram[2] = 0x12;
        mem.ram[0x1235] = 0xCC;
        cpu.y = 0x01;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0xC0);
        assert_eq!(cpu.x, 0xC0);
        assert_eq!(cpu.sp, 0xC0);
    }

    #[test]
    fn test_xaa_uses_deterministic_magic_constant_model() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xFF;
        cpu.x = 0x3C;
        mem.ram[0] = 0x8B;
        mem.ram[1] = 0x0F;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x0C);
        assert!(!cpu.status.z);
        assert!(!cpu.status.n);
    }

    #[test]
    fn test_ahx_stores_a_and_x_and_high_plus_one() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xFF;
        cpu.x = 0xAA;
        cpu.y = 0x01;
        mem.ram[0] = 0x9F;
        mem.ram[1] = 0x34;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.ram[0x1235], 0x02);
    }

    #[test]
    fn test_ahx_page_cross_replaces_high_address_byte_with_stored_value() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xFF;
        cpu.x = 0x0F;
        cpu.y = 0x01;
        mem.ram[0] = 0x9F;
        mem.ram[1] = 0xFF;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.ram[0x0300], 0x03);
        assert_eq!(mem.ram[0x1300], 0x00);
    }

    #[test]
    fn test_shy_stores_y_masked_by_high_plus_one() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.x = 0x01;
        cpu.y = 0xFF;
        mem.ram[0] = 0x9C;
        mem.ram[1] = 0x34;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.ram[0x1235], 0x13);
    }

    #[test]
    fn test_shy_page_cross_replaces_high_address_byte_with_stored_value() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.x = 0x01;
        cpu.y = 0x0F;
        mem.ram[0] = 0x9C;
        mem.ram[1] = 0xFF;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.ram[0x0300], 0x03);
        assert_eq!(mem.ram[0x1300], 0x00);
    }

    #[test]
    fn test_shx_stores_x_masked_by_high_plus_one() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.x = 0xFF;
        cpu.y = 0x01;
        mem.ram[0] = 0x9E;
        mem.ram[1] = 0x34;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.ram[0x1235], 0x13);
    }

    #[test]
    fn test_shx_page_cross_replaces_high_address_byte_with_stored_value() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.x = 0x0F;
        cpu.y = 0x01;
        mem.ram[0] = 0x9E;
        mem.ram[1] = 0xFF;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.ram[0x0300], 0x03);
        assert_eq!(mem.ram[0x1300], 0x00);
    }

    #[test]
    fn test_tas_updates_sp_and_stores_masked_value() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xF0;
        cpu.x = 0xCC;
        cpu.y = 0x01;
        mem.ram[0] = 0x9B;
        mem.ram[1] = 0x34;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.sp, 0xC0);
        assert_eq!(mem.ram[0x1235], 0x00);
    }

    #[test]
    fn test_tas_page_cross_uses_stored_value_as_high_address_byte() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.a = 0xF0;
        cpu.x = 0x0F;
        cpu.y = 0x01;
        mem.ram[0] = 0x9B;
        mem.ram[1] = 0xFF;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(cpu.sp, 0x00);
        assert_eq!(mem.ram[0x0000], 0x00);
        assert_eq!(mem.ram[0x1300], 0x00);
    }

    #[test]
    fn test_nop_zpx_performs_base_and_indexed_dummy_reads() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.x = 0x05;
        mem.ram[0] = 0x14;
        mem.ram[1] = 0x10;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.reads, vec![0x0000, 0x0001, 0x0010, 0x0015]);
    }

    #[test]
    fn test_nop_absx_page_cross_reads_wrong_then_final_address() {
        let mut cpu = CPU::new();
        let mut mem = TestMemory::new();

        cpu.x = 0x01;
        mem.ram[0] = 0x1C;
        mem.ram[1] = 0xFF;
        mem.ram[2] = 0x12;
        cpu.pc = 0;

        cpu.step(&mut mem);
        assert_eq!(mem.reads, vec![0x0000, 0x0001, 0x0002, 0x1200, 0x1300]);
    }
}
