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

// 喇叭翻轉 → 推入時間戳環形緩衝（見 lib.rs AUDIO_RING）。
// 緩衝滿時丟棄（重放端正常消耗下不會發生：4ms 延遲窗 × 最高翻轉率 ≪ 1024）。
fn push_speaker_toggle(cycle: u32) {
    use core::sync::atomic::Ordering;
    let head = crate::AUDIO_HEAD.load(Ordering::Relaxed);
    let tail = crate::AUDIO_TAIL.load(Ordering::Acquire);
    if head.wrapping_sub(tail) < crate::AUDIO_RING_SIZE as u32 {
        unsafe {
            *(core::ptr::addr_of_mut!(crate::AUDIO_RING) as *mut u32)
                .add((head as usize) & (crate::AUDIO_RING_SIZE - 1)) = cycle;
        }
        crate::AUDIO_HEAD.store(head.wrapping_add(1), Ordering::Release);
    }
}

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

    pub fn load_rom(&mut self, data: &'static [u8]) {
        // read() 的 ROM 路徑以 get_unchecked 依賴此長度不變量
        assert!(data.len() == 12288, "system ROM must be exactly 12KB");
        self.rom = data;
    }

    pub fn power_on_reset(&mut self) {
        unsafe { (*core::ptr::addr_of_mut!(crate::RAM_48K)).fill(0); }
        self.text_mode = true; self.mixed_mode = false; self.page2 = false; self.hires_mode = false;
        self.keyboard_latch = 0; self.speaker = false; self.disk2.reset();
        // 丟棄殘留的喇叭翻轉，避免重置後播出舊聲音
        crate::AUDIO_TAIL.store(crate::AUDIO_HEAD.load(core::sync::atomic::Ordering::Acquire), core::sync::atomic::Ordering::Release);
        self.lc_read_enable = false; self.lc_write_enable = false; self.lc_bank2 = true;
        unsafe { crate::RAM_48K[0x03F4] = 0; }
    }

    /// 暖開機（Ctrl+Reset）用：只丟棄殘留的喇叭翻轉並歸零喇叭電位，
    /// 不清 RAM、不動 $3F4 powerup byte（保留 warm start 語義）。
    /// 缺了這步，reset 後 total_cycles 跳回 0 而 ring 仍留著舊大週期戳，
    /// 韌體 audioDueUs 會在不連續點換算出錯誤間距 → beep 音高偏低/破音。
    pub fn reset_audio_io(&mut self) {
        self.speaker = false;
        crate::AUDIO_TAIL.store(crate::AUDIO_HEAD.load(core::sync::atomic::Ordering::Acquire), core::sync::atomic::Ordering::Release);
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
        // (wrapping_add 即可：單條指令內的匯流排存取數遠不可能溢位 u32)
        if self.cpu_step_audio_active { self.cpu_step_cycle_cursor = self.cpu_step_cycle_cursor.wrapping_add(1); }
    }

    /// 影像掃描器在指定 CPU cycle 從 RAM 抓取的位址。
    /// 移植自 AppleWin `VideoGetScannerAddress`(Jim Sather《Understanding
    /// the Apple IIe》第 5 章模型）。NTSC:65 horizontal clocks/掃描線、262
    /// 線/frame。掃描器在 HBL/VBL 期間仍持續產生位址 —— 這正是 floating bus
    /// 能當亂數源的原因:其值隨光束位置變動,逐次讀取皆不同。
    fn video_scanner_address(&self, cycle: u64) -> u16 {
        const H_CLOCKS: u64 = 65;
        const H_PE_CLOCK: u64 = 40;
        const H_PRESET_CLOCK: u64 = 41;
        const H_CLOCK0_STATE: i32 = 0x18;
        const V_LINE0_STATE: i32 = 0x100;
        const V_PRESET_LINE: u64 = 256;
        const SCAN_LINES: u64 = 262; // NTSC
        const SCAN_CYCLES: u64 = SCAN_LINES * H_CLOCKS;

        let n_cycles = cycle % SCAN_CYCLES;

        // 水平計數器狀態 (h_0..h_5)
        let n_hclock = (n_cycles + H_PE_CLOCK) % H_CLOCKS;
        let mut n_hstate = H_CLOCK0_STATE + n_hclock as i32;
        if n_hclock >= H_PRESET_CLOCK { n_hstate -= 1; }
        let h_0 = (n_hstate >> 0) & 1;
        let h_1 = (n_hstate >> 1) & 1;
        let h_2 = (n_hstate >> 2) & 1;
        let h_3 = (n_hstate >> 3) & 1;
        let h_4 = (n_hstate >> 4) & 1;
        let h_5 = (n_hstate >> 5) & 1;

        // 垂直計數器狀態 (v_a..v_4)
        let n_vline = n_cycles / H_CLOCKS;
        let mut n_vstate = V_LINE0_STATE + n_vline as i32;
        if n_vline >= V_PRESET_LINE { n_vstate -= SCAN_LINES as i32; }
        let v_a = (n_vstate >> 0) & 1;
        let v_b = (n_vstate >> 1) & 1;
        let v_c = (n_vstate >> 2) & 1;
        let v_0 = (n_vstate >> 3) & 1;
        let v_1 = (n_vstate >> 4) & 1;
        let v_2 = (n_vstate >> 5) & 1;
        let v_3 = (n_vstate >> 6) & 1;
        let v_4 = (n_vstate >> 7) & 1;

        let mut hires = self.hires_mode && !self.text_mode;
        let page2 = self.page2;
        // 80STORE 是 //e 功能,II/II+ 永遠為 off。
        if hires && self.mixed_mode && v_4 != 0 && v_2 != 0 { hires = false; }

        // Sather 的 4-bit「sum」,構成位址 bit A3..A6。
        let addend0 = 0x0D;
        let addend1 = (h_5 << 2) | (h_4 << 1) | (h_3 << 0);
        let addend2 = (v_4 << 3) | (v_3 << 2) | (v_4 << 1) | (v_3 << 0);
        let sum = (addend0 + addend1 + addend2) & 0x0F;

        let mut addr_h: u16 = 0;
        addr_h |= (h_0 as u16) << 0;
        addr_h |= (h_1 as u16) << 1;
        addr_h |= (h_2 as u16) << 2;
        addr_h |= (sum as u16) << 3;
        if !hires {
            // Apple II/II+:HBL 期間 text/lores 掃描器定址 $1000/$1800 區。
            if h_5 == 0 && (h_4 == 0 || h_3 == 0) { addr_h |= 1 << 12; }
        }

        let mut addr_v: u16 = 0;
        addr_v |= (v_0 as u16) << 7;
        addr_v |= (v_1 as u16) << 8;
        addr_v |= (v_2 as u16) << 9;

        // 80STORE off:p2a 選 page 1,p2b 選 page 2。
        let p2a = if !page2 { 1u16 } else { 0 };
        let p2b = if page2 { 1u16 } else { 0 };

        let mut addr_p: u16 = 0;
        if hires {
            addr_v |= (v_a as u16) << 10;
            addr_v |= (v_b as u16) << 11;
            addr_v |= (v_c as u16) << 12;
            addr_p |= p2a << 13; // $2000
            addr_p |= p2b << 14; // $4000
        } else {
            addr_p |= p2a << 10; // $0400
            addr_p |= p2b << 11; // $0800
        }

        addr_p | addr_v | addr_h
    }

    /// 讀取未驅動 `$C0xx` 時看到的值:影像掃描器這個 cycle 正在抓的位元組。
    /// 遊戲(如《德軍總部》)以此為唯一硬體亂數源 —— 爆炸白雜訊與中彈/穿透判定
    /// 都靠它。回傳常數會讓雜訊塌成單音、讓每次判定落同一邊。
    #[inline]
    fn floating_bus(&self) -> u8 {
        let cycle = self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64;
        let addr = self.video_scanner_address(cycle) as usize;
        // 所有掃描位址都落在 48K 主記憶體內;保險起見仍做邊界檢查。
        if addr < 49152 {
            unsafe { *(core::ptr::addr_of!(crate::RAM_48K) as *const u8).add(addr) }
        } else { 0 }
    }
}

