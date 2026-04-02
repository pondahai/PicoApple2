use crate::disk2::Disk2;

pub trait Memory {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8);
    fn read_word(&mut self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }
}

pub struct Apple2Memory {
    pub rom: &'static [u8],
    pub text_mode: bool,
    pub mixed_mode: bool,
    pub page2: bool,
    pub hires_mode: bool,
    pub keyboard_latch: u8,
    pub disk2: Disk2,
    pub disk2_motor_on: bool, // 緩存馬達狀態供 FFI 使用
    pub speaker: bool,
    pub cpu_step_cycle_base: u64,
    pub cpu_step_cycle_cursor: u32,
    pub cpu_step_audio_active: bool,
    pub pushbuttons: [bool; 2],
    pub paddles: [u8; 4],
    pub paddle_latch_cycle: u64,
    pub lc_read_enable: bool,
    pub lc_write_enable: bool,
    pub lc_bank2: bool,
    pub lc_pre_write_switch: u16,
}

#[cfg(not(test))]
unsafe extern "C" { fn arduino_toggle_speaker(); }
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn arduino_toggle_speaker() {}

impl Apple2Memory {
    pub fn new() -> Self {
        Self {
            rom: &[], text_mode: true, mixed_mode: false, page2: false, hires_mode: false,
            keyboard_latch: 0, disk2: Disk2::new(), disk2_motor_on: false,
            speaker: false, cpu_step_cycle_base: 0, cpu_step_cycle_cursor: 0, cpu_step_audio_active: false,
            pushbuttons: [false; 2], paddles: [127; 4], paddle_latch_cycle: 0,
            lc_read_enable: false, lc_write_enable: false, lc_bank2: true, lc_pre_write_switch: 0,
        }
    }

    pub fn load_rom(&mut self, data: &'static [u8]) { self.rom = data; }

    pub fn power_on_reset(&mut self) {
        unsafe { (*core::ptr::addr_of_mut!(crate::RAM_48K)).fill(0); }
        self.text_mode = true; self.mixed_mode = false; self.page2 = false; self.hires_mode = false;
        self.keyboard_latch = 0; self.speaker = false; self.disk2.reset();
        self.lc_read_enable = false; self.lc_write_enable = false; self.lc_bank2 = true;
        unsafe { crate::RAM_48K[0x03F4] = 0; }
    }

    pub fn begin_cpu_step(&mut self, cycle_base: u64) { self.cpu_step_cycle_base = cycle_base; self.cpu_step_cycle_cursor = 0; self.cpu_step_audio_active = true; }
    pub fn end_cpu_step(&mut self) { self.cpu_step_audio_active = false; }
    pub fn finalize_cpu_step_cycles(&mut self, total_cycles: u32) {
        // 一次性打包結算整條指令的週期給磁碟
        self.disk2.tick(total_cycles);
        self.cpu_step_cycle_cursor = total_cycles;
        self.disk2_motor_on = self.disk2.motor_on;
    }

    fn record_bus_access(&mut self) {
        // 移除頻繁的 disk2.tick(1) 呼叫，僅保留週期計數供 paddle 使用
        if self.cpu_step_audio_active { self.cpu_step_cycle_cursor = self.cpu_step_cycle_cursor.saturating_add(1); }
    }
}

impl Memory for Apple2Memory {
    fn read(&mut self, addr: u16) -> u8 {
        self.record_bus_access();
        match addr {
            0x0000..=0xBFFF => unsafe { crate::RAM_48K[addr as usize] },
            0xC000..=0xCFFF => {
                if addr >= 0xC600 && addr <= 0xC6FF { return self.disk2.rom[(addr & 0xFF) as usize]; }
                match addr {
                    0xC000..=0xC00F => self.keyboard_latch,
                    0xC010..=0xC01F => { let v = self.keyboard_latch; self.keyboard_latch &= 0x7F; v }
                    0xC080..=0xC08F => {
                        self.lc_bank2 = (addr & 0x08) == 0;
                        self.lc_read_enable = (addr & 0x03) == 0x00 || (addr & 0x03) == 0x03;
                        if (addr & 0x01) != 0 {
                            let sw = 0xC080 | (addr & 0x000B);
                            if self.lc_pre_write_switch == sw { self.lc_write_enable = true; }
                            self.lc_pre_write_switch = sw;
                            return 0; // 阻止 clear_pre_write
                        }
                        self.lc_write_enable = false; 0
                    }
                    0xC0E0..=0xC0EF => self.disk2.read_io(addr),
                    0xC030 => { self.speaker = !self.speaker; unsafe { arduino_toggle_speaker(); } 0 }
                    0xC050 => { self.text_mode = false; 0 } 0xC051 => { self.text_mode = true; 0 }
                    0xC052 => { self.mixed_mode = false; 0 } 0xC053 => { self.mixed_mode = true; 0 }
                    0xC054 => { self.page2 = false; 0 } 0xC055 => { self.page2 = true; 0 }
                    0xC056 => { self.hires_mode = false; 0 } 0xC057 => { self.hires_mode = true; 0 }
                    0xC061 => if self.pushbuttons[0] { 0x80 } else { 0x00 },
                    0xC062 => if self.pushbuttons[1] { 0x80 } else { 0x00 },
                    0xC064..=0xC067 => {
                        let el = (self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64).saturating_sub(self.paddle_latch_cycle);
                        if el < (8 + (self.paddles[(addr - 0xC064) as usize] as u64 * 11)) { 0x80 } else { 0x00 }
                    }
                    0xC070 => { self.paddle_latch_cycle = self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64; 0 }
                    _ => 0,
                }
            }
            0xD000..=0xFFFF => {
                if self.lc_read_enable {
                    let off = if addr < 0xE000 { if self.lc_bank2 { 0x1000 } else { 0 } } else { 0x2000 };
                    unsafe { crate::LC_RAM_16K[(addr - 0xD000 + off as u16) as usize] }
                } else { self.rom[(addr - 0xD000) as usize] }
            }
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.record_bus_access();
        match addr {
            0x0000..=0xBFFF => unsafe { crate::RAM_48K[addr as usize] = data; },
            0xC000..=0xCFFF => {
                match addr {
                    0xC010 => self.keyboard_latch &= 0x7F,
                    0xC0E0..=0xC0EF => self.disk2.write_io(addr, data),
                    0xC030 => { self.speaker = !self.speaker; unsafe { arduino_toggle_speaker(); } }
                    0xC050 => self.text_mode = false, 0xC051 => self.text_mode = true,
                    0xC052 => self.mixed_mode = false, 0xC053 => self.mixed_mode = true,
                    0xC054 => self.page2 = false, 0xC055 => self.page2 = true,
                    0xC056 => self.hires_mode = false, 0xC057 => self.hires_mode = true,
                    0xC070 => self.paddle_latch_cycle = self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64,
                    _ => {}
                }
            }
            0xD000..=0xFFFF => {
                if self.lc_write_enable {
                    let off = if addr < 0xE000 { if self.lc_bank2 { 0x1000 } else { 0 } } else { 0x2000 };
                    unsafe { crate::LC_RAM_16K[(addr - 0xD000 + off as u16) as usize] = data; }
                }
            }
        }
    }
}
