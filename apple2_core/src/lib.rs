#![no_std]

extern crate alloc;
use embedded_alloc::LlffHeap as Heap;
use core::panic::PanicInfo;
use core::ptr::addr_of_mut;

#[global_allocator]
static HEAP: Heap = Heap::empty();

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

static mut MACHINE: Option<Apple2Machine> = None;

const CHAR_ROM: &[u8; 2048] = include_bytes!("apple2_char.rom");
const SYS_ROM: &[u8; 12288] = include_bytes!("apple2_sys.rom");
const DISK2_ROM: &[u8; 256] = include_bytes!("disk2.rom");

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_char_rom_ptr() -> *const u8 {
    CHAR_ROM.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_init() {
    unsafe {
        static mut HEAP_MEM: [u8; 32768] = [0u8; 32768];
        HEAP.init(addr_of_mut!(HEAP_MEM) as usize, 32768);
        *addr_of_mut!(MACHINE) = Some(Apple2Machine::new());
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            m.mem.load_rom(SYS_ROM);
            m.mem.disk2.rom.copy_from_slice(DISK2_ROM);
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
            // m.step() 內部已經會精確同步磁碟 Tick (透過 finalize_cpu_step_cycles)
            // 這裡只需要單純執行指令即可
            for _ in 0..1000 { 
                total_cycles += m.step(); 
            }
        }
        total_cycles
    }
}

// 獲取 Apple II RAM 的指標 (48KB)
#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_ram_ptr() -> *const u8 {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            return m.mem.ram.as_ptr();
        }
        core::ptr::null()
    }
}

// 獲取影片模式：Bit 0: Text, Bit 1: Mixed, Bit 2: Page2, Bit 3: Hires
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

// 原本的 update_framebuffer 已不再需要
#[unsafe(no_mangle)]
pub extern "C" fn apple2_update_framebuffer() {}

// 原本的 get_framebuffer 也回傳 null
#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_framebuffer() -> *const u8 { core::ptr::null() }

#[unsafe(no_mangle)]
pub extern "C" fn apple2_is_ready_for_key() -> bool {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            return (m.mem.keyboard_latch & 0x80) == 0;
        }
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_handle_key(ascii: u8) {
    unsafe {
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            m.mem.keyboard_latch = ascii | 0x80;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_set_paddle(index: u8, value: u8) {
    unsafe {
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            if index < 4 {
                m.mem.paddles[index as usize] = value;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_set_button(index: u8, pressed: bool) {
    unsafe {
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            if index < 3 {
                m.mem.pushbuttons[index as usize] = pressed;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_disk_motor_status() -> bool {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            return m.mem.disk2.motor_on;
        }
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_is_track_dirty() -> bool {
    unsafe {
        if let Some(ref m) = *addr_of_mut!(MACHINE) {
            return m.mem.disk2.is_dirty;
        }
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_get_denibblized_track(out_buffer: *mut u8) -> u8 {
    unsafe {
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            let buf = &mut *(out_buffer as *mut [u8; 4096]);
            // 解碼目前的磁軌資料
            let sector_count = nibble::denibblize_track(&m.mem.disk2.current_track_data, m.mem.disk2.loaded_track_num, buf);

            if sector_count > 0 {
                // 修正：不再呼叫 nibblize_single_track 覆寫原始磁軌。
                // 這樣即使解碼器漏掉一個扇區，原始磁軌上的資料仍被保留，不會永久消失。
                m.mem.disk2.is_dirty = false;
                // 手動清除 dirty_mask，因為已經寫回 SD 了
                m.mem.disk2.current_track_data.dirty_mask.fill(false);
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
            if m.mem.disk2.is_disk_loaded && m.mem.disk2.needs_reload {
                return m.mem.disk2.current_track as i32;
            }
        }
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_load_track(track: u8, data: *const u8, size: u32) {
    unsafe {
        if let Some(ref mut m) = *addr_of_mut!(MACHINE) {
            let track_data = core::slice::from_raw_parts(data, size as usize);
            let nibblized = nibble::nibblize_single_track(track as usize, track_data);
            m.mem.disk2.load_track_data(track, nibblized);
            m.mem.disk2.needs_reload = false;
            m.mem.disk2.is_disk_loaded = true;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apple2_load_disk(data: *const u8, size: u32) {
    apple2_load_track(0, data, size);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop {} }
