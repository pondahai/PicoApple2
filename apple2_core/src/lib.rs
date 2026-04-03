#![no_std]

use core::panic::PanicInfo;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

#[unsafe(no_mangle)]
pub extern "C" fn _critical_section_1_0_acquire() -> u8 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn _critical_section_1_0_release(_token: u8) {}

pub mod cpu;
pub mod disk2;
pub mod instructions;
pub mod machine;
pub mod memory;
pub mod nibble;
pub mod video;

use machine::Apple2Machine;

// --- [最穩定的全域變數定義] ---
pub static mut RAM_48K: [u8; 49152] = [0u8; 49152];
pub static mut LC_RAM_16K: [u8; 16384] = [0u8; 16384];
pub static mut TRACK_DATA_RAW: [u8; 6656] = [0u8; 6656];
pub static mut TRACK_DATA_DIRTY: [bool; 6656] = [false; 6656];
pub static mut WRITE_LOG: [u8; 1024] = [0; 1024];
pub static mut WRITE_LOG_IDX: usize = 0;

pub static BEAM_Y: AtomicU32 = AtomicU32::new(0);
pub static VIDEO_MODE: AtomicU8 = AtomicU8::new(0);

static mut MACHINE: Option<Apple2Machine> = None;

const CHAR_ROM: &[u8; 2048] = include_bytes!("apple2_char.rom");
const SYS_ROM: &[u8; 12288] = include_bytes!("apple2_sys.rom");
const DISK_ROM_P5: &[u8; 256] = include_bytes!("disk2_p5.rom");

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_char_rom_ptr() -> *const u8 { CHAR_ROM.as_ptr() }

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_disk_rom_ptr() -> *const u8 { DISK_ROM_P5.as_ptr() }

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_ram_ptr() -> *const u8 { unsafe { (addr_of_mut!(RAM_48K) as *mut u8) } }

#[unsafe(no_mangle)]
pub extern "C" fn apple2_init() {
    unsafe {
        *addr_of_mut!(MACHINE) = Some(Apple2Machine::new());
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            m.mem.load_rom(SYS_ROM);
            m.power_on();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_reset() {
    unsafe { if let Some(ref mut m) = *addr_of_mut!(MACHINE) { m.power_on(); } }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_warm_reset() {
    unsafe { if let Some(ref mut m) = *addr_of_mut!(MACHINE) { m.reset(); } }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_tick() -> u32 {
    unsafe {
        let mut total_cycles = 0;
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            // 降回 300 條指令以確保鎖定時間不會過長導致 Core 1 等待超時
            for _ in 0..300 {
                total_cycles += m.step();
                if m.mem.disk2.needs_reload {
                    break;
                }
            }
        }
        total_cycles
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_video_mode() -> u8 {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            let mut mode = 0u8;
            if m.mem.text_mode { mode |= 0x01; }
            if m.mem.mixed_mode { mode |= 0x02; }
            if m.mem.page2 { mode |= 0x04; }
            if m.mem.hires_mode { mode |= 0x08; }
            return mode;
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_beam_y() -> u16 {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            // 17030 cycles per frame, 65 cycles per line.
            return ((m.total_cycles % 17030) / 65) as u16;
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_is_ready_for_key() -> bool {
    unsafe { if let Some(ref m) = *addr_of_mut!(MACHINE) { return (m.mem.keyboard_latch & 0x80) == 0; } false }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_handle_key(ascii: u8) {
    unsafe { if let Some(ref mut m) = *addr_of_mut!(MACHINE) { m.mem.keyboard_latch = ascii | 0x80; } }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_set_paddle(index: u8, value: u8) {
    unsafe { if let Some(ref mut m) = *addr_of_mut!(MACHINE) { if index < 4 { m.mem.paddles[index as usize] = value; } } }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_set_button(index: u8, pressed: bool) {
    unsafe { if let Some(ref mut m) = *addr_of_mut!(MACHINE) { if index < 3 { m.mem.pushbuttons[index as usize] = pressed; } } }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_disk_motor_status() -> bool {
    unsafe { if let Some(ref m) = *addr_of_mut!(MACHINE) { return m.mem.disk2.motor_on; } false }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_disk_byte_index() -> u32 {
    unsafe { if let Some(ref m) = *addr_of_mut!(MACHINE) { return m.mem.disk2.byte_index as u32; } 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_is_track_dirty() -> bool {
    unsafe { if let Some(ref m) = *addr_of_mut!(MACHINE) { return m.mem.disk2.is_dirty; } false }
}

pub static mut LAST_DENIB_ERROR: u32 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_denibblize_error() -> u32 {
    unsafe { LAST_DENIB_ERROR }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_denibblized_track(out_buffer: *mut u8) -> u8 {
    unsafe {
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            let buf = &mut *(out_buffer as *mut [u8; 4096]);
            let raw_ptr = addr_of_mut!(TRACK_DATA_RAW) as *const [u8; 6656];
            let dirty_ptr = addr_of_mut!(TRACK_DATA_DIRTY) as *const [bool; 6656];
            LAST_DENIB_ERROR = 0;
            let sector_count = nibble::denibblize_track(&*raw_ptr, &*dirty_ptr, 6656, m.mem.disk2.loaded_track_num, buf);
            if sector_count > 0 || LAST_DENIB_ERROR != 0 {
                // Return exactly what was recorded!
                m.mem.disk2.is_dirty = false;
                (addr_of_mut!(TRACK_DATA_DIRTY) as *mut [bool; 6656]).as_mut().unwrap().fill(false);
            }
            return sector_count;
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_needs_disk_reload() -> i32 {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            if m.mem.disk2.is_disk_loaded && m.mem.disk2.needs_reload { return m.mem.disk2.current_track as i32; }
        }
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_load_track(track: u8, data: *const u8, size: u32) {
    unsafe {
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            let track_data_in = core::slice::from_raw_parts(data, size as usize);
            let raw_ptr = addr_of_mut!(TRACK_DATA_RAW) as *mut [u8; 6656];
            let dirty_ptr = addr_of_mut!(TRACK_DATA_DIRTY) as *mut [bool; 6656];
            nibble::nibblize_into_track(track as usize, track_data_in, &mut *raw_ptr, &mut *dirty_ptr);
            m.mem.disk2.needs_reload = false;
            m.mem.disk2.is_disk_loaded = true;
            m.mem.disk2.loaded_track_num = track;
            m.mem.disk2.current_track = track as usize;
            m.mem.disk2.byte_index = 0; 
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_load_disk(data: *const u8, size: u32) { apple2_load_track(0, data, size); }

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_write_log(out_buffer: *mut u8) -> u16 {
    unsafe {
        let count = WRITE_LOG_IDX.min(1024);
        for i in 0..count {
            *out_buffer.add(i) = WRITE_LOG[i];
        }
        WRITE_LOG_IDX = 0;
        count as u16
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_cpu_state(out_pc: *mut u16, out_a: *mut u8, out_x: *mut u8, out_y: *mut u8, out_sp: *mut u8, out_status: *mut u8) {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            *out_pc = m.cpu.pc;
            *out_a = m.cpu.a;
            *out_x = m.cpu.x;
            *out_y = m.cpu.y;
            *out_sp = m.cpu.sp;
            *out_status = m.cpu.status.to_byte();
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Trigger Cortex-M0+ System Reset (SYSRESETREQ)
    unsafe {
        let aircr = 0xE000ED0C as *mut u32;
        core::ptr::write_volatile(aircr, 0x05FA0004);
    }
    loop {}
}
