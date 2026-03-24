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
    pub read_shift_register: u8,
    pub read_bit_phase: u8,
    pub write_ready: bool,
    pub write_bit_phase: u8,
    pub is_dirty: bool,
}

impl Disk2 {
    pub fn new() -> Self {
        Self {
            rom: [0; 256],
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
            read_shift_register: 0,
            read_bit_phase: 0,
            write_ready: false,
            write_bit_phase: 0,
            is_dirty: false,
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
            if !self.is_disk_loaded { return 0x7F; } // 浮空狀態

            // Q7 ON, Q6 OFF 模式 (由 handle_io 設定)：這是 Write Protect Sense
            if self.q7 && !self.q6 {
                return 0x00; // 0x00 代表「未寫入保護」，0x80 代表「有保護」
            }

            // Q7 ON, Q6 ON 模式：寫入準備檢查
            if self.q6 && self.q7 {
                let ready = self.write_ready;
                self.write_ready = false;
                return if ready { 0x80 } else { 0x00 };
            }

            // 正常的讀取模式 (Q7 OFF)
            let val = self.data_latch;
            self.data_latch &= 0x7F;
            return val;
        }
        0x00
    }

    pub fn write_io(&mut self, addr: u16, data: u8) {
        self.handle_io(addr);
        let switch = addr & 0x0F;
        // 只要碰觸寫入暫存器，不論模式，先更新 Latch
        if switch == 0x0D {
            self.data_latch = data;
            self.write_bit_phase = 0;
            self.write_ready = false;
            // 如果寫入門 (q7) 開啟，則標記為髒資料
            if self.q7 {
                self.is_dirty = true;
            }
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
            if self.current_track_data.length == 0 { return; }

            while self.cycles_accumulator >= 4 {
                self.cycles_accumulator -= 4;
                
                let mut byte_finished = false;

                if self.q7 {
                    // --- 寫入模式 (Write Mode) ---
                    // 只要 Q7 為 ON，磁頭就在輸出信號。
                    // 雖然真實硬體在 Q6 ON 時是載入 (Load)，Q6 OFF 時是移位 (Shift)，
                    // 但在位元級模擬中，我們簡化為：只要在寫入模式，就持續將 Latch 內容輸出至磁軌。
                    self.write_bit_phase += 1;
                    if self.write_bit_phase >= 8 {
                        self.write_bit_phase = 0;
                        let val = self.data_latch;
                        self.is_dirty = true; 
                        self.current_track_data.raw_bytes[self.byte_index] = val;
                        self.current_track_data.dirty_mask[self.byte_index] = true; // 真正標記軟體寫入
                        self.write_ready = true;
                        byte_finished = true;
                    }
                } else {
                    // --- 讀取或閒置模式 (Read / Idle Mode) ---
                    let bit = (self.current_track_data.raw_bytes[self.byte_index] >> (7 - self.read_bit_phase)) & 1;
                    self.read_shift_register = (self.read_shift_register << 1) | bit;
                    self.read_bit_phase += 1;
                    
                    if self.read_bit_phase >= 8 {
                        self.read_bit_phase = 0;
                        let published_byte = self.read_shift_register;
                        // 只有在純讀取模式 (!q6 && !q7) 下才更新 Latch
                        if !self.q6 && !self.q7 && (published_byte & 0x80) != 0 {
                            self.data_latch = published_byte;
                        }
                        byte_finished = true;
                    }
                }

                if byte_finished {
                    self.byte_index = (self.byte_index + 1) % self.current_track_data.length;
                }
            }
        }
    }

    fn reset_rotation_state(&mut self) {
        self.byte_index = 0;
        self.read_shift_register = 0;
        self.read_bit_phase = 0;
        self.write_bit_phase = 0;
    }

    pub fn reset(&mut self) {
        self.motor_on = false;
        self.current_track = 0;
        self.is_disk_loaded = false;
        self.needs_reload = false;
        self.is_dirty = false;
        self.q6 = false;
        self.q7 = false;
    }
}
