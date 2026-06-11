use crate::cpu::CPU;
use crate::memory::Apple2Memory;

pub struct Apple2Machine {
    pub cpu: CPU,
    pub mem: Apple2Memory,
    pub total_cycles: u64,
    // 電子束位置以增量計數維護：Cortex-M0+ 無硬體除法器，
    // 每條指令做 u64 % / ÷ (軟體除法數百週期) 是不可接受的固定開銷
    line_cycle: u32, // 0..65，掃描線內週期
    beam_line: u32,  // 0..262，目前掃描線
}

impl Apple2Machine {
    pub fn new() -> Self {
        Self {
            cpu: CPU::new(),
            mem: Apple2Memory::new(),
            total_cycles: 0,
            line_cycle: 0,
            beam_line: 0,
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.mem);
        self.total_cycles = 0;
        self.line_cycle = 0;
        self.beam_line = 0;
        crate::BEAM_Y.store(0, core::sync::atomic::Ordering::Relaxed);
    }

    pub fn step(&mut self) -> u32 {
        self.mem.begin_cpu_step(self.total_cycles);
        let cycles = self.cpu.step(&mut self.mem);
        self.mem.finalize_cpu_step_cycles(cycles);
        self.mem.end_cpu_step();
        self.total_cycles = self.total_cycles.wrapping_add(cycles as u64);

        // 65 cycles/line, 262 lines/frame (17030 cycles)；單條指令 ≤ 8+ cycles，
        // 一次最多跨一條掃描線，所以單次減法即可
        self.line_cycle += cycles;
        if self.line_cycle >= 65 {
            self.line_cycle -= 65;
            self.beam_line += 1;
            if self.beam_line >= 262 {
                self.beam_line = 0;
            }
            crate::BEAM_Y.store(self.beam_line, core::sync::atomic::Ordering::Relaxed);
        }

        cycles
    }

    pub fn power_on(&mut self) {
        self.mem.power_on_reset();
        self.cpu.reset(&mut self.mem);
        self.total_cycles = 0;
        self.line_cycle = 0;
        self.beam_line = 0;
        crate::BEAM_Y.store(0, core::sync::atomic::Ordering::Relaxed);
    }
}
