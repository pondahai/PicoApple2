extern crate alloc;
use crate::nibble::TrackData;

pub struct Disk2 {
    pub rom: [u8; 256],
    pub motor_on: bool,
    pub drive_select: u8,
    pub q6: bool,
    pub q7: bool,
    pub current_track: usize,
    pub loaded_track_num: u8,
    pub current_track_data: TrackData,
    pub is_disk_loaded: bool,
    pub needs_reload: bool,
    pub phases: [bool; 4],
    pub current_qtr_track: i32,

    pub byte_index: usize,
    pub cycles_accumulator: u32,
    pub data_latch: u8,
    pub is_dirty: bool,

    // --- 時序修正：新資料追蹤 ---
    pub data_updated: bool,
}

const DISK2_ROM: [u8; 256] = [
    0x18, 0xD8, 0x18, 0x08, 0x0A, 0x0A, 0x0A, 0x0A, 0x18, 0x39, 0x18, 0x39, 0x18, 0x3B, 0x18, 0x3B,
    0x18, 0x38, 0x18, 0x28, 0x0A, 0x0A, 0x0A, 0x0A, 0x18, 0x39, 0x18, 0x39, 0x18, 0x3B, 0x18, 0x3B,
    0x2D, 0xD8, 0x38, 0x48, 0x0A, 0x0A, 0x0A, 0x0A, 0x28, 0x48, 0x28, 0x48, 0x28, 0x48, 0x28, 0x48,
    0x2D, 0x48, 0x38, 0x48, 0x0A, 0x0A, 0x0A, 0x0A, 0x28, 0x48, 0x28, 0x48, 0x28, 0x48, 0x28, 0x48,
    0xD8, 0xD8, 0xD8, 0xD8, 0x0A, 0x0A, 0x0A, 0x0A, 0x58, 0x78, 0x58, 0x78, 0x58, 0x78, 0x58, 0x78,
    0x58, 0x78, 0x58, 0x78, 0x0A, 0x0A, 0x0A, 0x0A, 0x58, 0x78, 0x58, 0x78, 0x58, 0x78, 0x58, 0x78,
    0xD8, 0xD8, 0xD8, 0xD8, 0x0A, 0x0A, 0x0A, 0x0A, 0x68, 0x08, 0x68, 0x88, 0x68, 0x08, 0x68, 0x88,
    0x68, 0x88, 0x68, 0x88, 0x0A, 0x0A, 0x0A, 0x0A, 0x68, 0x08, 0x68, 0x88, 0x68, 0x08, 0x68, 0x88,
    0xD8, 0xCD, 0xD8, 0xD8, 0x0A, 0x0A, 0x0A, 0x0A, 0x98, 0xB9, 0x98, 0xB9, 0x98, 0xBB, 0x98, 0xBB,
    0x98, 0xBD, 0x98, 0xB8, 0x0A, 0x0A, 0x0A, 0x0A, 0x98, 0xB9, 0x98, 0xB9, 0x98, 0xBB, 0x98, 0xBB,
    0xD8, 0xD9, 0xD8, 0xD8, 0x0A, 0x0A, 0x0A, 0x0A, 0xA8, 0xC8, 0xA8, 0xC8, 0xA8, 0xC8, 0xA8, 0xC8,
    0x29, 0x59, 0xA8, 0xC8, 0x0A, 0x0A, 0x0A, 0x0A, 0xA8, 0xC8, 0xA8, 0xC8, 0xA8, 0xC8, 0xA8, 0xC8,
    0xD9, 0xFD, 0xD8, 0xF8, 0x0A, 0x0A, 0x0A, 0x0A, 0xD8, 0xF8, 0xD8, 0xF8, 0xD8, 0xF8, 0xD8, 0xF8,
    0xD9, 0xFD, 0xA0, 0xF8, 0x0A, 0x0A, 0x0A, 0x0A, 0xD8, 0xF8, 0xD8, 0xF8, 0xD8, 0xF8, 0xD8, 0xF8,
    0xD8, 0xDD, 0xE8, 0xE0, 0x0A, 0x0A, 0x0A, 0x0A, 0xE8, 0x88, 0xE8, 0x08, 0xE8, 0x88, 0xE8, 0x08,
    0x08, 0x4D, 0xE8, 0xE0, 0x0A, 0x0A, 0x0A, 0x0A, 0xE8, 0x88, 0xE8, 0x08, 0xE8, 0x88, 0xE8, 0x08,
];

