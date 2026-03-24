use crate::memory::Memory;

/// Status Flags for the 6502 CPU (P Register)
#[derive(Clone, Copy, Debug)]
pub struct StatusFlags {
    pub c: bool, // Carry
    pub z: bool, // Zero
    pub i: bool, // Interrupt Disable
    pub d: bool, // Decimal Mode
    pub b: bool, // Break Command
    pub u: bool, // Unused (always 1)
    pub v: bool, // Overflow
    pub n: bool, // Negative
}

impl Default for StatusFlags {
    fn default() -> Self {
        Self {
            c: false,
            z: false,
            i: true, // Interrupt disabled by default on reset
            d: false,
            b: false,
            u: true, // Always true
            v: false,
            n: false,
        }
    }
}

impl StatusFlags {
    pub fn to_byte(&self) -> u8 {
        (if self.c { 1 } else { 0 })
            | (if self.z { 1 << 1 } else { 0 })
            | (if self.i { 1 << 2 } else { 0 })
            | (if self.d { 1 << 3 } else { 0 })
            | (if self.b { 1 << 4 } else { 0 })
            | (if self.u { 1 << 5 } else { 0 })
            | (if self.v { 1 << 6 } else { 0 })
            | (if self.n { 1 << 7 } else { 0 })
    }

    pub fn from_byte(&mut self, b: u8) {
        self.c = (b & 1) != 0;
        self.z = (b & (1 << 1)) != 0;
        self.i = (b & (1 << 2)) != 0;
        self.d = (b & (1 << 3)) != 0;
        self.b = (b & (1 << 4)) != 0;
        self.u = true; // Always 1
        self.v = (b & (1 << 6)) != 0;
        self.n = (b & (1 << 7)) != 0;
    }
}

pub struct CPU {
    pub a: u8,               // Accumulator
    pub x: u8,               // X Register
    pub y: u8,               // Y Register
    pub pc: u16,             // Program Counter
    pub sp: u8,              // Stack Pointer
    pub status: StatusFlags, // P Register (Processor Status)
    pub jammed: bool,        // NMOS KIL/JAM state
}

