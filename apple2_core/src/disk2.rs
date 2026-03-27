pub struct Disk2 {
    pub rom: &'static [u8],
    pub motor_on: bool,
    pub q6: bool,
    pub q7: bool,
    pub current_track: usize,
    pub loaded_track_num: u8,
    pub is_disk_loaded: bool,
    pub needs_reload: bool,
    pub phases: [bool; 4],
    pub current_qtr_track: i32,
    pub byte_index: usize,
    pub cycles_accumulator: u32,
    pub data_latch: u8,
    pub is_dirty: bool,
}

impl Disk2 {
    pub fn new() -> Self {
        Self {
            rom: include_bytes!("disk2_p5.rom"),
            motor_on: false,
            q6: false,
            q7: false,
            current_track: 0,
            loaded_track_num: 0,
            is_disk_loaded: false,
            needs_reload: false,
            phases: [false; 4],
            current_qtr_track: 0,
            byte_index: 0,
            cycles_accumulator: 0,
            data_latch: 0,
            is_dirty: false,
        }
    }

    pub fn read_io(&mut self, addr: u16) -> u8 {
        self.handle_io(addr);
        let switch = addr & 0x0F;
        if self.motor_on && switch == 0x0C {
            if !self.is_disk_loaded { return 0xFF; }
            let val = if self.q6 { 0x00 } else { self.data_latch };
            // 在真實硬體中，讀取 $C08C 會讓移位暫存器重新開始填充
            // 這裡模擬：讀取後將 data_latch 清除最高位，直到下一個 32 週期循環再次填入
            self.data_latch &= 0x7F; 
            return val;
        }
        0x00
    }

    pub fn write_io(&mut self, addr: u16, data: u8) {
        self.handle_io(addr);
        if (addr & 0x0F) == 0x0D { self.data_latch = data; }
    }

    fn handle_io(&mut self, addr: u16) {
        let switch = (addr & 0x0F) as usize;
        match switch {
            0x00..=0x07 => {
                let phase = switch >> 1;
                let on = (switch & 1) != 0;
                if on != self.phases[phase] { self.phases[phase] = on; self.step_motor(); }
            }
            0x08 => self.motor_on = false,
            0x09 => self.motor_on = true,
            0x0C => self.q6 = false,
            0x0D => self.q6 = true,
            0x0E => self.q7 = false,
            0x0F => self.q7 = true,
            _ => {}
        }
    }

    fn step_motor(&mut self) {
        let phase_mask = (self.phases[0] as u8) | ((self.phases[1] as u8) << 1) | ((self.phases[2] as u8) << 2) | ((self.phases[3] as u8) << 3);
        let target_mod = match phase_mask {
            0b0001 => Some(0), 0b0011 => Some(1), 0b0010 => Some(2), 0b0110 => Some(3),
            0b0100 => Some(4), 0b1100 => Some(5), 0b1000 => Some(6), 0b1001 => Some(7),
            _ => None,
        };
        let Some(t_mod) = target_mod else { return; };
        
        // 磁軌步進邏輯優化
        let current_mod = self.current_qtr_track.rem_euclid(8);
        let mut diff = t_mod as i32 - current_mod;
        if diff > 4 { diff -= 8; }
        else if diff < -4 { diff += 8; }
        
        self.current_qtr_track = (self.current_qtr_track + diff).clamp(0, 34 * 4);
        let next_track = (self.current_qtr_track / 4) as usize;
        if next_track != self.current_track { 
            self.current_track = next_track; 
            self.needs_reload = true; 
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        if self.motor_on {
            self.cycles_accumulator += cycles;
            if self.is_disk_loaded {
                unsafe {
                    use crate::{TRACK_DATA_RAW, TRACK_DATA_DIRTY};
                    while self.cycles_accumulator >= 32 {
                        self.cycles_accumulator -= 32;
                        if self.q7 && self.q6 {
                            TRACK_DATA_RAW[self.byte_index] = self.data_latch;
                            TRACK_DATA_DIRTY[self.byte_index] = true;
                            self.is_dirty = true;
                        } else {
                            self.data_latch = TRACK_DATA_RAW[self.byte_index];
                        }
                        self.byte_index = (self.byte_index + 1) % 6656;
                    }
                }
            } else {
                // 如果沒載入磁碟，馬達空轉時讓 data_latch 呈現 Sync 位元
                while self.cycles_accumulator >= 32 {
                    self.cycles_accumulator -= 32;
                    self.data_latch = 0xFF; 
                }
            }
            // 限制 accumulator 避免溢出
            if self.cycles_accumulator > 1000 { self.cycles_accumulator = 0; }
        }
    }

    pub fn reset(&mut self) {
        self.motor_on = false; self.current_track = 0; self.is_disk_loaded = false; self.needs_reload = false;
        self.is_dirty = false; self.q6 = false; self.q7 = false; self.byte_index = 0; self.cycles_accumulator = 0;
    }
}
