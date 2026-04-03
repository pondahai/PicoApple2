use crate::cpu::CPU;
use crate::memory::Apple2Memory;

pub struct Apple2Machine {
    pub cpu: CPU,
    pub mem: Apple2Memory,
    pub total_cycles: u64,
}

impl Apple2Machine {
    pub fn new() -> Self {
        Self {
            cpu: CPU::new(),
            mem: Apple2Memory::new(),
            total_cycles: 0,
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.mem);
        self.total_cycles = 0;
    }

    pub fn step(&mut self) -> u32 {
        self.mem.begin_cpu_step(self.total_cycles);
        let cycles = self.cpu.step(&mut self.mem);
        self.mem.finalize_cpu_step_cycles(cycles);
        self.mem.end_cpu_step();
        self.total_cycles = self.total_cycles.wrapping_add(cycles as u64);
        
        crate::BEAM_Y.store(((self.total_cycles % 17030) / 65) as u32, core::sync::atomic::Ordering::Relaxed);
        
        cycles
    }

    pub fn power_on(&mut self) {
        self.mem.power_on_reset();
        self.cpu.reset(&mut self.mem);
        self.total_cycles = 0;
    }
}