impl CPU {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            sp: 0xFD,
            status: StatusFlags::default(),
            jammed: false,
        }
    }

    pub fn reset<M: Memory>(&mut self, mem: &mut M) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = StatusFlags::default();
        self.jammed = false;
        // Load Reset Vector from $FFFC / $FFFD
        self.pc = mem.read_word(0xFFFC);
    }

    /// Read a byte from memory and increment PC
    pub(crate) fn fetch_byte<M: Memory>(&mut self, mem: &mut M) -> u8 {
        let result = mem.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        result
    }

    /// Read a word from memory and increment PC by 2
    pub(crate) fn fetch_word<M: Memory>(&mut self, mem: &mut M) -> u16 {
        let result = mem.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        result
    }

    /// Calculate the target address based on addressing mode
    pub(crate) fn get_operand_address<M: Memory>(
        &mut self,
        mem: &mut M,
        mode: AddressingMode,
    ) -> (u16, bool) {
        match mode {
            AddressingMode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                (addr, false)
            }
            AddressingMode::ZeroPage => (self.fetch_byte(mem) as u16, false),
            AddressingMode::ZeroPageX => {
                let base = self.fetch_byte(mem);
                (base.wrapping_add(self.x) as u16, false)
            }
            AddressingMode::ZeroPageY => {
                let base = self.fetch_byte(mem);
                (base.wrapping_add(self.y) as u16, false)
            }
            AddressingMode::Absolute => (self.fetch_word(mem), false),
            AddressingMode::AbsoluteX => {
                let base = self.fetch_word(mem);
                let addr = base.wrapping_add(self.x as u16);
                (addr, (base & 0xFF00) != (addr & 0xFF00))
            }
            AddressingMode::AbsoluteY => {
                let base = self.fetch_word(mem);
                let addr = base.wrapping_add(self.y as u16);
                (addr, (base & 0xFF00) != (addr & 0xFF00))
            }
            AddressingMode::Indirect => {
                let ptr = self.fetch_word(mem);
                let lo = mem.read(ptr) as u16;
                let hi_ptr = if ptr & 0x00FF == 0x00FF {
                    ptr & 0xFF00
                } else {
                    ptr + 1
                };
                let hi = mem.read(hi_ptr) as u16;
                ((hi << 8) | lo, false)
            }
            AddressingMode::IndirectX => {
                let base = self.fetch_byte(mem);
                let ptr = base.wrapping_add(self.x);
                let lo = mem.read(ptr as u16) as u16;
                let hi = mem.read(ptr.wrapping_add(1) as u16) as u16;
                ((hi << 8) | lo, false)
            }
            AddressingMode::IndirectY => {
                let base = self.fetch_byte(mem);
                let lo = mem.read(base as u16) as u16;
                let hi = mem.read(base.wrapping_add(1) as u16) as u16;
                let deref_base = (hi << 8) | lo;
                let addr = deref_base.wrapping_add(self.y as u16);
                (addr, (deref_base & 0xFF00) != (addr & 0xFF00))
            }
            AddressingMode::NoneAddressing => (0, false),
            AddressingMode::Relative => (0, false),
        }
    }

    /// Step one instruction
    pub fn step<M: Memory>(&mut self, mem: &mut M) -> u32 {
        if self.jammed {
            // KIL/JAM halts the NMOS 6502 until reset. Keep returning a
            // non-zero cycle count so the emulator does not spin forever.
            return 1;
        }

        let opcode = self.fetch_byte(mem);
        let mut extra_cycles = 0;

        let cycles = match opcode {
            // BRK
            0x00 => {
                let push_pc = self.pc.wrapping_add(1);
                self.stack_push(mem, (push_pc >> 8) as u8);
                self.stack_push(mem, (push_pc & 0xFF) as u8);
                self.stack_push(mem, self.status.to_byte() | 0x30);
                self.status.i = true;
                self.pc = mem.read_word(0xFFFE);
                7
            }

            // LDA
            0xA9 => {
                self.lda(mem, AddressingMode::Immediate);
                2
            }
            0xA5 => {
                self.lda(mem, AddressingMode::ZeroPage);
                3
            }
            0xB5 => {
                self.lda(mem, AddressingMode::ZeroPageX);
                4
            }
            0xAD => {
                self.lda(mem, AddressingMode::Absolute);
                4
            }
            0xBD => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.lda_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0xB9 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.lda_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0xA1 => {
                self.lda(mem, AddressingMode::IndirectX);
                6
            }
            0xB1 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.lda_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                5
            }

            // LDX
            0xA2 => {
                self.ldx(mem, AddressingMode::Immediate);
                2
            }
            0xA6 => {
                self.ldx(mem, AddressingMode::ZeroPage);
                3
            }
            0xB6 => {
                self.ldx(mem, AddressingMode::ZeroPageY);
                4
            }
            0xAE => {
                self.ldx(mem, AddressingMode::Absolute);
                4
            }
            0xBE => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.ldx_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }

            // LDY
            0xA0 => {
                self.ldy(mem, AddressingMode::Immediate);
                2
            }
            0xA4 => {
                self.ldy(mem, AddressingMode::ZeroPage);
                3
            }
            0xB4 => {
                self.ldy(mem, AddressingMode::ZeroPageX);
                4
            }
            0xAC => {
                self.ldy(mem, AddressingMode::Absolute);
                4
            }
            0xBC => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.ldy_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }

            // STA
            0x85 => {
                self.sta(mem, AddressingMode::ZeroPage);
                3
            }
            0x95 => {
                self.sta(mem, AddressingMode::ZeroPageX);
                4
            }
            0x8D => {
                self.sta(mem, AddressingMode::Absolute);
                4
            }
            0x9D => {
                self.sta(mem, AddressingMode::AbsoluteX);
                5
            }
            0x99 => {
                self.sta(mem, AddressingMode::AbsoluteY);
                5
            }
            0x81 => {
                self.sta(mem, AddressingMode::IndirectX);
                6
            }
            0x91 => {
                self.sta(mem, AddressingMode::IndirectY);
                6
            }

            // STX
            0x86 => {
                self.stx(mem, AddressingMode::ZeroPage);
                3
            }
            0x96 => {
                self.stx(mem, AddressingMode::ZeroPageY);
                4
            }
            0x8E => {
                self.stx(mem, AddressingMode::Absolute);
                4
            }

            // STY
            0x84 => {
                self.sty(mem, AddressingMode::ZeroPage);
                3
            }
            0x94 => {
                self.sty(mem, AddressingMode::ZeroPageX);
                4
            }
            0x8C => {
                self.sty(mem, AddressingMode::Absolute);
                4
            }

            // JMP
            0x4C => {
                self.jmp(mem, AddressingMode::Absolute);
                3
            }
            0x6C => {
                self.jmp(mem, AddressingMode::Indirect);
                5
            }

            // JSR, RTS, RTI
            0x20 => {
                self.jsr(mem, AddressingMode::Absolute);
                6
            }
            0x60 => {
                self.rts(mem);
                6
            }
            0x40 => {
                self.rti(mem);
                6
            }

            // Branches
            0x90 => {
                let off = self.fetch_byte(mem) as i8;
                let c = !self.status.c;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BCC
            0xB0 => {
                let off = self.fetch_byte(mem) as i8;
                let c = self.status.c;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BCS
            0xF0 => {
                let off = self.fetch_byte(mem) as i8;
                let c = self.status.z;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BEQ
            0xD0 => {
                let off = self.fetch_byte(mem) as i8;
                let c = !self.status.z;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BNE
            0x10 => {
                let off = self.fetch_byte(mem) as i8;
                let c = !self.status.n;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BPL
            0x30 => {
                let off = self.fetch_byte(mem) as i8;
                let c = self.status.n;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BMI
            0x50 => {
                let off = self.fetch_byte(mem) as i8;
                let c = !self.status.v;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BVC
            0x70 => {
                let off = self.fetch_byte(mem) as i8;
                let c = self.status.v;
                extra_cycles = self.branch(c, off);
                if c { 3 } else { 2 }
            } // BVS

            // Math: ADC
            0x69 => {
                self.adc(mem, AddressingMode::Immediate);
                2
            }
            0x65 => {
                self.adc(mem, AddressingMode::ZeroPage);
                3
            }
            0x75 => {
                self.adc(mem, AddressingMode::ZeroPageX);
                4
            }
            0x6D => {
                self.adc(mem, AddressingMode::Absolute);
                4
            }
            0x7D => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.adc_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x79 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.adc_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x61 => {
                self.adc(mem, AddressingMode::IndirectX);
                6
            }
            0x71 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.adc_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                5
            }

            // Math: SBC
            0xE9 | 0xEB => {
                self.sbc(mem, AddressingMode::Immediate);
                2
            }
            0xE5 => {
                self.sbc(mem, AddressingMode::ZeroPage);
                3
            }
            0xF5 => {
                self.sbc(mem, AddressingMode::ZeroPageX);
                4
            }
            0xED => {
                self.sbc(mem, AddressingMode::Absolute);
                4
            }
            0xFD => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.sbc_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0xF9 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.sbc_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0xE1 => {
                self.sbc(mem, AddressingMode::IndirectX);
                6
            }
            0xF1 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.sbc_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                5
            }

            // Logic: AND
            0x29 => {
                self.and(mem, AddressingMode::Immediate);
                2
            }
            0x25 => {
                self.and(mem, AddressingMode::ZeroPage);
                3
            }
            0x35 => {
                self.and(mem, AddressingMode::ZeroPageX);
                4
            }
            0x2D => {
                self.and(mem, AddressingMode::Absolute);
                4
            }
            0x3D => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.and_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x39 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.and_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x21 => {
                self.and(mem, AddressingMode::IndirectX);
                6
            }
            0x31 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.and_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                5
            }

            // Logic: ORA
            0x09 => {
                self.ora(mem, AddressingMode::Immediate);
                2
            }
            0x05 => {
                self.ora(mem, AddressingMode::ZeroPage);
                3
            }
            0x15 => {
                self.ora(mem, AddressingMode::ZeroPageX);
                4
            }
            0x0D => {
                self.ora(mem, AddressingMode::Absolute);
                4
            }
            0x1D => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.ora_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x19 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.ora_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x01 => {
                self.ora(mem, AddressingMode::IndirectX);
                6
            }
            0x11 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.ora_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                5
            }

            // Logic: EOR
            0x49 => {
                self.eor(mem, AddressingMode::Immediate);
                2
            }
            0x45 => {
                self.eor(mem, AddressingMode::ZeroPage);
                3
            }
            0x55 => {
                self.eor(mem, AddressingMode::ZeroPageX);
                4
            }
            0x4D => {
                self.eor(mem, AddressingMode::Absolute);
                4
            }
            0x5D => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.eor_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x59 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.eor_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0x41 => {
                self.eor(mem, AddressingMode::IndirectX);
                6
            }
            0x51 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.eor_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                5
            }

            // BIT
            0x24 => {
                self.bit(mem, AddressingMode::ZeroPage);
                3
            }
            0x2C => {
                self.bit(mem, AddressingMode::Absolute);
                4
            }

            // Compares: CMP
            0xC9 => {
                self.cmp(mem, AddressingMode::Immediate);
                2
            }
            0xC5 => {
                self.cmp(mem, AddressingMode::ZeroPage);
                3
            }
            0xD5 => {
                self.cmp(mem, AddressingMode::ZeroPageX);
                4
            }
            0xCD => {
                self.cmp(mem, AddressingMode::Absolute);
                4
            }
            0xDD => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.cmp_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0xD9 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.cmp_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0xC1 => {
                self.cmp(mem, AddressingMode::IndirectX);
                6
            }
            0xD1 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.cmp_with_addr(mem, addr);
                if p {
                    extra_cycles += 1;
                }
                5
            }

            // Compares: CPX
            0xE0 => {
                self.cpx(mem, AddressingMode::Immediate);
                2
            }
            0xE4 => {
                self.cpx(mem, AddressingMode::ZeroPage);
                3
            }
            0xEC => {
                self.cpx(mem, AddressingMode::Absolute);
                4
            }

            // Compares: CPY
            0xC0 => {
                self.cpy(mem, AddressingMode::Immediate);
                2
            }
            0xC4 => {
                self.cpy(mem, AddressingMode::ZeroPage);
                3
            }
            0xCC => {
                self.cpy(mem, AddressingMode::Absolute);
                4
            }

            // Increments/Decrements
            0xE6 => {
                self.inc(mem, AddressingMode::ZeroPage);
                5
            }
            0xF6 => {
                self.inc(mem, AddressingMode::ZeroPageX);
                6
            }
            0xEE => {
                self.inc(mem, AddressingMode::Absolute);
                6
            }
            0xFE => {
                self.inc(mem, AddressingMode::AbsoluteX);
                7
            }

            0xC6 => {
                self.dec(mem, AddressingMode::ZeroPage);
                5
            }
            0xD6 => {
                self.dec(mem, AddressingMode::ZeroPageX);
                6
            }
            0xCE => {
                self.dec(mem, AddressingMode::Absolute);
                6
            }
            0xDE => {
                self.dec(mem, AddressingMode::AbsoluteX);
                7
            }

            0xE8 => {
                self.inx();
                2
            }
            0xC8 => {
                self.iny();
                2
            }
            0xCA => {
                self.dex();
                2
            }
            0x88 => {
                self.dey();
                2
            }

            // Shifts/Rotates: ASL
            0x0A => {
                self.asl_acc();
                2
            }
            0x06 => {
                self.asl(mem, AddressingMode::ZeroPage);
                5
            }
            0x16 => {
                self.asl(mem, AddressingMode::ZeroPageX);
                6
            }
            0x0E => {
                self.asl(mem, AddressingMode::Absolute);
                6
            }
            0x1E => {
                self.asl(mem, AddressingMode::AbsoluteX);
                7
            }

            // Shifts/Rotates: LSR
            0x4A => {
                self.lsr_acc();
                2
            }
            0x46 => {
                self.lsr(mem, AddressingMode::ZeroPage);
                5
            }
            0x56 => {
                self.lsr(mem, AddressingMode::ZeroPageX);
                6
            }
            0x4E => {
                self.lsr(mem, AddressingMode::Absolute);
                6
            }
            0x5E => {
                self.lsr(mem, AddressingMode::AbsoluteX);
                7
            }

            // Shifts/Rotates: ROL
            0x2A => {
                self.rol_acc();
                2
            }
            0x26 => {
                self.rol(mem, AddressingMode::ZeroPage);
                5
            }
            0x36 => {
                self.rol(mem, AddressingMode::ZeroPageX);
                6
            }
            0x2E => {
                self.rol(mem, AddressingMode::Absolute);
                6
            }
            0x3E => {
                self.rol(mem, AddressingMode::AbsoluteX);
                7
            }

            // Shifts/Rotates: ROR
            0x6A => {
                self.ror_acc();
                2
            }
            0x66 => {
                self.ror(mem, AddressingMode::ZeroPage);
                5
            }
            0x76 => {
                self.ror(mem, AddressingMode::ZeroPageX);
                6
            }
            0x6E => {
                self.ror(mem, AddressingMode::Absolute);
                6
            }
            0x7E => {
                self.ror(mem, AddressingMode::AbsoluteX);
                7
            }

            // Transfers
            0xAA => {
                self.tax();
                2
            }
            0xA8 => {
                self.tay();
                2
            }
            0x8A => {
                self.txa();
                2
            }
            0x98 => {
                self.tya();
                2
            }
            0xBA => {
                self.tsx();
                2
            }
            0x9A => {
                self.txs();
                2
            }

            // Stack
            0x48 => {
                self.pha(mem);
                3
            }
            0x68 => {
                self.pla(mem);
                4
            }
            0x08 => {
                self.php(mem);
                3
            }
            0x28 => {
                self.plp(mem);
                4
            }

            // Flags
            0x18 => {
                self.clc();
                2
            }
            0x38 => {
                self.sec();
                2
            }
            0x58 => {
                self.cli();
                2
            }
            0x78 => {
                self.sei();
                2
            }
            0xB8 => {
                self.clv();
                2
            }
            0xD8 => {
                self.cld();
                2
            }
            0xF8 => {
                self.sed();
                2
            }

            // NOP
            0xEA => {
                self.nop();
                2
            }
            // SKB (Zero Page)
            0x04 | 0x44 | 0x64 => {
                let addr = self.fetch_byte(mem);
                mem.read(addr as u16);
                3
            }
            // SKB (Immediate)
            0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => {
                self.fetch_byte(mem);
                2
            }
            // 3-byte NOPs (SKW: Skip Word)
            0x0C => {
                let addr = self.fetch_word(mem);
                mem.read(addr);
                4
            }
            // Indexed 2-byte NOPs (Zero Page,X)
            0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => {
                let b = self.fetch_byte(mem);
                mem.read(b as u16);
                mem.read(b.wrapping_add(self.x) as u16);
                4
            }
            // Indexed 3-byte NOPs (Absolute,X)
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                let wrong_page = (addr.wrapping_sub(self.x as u16) & 0xFF00)
                    | ((addr as u8).wrapping_sub(self.x).wrapping_add(self.x) as u16);
                mem.read(wrong_page);
                if p {
                    mem.read(addr);
                    extra_cycles += 1;
                }
                4
            }
            // 1-byte variants
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => 2,

            // ANC (Illegal: AND then move N into C)
            0x0B | 0x2B => {
                let val = self.fetch_byte(mem);
                self.anc_with_val(val);
                2
            }

            // ALR (Illegal: AND then LSR A)
            0x4B => {
                let val = self.fetch_byte(mem);
                self.alr_with_val(val);
                2
            }

            // ARR (Illegal: AND then ROR A)
            0x6B => {
                let val = self.fetch_byte(mem);
                self.arr_with_val(val);
                2
            }

            // SBX / AXS (Illegal: (A & X) - imm -> X)
            0xCB => {
                let val = self.fetch_byte(mem);
                self.axs_with_val(val);
                2
            }

            // LAX (Illegal: Load A and X)
            0xAF => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                let val = mem.read(addr);
                self.a = val;
                self.x = val;
                self.update_zero_and_negative_flags(val);
                4
            }
            0xA7 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                let val = mem.read(addr);
                self.a = val;
                self.x = val;
                self.update_zero_and_negative_flags(val);
                3
            }
            0xB7 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageY);
                let val = mem.read(addr);
                self.a = val;
                self.x = val;
                self.update_zero_and_negative_flags(val);
                4
            }
            0xBF => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                let val = mem.read(addr);
                self.a = val;
                self.x = val;
                self.update_zero_and_negative_flags(val);
                if p {
                    extra_cycles += 1;
                }
                4
            }
            0xA3 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                let val = mem.read(addr);
                self.a = val;
                self.x = val;
                self.update_zero_and_negative_flags(val);
                6
            }
            0xB3 => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::IndirectY);
                let val = mem.read(addr);
                self.a = val;
                self.x = val;
                self.update_zero_and_negative_flags(val);
                if p {
                    extra_cycles += 1;
                }
                5
            }
            0xAB => {
                let val = self.fetch_byte(mem);
                self.a = val;
                self.x = val;
                self.update_zero_and_negative_flags(val);
                2
            } // LAX #imm
            0xBB => {
                let (addr, p) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                let val = mem.read(addr) & self.sp;
                self.a = val;
                self.x = val;
                self.sp = val;
                self.update_zero_and_negative_flags(val);
                if p {
                    extra_cycles += 1;
                }
                4
            } // LAS abs,Y

            // XAA / ANE (Illegal, unstable on real NMOS 6502)
            0x8B => {
                let val = self.fetch_byte(mem);
                self.xaa_with_val(val);
                2
            }

            // SAX (Illegal: Store A AND X)
            0x8F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                mem.write(addr, self.a & self.x);
                4
            }
            0x87 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                mem.write(addr, self.a & self.x);
                3
            }
            0x97 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageY);
                mem.write(addr, self.a & self.x);
                4
            }
            0x83 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                mem.write(addr, self.a & self.x);
                6
            }

            // SHA / AHX (Illegal, unstable on real NMOS 6502)
            0x9F => {
                let (base, addr, page_crossed) = self.h_plus_one_absolute_y(mem);
                self.ahx_store(mem, base, addr, self.a & self.x, page_crossed);
                5
            }
            0x93 => {
                let (base, addr, page_crossed) = self.h_plus_one_indirect_y(mem);
                self.ahx_store(mem, base, addr, self.a & self.x, page_crossed);
                6
            }

            // TAS / SHS (Illegal, unstable on real NMOS 6502)
            0x9B => {
                let (base, addr, page_crossed) = self.h_plus_one_absolute_y(mem);
                let val = self.a & self.x;
                self.sp = val;
                self.ahx_store(mem, base, addr, val, page_crossed);
                5
            }

            // SHY / SYA (Illegal, unstable on real NMOS 6502)
            0x9C => {
                let (base, addr, page_crossed) = self.h_plus_one_absolute_x(mem);
                self.h_plus_one_store(mem, base, addr, self.y, page_crossed);
                5
            }

            // SHX / SXA (Illegal, unstable on real NMOS 6502)
            0x9E => {
                let (base, addr, page_crossed) = self.h_plus_one_absolute_y(mem);
                self.h_plus_one_store(mem, base, addr, self.x, page_crossed);
                5
            }

            // DCP (Illegal: DEC then CMP)
            0xCF => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                self.dcp_with_addr(mem, addr);
                6
            }
            0xC7 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                self.dcp_with_addr(mem, addr);
                5
            }
            0xD7 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageX);
                self.dcp_with_addr(mem, addr);
                6
            }
            0xDF => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.dcp_with_addr(mem, addr);
                7
            }
            0xDB => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.dcp_with_addr(mem, addr);
                7
            }
            0xC3 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                self.dcp_with_addr(mem, addr);
                8
            }
            0xD3 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.dcp_with_addr(mem, addr);
                8
            }

            // ISC (Illegal: INC then SBC)
            0xEF => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                self.isc_with_addr(mem, addr);
                6
            }
            0xE7 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                self.isc_with_addr(mem, addr);
                5
            }
            0xF7 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageX);
                self.isc_with_addr(mem, addr);
                6
            }
            0xFF => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.isc_with_addr(mem, addr);
                7
            }
            0xFB => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.isc_with_addr(mem, addr);
                7
            }
            0xE3 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                self.isc_with_addr(mem, addr);
                8
            }
            0xF3 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.isc_with_addr(mem, addr);
                8
            }

            // SLO (Illegal: ASL then ORA)
            0x0F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                self.slo_with_addr(mem, addr);
                6
            }
            0x07 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                self.slo_with_addr(mem, addr);
                5
            }
            0x17 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageX);
                self.slo_with_addr(mem, addr);
                6
            }
            0x1F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.slo_with_addr(mem, addr);
                7
            }
            0x1B => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.slo_with_addr(mem, addr);
                7
            }
            0x03 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                self.slo_with_addr(mem, addr);
                8
            }
            0x13 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.slo_with_addr(mem, addr);
                8
            }

            // RLA (Illegal: ROL then AND)
            0x2F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                self.rla_with_addr(mem, addr);
                6
            }
            0x27 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                self.rla_with_addr(mem, addr);
                5
            }
            0x37 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageX);
                self.rla_with_addr(mem, addr);
                6
            }
            0x3F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.rla_with_addr(mem, addr);
                7
            }
            0x3B => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.rla_with_addr(mem, addr);
                7
            }
            0x23 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                self.rla_with_addr(mem, addr);
                8
            }
            0x33 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.rla_with_addr(mem, addr);
                8
            }

            // SRE (Illegal: LSR then EOR)
            0x4F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                self.sre_with_addr(mem, addr);
                6
            }
            0x47 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                self.sre_with_addr(mem, addr);
                5
            }
            0x57 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageX);
                self.sre_with_addr(mem, addr);
                6
            }
            0x5F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.sre_with_addr(mem, addr);
                7
            }
            0x5B => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.sre_with_addr(mem, addr);
                7
            }
            0x43 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                self.sre_with_addr(mem, addr);
                8
            }
            0x53 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.sre_with_addr(mem, addr);
                8
            }

            // RRA (Illegal: ROR then ADC)
            0x6F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::Absolute);
                self.rra_with_addr(mem, addr);
                6
            }
            0x67 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPage);
                self.rra_with_addr(mem, addr);
                5
            }
            0x77 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::ZeroPageX);
                self.rra_with_addr(mem, addr);
                6
            }
            0x7F => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteX);
                self.rra_with_addr(mem, addr);
                7
            }
            0x7B => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::AbsoluteY);
                self.rra_with_addr(mem, addr);
                7
            }
            0x63 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectX);
                self.rra_with_addr(mem, addr);
                8
            }
            0x73 => {
                let (addr, _) = self.get_operand_address(mem, AddressingMode::IndirectY);
                self.rra_with_addr(mem, addr);
                8
            }

            // KIL / JAM (Illegal: halt until reset)
            0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2
            | 0xD2 | 0xF2 => {
                self.jammed = true;
                self.pc = self.pc.wrapping_sub(1);
                1
            }
        };
        cycles + extra_cycles
    }

    fn anc_with_val(&mut self, value: u8) {
        self.a &= value;
        self.update_zero_and_negative_flags(self.a);
        self.status.c = self.status.n;
    }

    fn alr_with_val(&mut self, value: u8) {
        self.a &= value;
        self.status.c = (self.a & 0x01) != 0;
        self.a >>= 1;
        self.update_zero_and_negative_flags(self.a);
    }

    fn arr_with_val(&mut self, value: u8) {
        self.a &= value;
        let old_c = self.status.c;
        self.a = (self.a >> 1) | (if old_c { 0x80 } else { 0 });
        self.update_zero_and_negative_flags(self.a);
        self.status.c = (self.a & 0x40) != 0;
        self.status.v = ((self.a >> 6) ^ (self.a >> 5)) & 0x01 != 0;
    }

    fn axs_with_val(&mut self, value: u8) {
        let anded = self.a & self.x;
        self.status.c = anded >= value;
        self.x = anded.wrapping_sub(value);
        self.update_zero_and_negative_flags(self.x);
    }

    fn xaa_with_val(&mut self, value: u8) {
        // XAA is hardware-unstable; use the common emulator approximation with
        // a fixed "magic constant" to keep behavior deterministic.
        self.a = (self.a | 0xEE) & self.x & value;
        self.update_zero_and_negative_flags(self.a);
    }

    fn h_plus_one_absolute_x<M: Memory>(&mut self, mem: &mut M) -> (u16, u16, bool) {
        let base = self.fetch_word(mem);
        let addr = base.wrapping_add(self.x as u16);
        (base, addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    fn h_plus_one_absolute_y<M: Memory>(&mut self, mem: &mut M) -> (u16, u16, bool) {
        let base = self.fetch_word(mem);
        let addr = base.wrapping_add(self.y as u16);
        (base, addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    fn h_plus_one_indirect_y<M: Memory>(&mut self, mem: &mut M) -> (u16, u16, bool) {
        let zp = self.fetch_byte(mem);
        let lo = mem.read(zp as u16) as u16;
        let hi = mem.read(zp.wrapping_add(1) as u16) as u16;
        let base = (hi << 8) | lo;
        let addr = base.wrapping_add(self.y as u16);
        (base, addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    fn h_plus_one_mask(base: u16) -> u8 {
        ((base >> 8) as u8).wrapping_add(1)
    }

    fn h_plus_one_write_addr(addr: u16, value: u8, page_crossed: bool) -> u16 {
        if page_crossed {
            ((value as u16) << 8) | (addr & 0x00FF)
        } else {
            addr
        }
    }

    fn h_plus_one_store<M: Memory>(
        &mut self,
        mem: &mut M,
        base: u16,
        addr: u16,
        value: u8,
        page_crossed: bool,
    ) {
        let stored = value & Self::h_plus_one_mask(base);
        let write_addr = Self::h_plus_one_write_addr(addr, stored, page_crossed);
        mem.write(write_addr, stored);
    }

    fn ahx_store<M: Memory>(
        &mut self,
        mem: &mut M,
        base: u16,
        addr: u16,
        value: u8,
        page_crossed: bool,
    ) {
        self.h_plus_one_store(mem, base, addr, value, page_crossed);
    }

    fn dcp_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let val = mem.read(addr).wrapping_sub(1);
        mem.write(addr, val);
        self.compare(self.a, val);
    }

    fn isc_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let val = mem.read(addr).wrapping_add(1);
        mem.write(addr, val);
        self.adc_with_val(!val);
    }

    fn slo_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let mut data = mem.read(addr);
        self.status.c = (data >> 7) == 1;
        data <<= 1;
        mem.write(addr, data);
        self.a |= data;
        self.update_zero_and_negative_flags(self.a);
    }

    fn rla_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let mut data = mem.read(addr);
        let old_c = self.status.c;
        self.status.c = (data >> 7) == 1;
        data = (data << 1) | (if old_c { 1 } else { 0 });
        mem.write(addr, data);
        self.a &= data;
        self.update_zero_and_negative_flags(self.a);
    }

    fn sre_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let mut data = mem.read(addr);
        self.status.c = (data & 1) == 1;
        data >>= 1;
        mem.write(addr, data);
        self.a ^= data;
        self.update_zero_and_negative_flags(self.a);
    }

    fn rra_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let mut data = mem.read(addr);
        let old_c = self.status.c;
        self.status.c = (data & 1) == 1;
        data = (data >> 1) | (if old_c { 0x80 } else { 0 });
        mem.write(addr, data);
        self.adc_with_val(data);
    }
}

pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    NoneAddressing,
    Relative,
}
