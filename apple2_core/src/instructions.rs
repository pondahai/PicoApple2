use crate::cpu::{AddressingMode, CPU};
use crate::memory::Memory;

impl CPU {
    // --- Load/Store ---
    pub(crate) fn lda<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.a = value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn lda_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.a = value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn ldx<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.x = value;
        self.update_zero_and_negative_flags(self.x);
    }

    pub(crate) fn ldx_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.x = value;
        self.update_zero_and_negative_flags(self.x);
    }

    pub(crate) fn ldy<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.y = value;
        self.update_zero_and_negative_flags(self.y);
    }

    pub(crate) fn ldy_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.y = value;
        self.update_zero_and_negative_flags(self.y);
    }

    pub(crate) fn sta<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        mem.write(addr, self.a);
    }

    pub(crate) fn stx<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        mem.write(addr, self.x);
    }

    pub(crate) fn sty<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        mem.write(addr, self.y);
    }

    // --- Arithmetic ---
    pub(crate) fn adc<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.adc_with_val(value);
    }

    pub(crate) fn adc_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.adc_with_val(value);
    }

    pub(crate) fn adc_with_val(&mut self, value: u8) {
        if self.status.d {
            self.adc_decimal(value);
            return;
        }

        let sum = self.a as u16 + value as u16 + (if self.status.c { 1 } else { 0 });
        self.status.c = sum > 0xFF;
        let result = sum as u8;
        self.status.v = (self.a ^ result) & (value ^ result) & 0x80 != 0;
        self.a = result;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn sbc<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.sbc_with_val(value);
    }

    pub(crate) fn sbc_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.sbc_with_val(value);
    }

    // --- Logical ---
    pub(crate) fn and<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.a &= value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn and_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.a &= value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn ora<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.a |= value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn ora_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.a |= value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn eor<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.a ^= value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn eor_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.a ^= value;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn bit<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.status.z = (self.a & value) == 0;
        self.status.n = (value & 0x80) != 0;
        self.status.v = (value & 0x40) != 0;
    }

    // --- Compares ---
    pub(crate) fn cmp<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.compare(self.a, value);
    }

    pub(crate) fn cmp_with_addr<M: Memory>(&mut self, mem: &mut M, addr: u16) {
        let value = mem.read(addr);
        self.compare(self.a, value);
    }

    pub(crate) fn cpx<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.compare(self.x, value);
    }

    pub(crate) fn cpy<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let value = self.read_operand(mem, mode);
        self.compare(self.y, value);
    }

    pub(crate) fn compare(&mut self, reg: u8, value: u8) {
        self.status.c = reg >= value;
        self.update_zero_and_negative_flags(reg.wrapping_sub(value));
    }

    // --- Increments / Decrements ---
    pub(crate) fn inc<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        let value = mem.read(addr).wrapping_add(1);
        mem.write(addr, value);
        self.update_zero_and_negative_flags(value);
    }

    pub(crate) fn dec<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        let value = mem.read(addr).wrapping_sub(1);
        mem.write(addr, value);
        self.update_zero_and_negative_flags(value);
    }

    pub(crate) fn inx(&mut self) {
        self.x = self.x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.x);
    }
    pub(crate) fn iny(&mut self) {
        self.y = self.y.wrapping_add(1);
        self.update_zero_and_negative_flags(self.y);
    }
    pub(crate) fn dex(&mut self) {
        self.x = self.x.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.x);
    }
    pub(crate) fn dey(&mut self) {
        self.y = self.y.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.y);
    }

    // --- Shifts / Rotates ---
    pub(crate) fn asl_acc(&mut self) {
        self.status.c = (self.a & 0x80) != 0;
        self.a <<= 1;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn asl<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        let mut value = mem.read(addr);
        self.status.c = (value & 0x80) != 0;
        value <<= 1;
        mem.write(addr, value);
        self.update_zero_and_negative_flags(value);
    }

    pub(crate) fn lsr_acc(&mut self) {
        self.status.c = (self.a & 0x01) != 0;
        self.a >>= 1;
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn lsr<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        let mut value = mem.read(addr);
        self.status.c = (value & 0x01) != 0;
        value >>= 1;
        mem.write(addr, value);
        self.update_zero_and_negative_flags(value);
    }

    pub(crate) fn rol_acc(&mut self) {
        let old_c = self.status.c;
        self.status.c = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | (if old_c { 1 } else { 0 });
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn rol<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        let mut value = mem.read(addr);
        let old_c = self.status.c;
        self.status.c = (value & 0x80) != 0;
        value = (value << 1) | (if old_c { 1 } else { 0 });
        mem.write(addr, value);
        self.update_zero_and_negative_flags(value);
    }

    pub(crate) fn ror_acc(&mut self) {
        let old_c = self.status.c;
        self.status.c = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | (if old_c { 0x80 } else { 0 });
        self.update_zero_and_negative_flags(self.a);
    }

    pub(crate) fn ror<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        let mut value = mem.read(addr);
        let old_c = self.status.c;
        self.status.c = (value & 0x01) != 0;
        value = (value >> 1) | (if old_c { 0x80 } else { 0 });
        mem.write(addr, value);
        self.update_zero_and_negative_flags(value);
    }

    // --- Branching ---
    pub(crate) fn branch(&mut self, condition: bool, offset: i8) -> u32 {
        if condition {
            let old_pc = self.pc;
            self.pc = self.pc.wrapping_add(offset as i16 as u16);
            if (old_pc & 0xFF00) != (self.pc & 0xFF00) {
                2
            } else {
                1
            }
        } else {
            0
        }
    }

    // --- Jumps / Branches ---
    pub(crate) fn jmp<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        self.pc = addr;
    }

    pub(crate) fn jsr<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) {
        let (addr, _) = self.get_operand_address(mem, mode);
        let ret_pc = self.pc.wrapping_sub(1);
        self.stack_push(mem, (ret_pc >> 8) as u8);
        self.stack_push(mem, (ret_pc & 0xFF) as u8);
        self.pc = addr;
    }

    pub(crate) fn rts<M: Memory>(&mut self, mem: &mut M) {
        let lo = self.stack_pop(mem) as u16;
        let hi = self.stack_pop(mem) as u16;
        self.pc = ((hi << 8) | lo).wrapping_add(1);
    }

    pub(crate) fn rti<M: Memory>(&mut self, mem: &mut M) {
        let p = self.stack_pop(mem);
        self.status.from_byte(p);
        self.status.b = false; // B flag is not restored from stack (Bug 3)
        self.status.u = true; // U flag always 1
        let lo = self.stack_pop(mem) as u16;
        let hi = self.stack_pop(mem) as u16;
        self.pc = (hi << 8) | lo;
    }

    // --- Transfers ---
    pub(crate) fn tax(&mut self) {
        self.x = self.a;
        self.update_zero_and_negative_flags(self.x);
    }
    pub(crate) fn tay(&mut self) {
        self.y = self.a;
        self.update_zero_and_negative_flags(self.y);
    }
    pub(crate) fn txa(&mut self) {
        self.a = self.x;
        self.update_zero_and_negative_flags(self.a);
    }
    pub(crate) fn tya(&mut self) {
        self.a = self.y;
        self.update_zero_and_negative_flags(self.a);
    }
    pub(crate) fn tsx(&mut self) {
        self.x = self.sp;
        self.update_zero_and_negative_flags(self.x);
    }
    pub(crate) fn txs(&mut self) {
        self.sp = self.x;
    }

    // --- Stack ---
    pub(crate) fn pha<M: Memory>(&mut self, mem: &mut M) {
        self.stack_push(mem, self.a);
    }
    pub(crate) fn pla<M: Memory>(&mut self, mem: &mut M) {
        self.a = self.stack_pop(mem);
        self.update_zero_and_negative_flags(self.a);
    }

    // --- Flags ---
    pub(crate) fn clc(&mut self) {
        self.status.c = false;
    }
    pub(crate) fn sec(&mut self) {
        self.status.c = true;
    }
    pub(crate) fn cli(&mut self) {
        self.status.i = false;
    }
    pub(crate) fn sei(&mut self) {
        self.status.i = true;
    }
    pub(crate) fn clv(&mut self) {
        self.status.v = false;
    }
    pub(crate) fn cld(&mut self) {
        self.status.d = false;
    }
    pub(crate) fn sed(&mut self) {
        self.status.d = true;
    }

    pub(crate) fn nop(&mut self) {}

    // --- Helper Methods ---
    pub(crate) fn update_zero_and_negative_flags(&mut self, result: u8) {
        self.status.z = result == 0;
        self.status.n = (result & 0x80) != 0;
    }

    pub(crate) fn stack_push<M: Memory>(&mut self, mem: &mut M, data: u8) {
        mem.write(0x0100 + self.sp as u16, data);
        self.sp = self.sp.wrapping_sub(1);
    }

    pub(crate) fn stack_pop<M: Memory>(&mut self, mem: &mut M) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        mem.read(0x0100 + self.sp as u16)
    }

    pub(crate) fn read_operand<M: Memory>(&mut self, mem: &mut M, mode: AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mem, mode);
        mem.read(addr)
    }

    pub(crate) fn php<M: Memory>(&mut self, mem: &mut M) {
        // NMOS 6502: PHP always pushes status with Bit 4 (B) and Bit 5 (U) set.
        self.stack_push(mem, self.status.to_byte() | 0x30);
    }

    pub(crate) fn plp<M: Memory>(&mut self, mem: &mut M) {
        let p = self.stack_pop(mem);
        self.status.from_byte(p);
        self.status.b = false; // B flag is always false inside CPU (Bug 2)
        self.status.u = true; // U flag always 1
    }

    fn sbc_with_val(&mut self, value: u8) {
        if self.status.d {
            self.sbc_decimal(value);
        } else {
            self.adc_with_val(!value);
        }
    }

    fn adc_decimal(&mut self, value: u8) {
        let carry_in = if self.status.c { 1 } else { 0 };
        let a = self.a;
        let sum = a as u16 + value as u16 + carry_in as u16;
        let binary_result = sum as u8;

        self.status.v = (a ^ binary_result) & (value ^ binary_result) & 0x80 != 0;

        let mut lo = (a & 0x0F) + (value & 0x0F) + carry_in;
        let mut carry_hi = 0;
        if lo > 9 {
            lo = lo.wrapping_add(6);
            carry_hi = 1;
        }

        let mut hi = (a >> 4) + (value >> 4) + carry_hi;
        self.status.c = hi > 9;
        if hi > 9 {
            hi = hi.wrapping_add(6);
        }

        self.a = (hi << 4) | (lo & 0x0F);
        self.update_zero_and_negative_flags(self.a);
    }

    fn sbc_decimal(&mut self, value: u8) {
        let carry_in = if self.status.c { 1 } else { 0 };
        let a = self.a;
        let diff = a as i16 - value as i16 - (1 - carry_in) as i16;
        let binary_result = diff as u8;

        self.status.v = ((a ^ value) & (a ^ binary_result) & 0x80) != 0;

        let mut lo = (a & 0x0F) as i16 - (value & 0x0F) as i16 - (1 - carry_in) as i16;
        let mut borrow_hi = 0;
        if lo < 0 {
            lo -= 6;
            borrow_hi = 1;
        }

        let mut hi = (a >> 4) as i16 - (value >> 4) as i16 - borrow_hi;
        self.status.c = diff >= 0;
        if hi < 0 {
            hi -= 6;
        }

        self.a = (((hi as u8) << 4) & 0xF0) | ((lo as u8) & 0x0F);
        self.update_zero_and_negative_flags(self.a);
    }
}