impl Disk2 {
    pub fn new() -> Self {
        Self {
            rom: DISK2_ROM,
            motor_on: false,
            drive_select: 1,
            q6: false,
            q7: false,
            current_track: 0,
            loaded_track_num: 0,
            current_track_data: TrackData::new(),
            is_disk_loaded: false,
            needs_reload: false,
            phases: [false; 4],
            current_qtr_track: 0,
            byte_index: 0,
            cycles_accumulator: 0,
            data_latch: 0,
            is_dirty: false,
            data_updated: false,
        }
    }

    pub fn load_track_data(&mut self, track_num: u8, data: TrackData) {
        self.loaded_track_num = track_num;
        self.current_track_data = data;
        self.reset_rotation_state();
    }

    pub fn read_io(&mut self, addr: u16) -> u8 {
        self.handle_io(addr);
        let switch = addr & 0x0F;
        if self.motor_on && switch == 0x0C {
            if !self.is_disk_loaded { return 0x7F; } 
            let val = self.data_latch;
            if !self.q6 { 
                self.data_latch &= 0x7F; 
            }
            return val;
        }
        0x00
    }

    pub fn write_io(&mut self, addr: u16, data: u8) {
        self.handle_io(addr);
        let switch = addr & 0x0F;
        if switch == 0x0D {
            self.data_latch = data;
            self.data_updated = true;
        }
    }

    fn handle_io(&mut self, addr: u16) {
        let switch = (addr & 0x0F) as usize;
        match switch {
            0x00..=0x07 => {
                let phase = switch >> 1;
                let on = (switch & 1) != 0;
                if on != self.phases[phase] {
                    self.phases[phase] = on;
                    self.step_motor();
                }
            }
            0x08 => self.motor_on = false,
            0x09 => self.motor_on = true,
            0x0A => self.drive_select = 1,
            0x0B => self.drive_select = 2,
            0x0C => self.q6 = false,
            0x0D => self.q6 = true,
            0x0E => self.q7 = false,
            0x0F => self.q7 = true,
            _ => {}
        }
    }

    fn step_motor(&mut self) {
        let phase_mask = (self.phases[0] as u8)
            | ((self.phases[1] as u8) << 1)
            | ((self.phases[2] as u8) << 2)
            | ((self.phases[3] as u8) << 3);

        let target_mod = match phase_mask {
            0b0001 => Some(0), 0b0011 => Some(1), 0b0010 => Some(2), 0b0110 => Some(3),
            0b0100 => Some(4), 0b1100 => Some(5), 0b1000 => Some(6), 0b1001 => Some(7),
            _ => None,
        };

        let Some(target_mod) = target_mod else { return; };
        let base = self.current_qtr_track.div_euclid(8) * 8;
        let candidates = [base + target_mod, base + target_mod - 8, base + target_mod + 8];
        let mut target_qtr = self.current_qtr_track;
        let mut best_diff = i32::MAX;

        for candidate in candidates {
            let diff = (candidate - self.current_qtr_track).abs();
            if diff < best_diff {
                best_diff = diff;
                target_qtr = candidate;
            }
        }

        self.current_qtr_track = target_qtr.clamp(0, 34 * 4);
        let next_track = (self.current_qtr_track / 4) as usize;
        if next_track != self.current_track {
            self.current_track = next_track;
            self.needs_reload = true;
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        if self.motor_on && self.is_disk_loaded {
            self.cycles_accumulator += cycles;
            let track_len = self.current_track_data.length;
            if track_len == 0 { return; }

            while self.cycles_accumulator >= 32 {
                self.cycles_accumulator -= 32;

                if self.q7 {
                    // 寫入模式：刻入磁軌
                    self.current_track_data.raw_bytes[self.byte_index] = self.data_latch;
                    self.current_track_data.dirty_mask[self.byte_index] = true;
                    self.is_dirty = true;
                    self.data_updated = false;
                } else {
                    // 讀取模式
                    self.data_latch = self.current_track_data.raw_bytes[self.byte_index];
                }

                self.byte_index = (self.byte_index + 1) % track_len;
            }
        }
    }

    fn reset_rotation_state(&mut self) {
        self.byte_index = 0;
        self.cycles_accumulator = 0;
    }

    pub fn reset(&mut self) {
        self.motor_on = false;
        self.current_track = 0;
        self.is_disk_loaded = false;
        self.needs_reload = false;
        self.is_dirty = false;
        self.q6 = false;
        self.q7 = false;
        self.reset_rotation_state();
    }
}