impl Memory for Apple2Memory {
    fn read(&mut self, addr: u16) -> u8 {
        self.record_bus_access();
        match addr {
            // SAFETY: match arm 保證 addr <= 0xBFFF < 49152，免邊界檢查（最熱路徑）
            0x0000..=0xBFFF => unsafe { *(core::ptr::addr_of!(crate::RAM_48K) as *const u8).add(addr as usize) },
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
                            return self.floating_bus(); // 阻止 clear_pre_write;讀取浮空
                        }
                        self.lc_write_enable = false; self.floating_bus()
                    }
                    0xC0E0..=0xC0EF => self.disk2.read_io(addr),
                    0xC030 => {
                        self.speaker = !self.speaker;
                        push_speaker_toggle((self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64) as u32);
                        // 讀取同時 click 喇叭並回傳 floating bus;雜訊程式在此計時迴圈取亂數
                        self.floating_bus()
                    }
                    0xC050 => { self.text_mode = false; self.floating_bus() } 0xC051 => { self.text_mode = true; self.floating_bus() }
                    0xC052 => { self.mixed_mode = false; self.floating_bus() } 0xC053 => { self.mixed_mode = true; self.floating_bus() }
                    0xC054 => { self.page2 = false; self.floating_bus() } 0xC055 => { self.page2 = true; self.floating_bus() }
                    0xC056 => { self.hires_mode = false; self.floating_bus() } 0xC057 => { self.hires_mode = true; self.floating_bus() }
                    0xC061 => if self.pushbuttons[0] { 0x80 } else { 0x00 },
                    0xC062 => if self.pushbuttons[1] { 0x80 } else { 0x00 },
                    0xC064..=0xC067 => {
                        let el = (self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64).saturating_sub(self.paddle_latch_cycle);
                        // 脈衝寬度：中低段維持線性 8+v*11(PREAD 讀值不變)；滿舵段額外拉長。
                        // 真實 Apple II 滿舵脈衝約 3300+ cycle(進入 PREAD 量程外的飽和區)，
                        // PREAD(含 DEY)仍讀 255，但每圈 54-cycle 的粗略讀桿迴圈(如 Championship
                        // Lode Runner，門檻 55 圈)需要這段額外長度。純線性只到 2813(v=255)，
                        // 讓 CLR 滿舵僅數到 52 < 55 → 右/下失效。對 v>192 追加斜率補進飽和區。
                        let v = self.paddles[(addr - 0xC064) as usize] as u64;
                        if el < (8 + v * 11 + v.saturating_sub(192) * 6) { 0x80 } else { 0x00 }
                    }
                    0xC070 => { self.paddle_latch_cycle = self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64; 0 }
                    // 其餘未驅動 I/O 讀取走 floating bus。
                    _ => self.floating_bus(),
                }
            }
            0xD000..=0xFFFF => {
                if self.lc_read_enable {
                    let idx = if addr < 0xE000 {
                        let base = (addr - 0xD000) as usize;
                        if self.lc_bank2 { base } else { base + 4096 }
                    } else {
                        (addr - 0xE000) as usize + 8192
                    };
                    if idx < 16384 { unsafe { crate::LC_RAM_16K[idx] } } else { 0xFF }
                } else {
                    // SAFETY: match arm 保證 addr ∈ [0xD000, 0xFFFF] → idx <= 12287；
                    // load_rom() 強制 rom.len() == 12288（指令擷取熱路徑，免邊界檢查）
                    unsafe { *self.rom.get_unchecked((addr - 0xD000) as usize) }
                }
            }
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.record_bus_access();
        match addr {
            // SAFETY: match arm 保證 addr <= 0xBFFF < 49152，免邊界檢查（最熱路徑）
            0x0000..=0xBFFF => unsafe { *(core::ptr::addr_of_mut!(crate::RAM_48K) as *mut u8).add(addr as usize) = data; },
            0xC000..=0xCFFF => {
                match addr {
                    0xC010 => self.keyboard_latch &= 0x7F,
                    0xC0E0..=0xC0EF => self.disk2.write_io(addr, data),
                    0xC030 => {
                        self.speaker = !self.speaker;
                        push_speaker_toggle((self.cpu_step_cycle_base + self.cpu_step_cycle_cursor as u64) as u32);
                    }
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
                    let idx = if addr < 0xE000 {
                        // $D000-$DFFF: Bank 2 (0..4095), Bank 1 (4096..8191)
                        let base = (addr - 0xD000) as usize;
                        if self.lc_bank2 { base } else { base + 4096 }
                    } else {
                        // $E000-$FFFF: (8192..16383)
                        (addr - 0xE000) as usize + 8192
                    };
                    if idx < 16384 {
                        unsafe { crate::LC_RAM_16K[idx] = data; }
                    }
                }
            }
        }
    }
}
