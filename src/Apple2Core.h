#ifndef APPLE2_CORE_H
#define APPLE2_CORE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// 系統核心控制
void apple2_init();
void apple2_reset();
void apple2_warm_reset();
uint32_t apple2_tick(); 

// 鍵盤與搖桿輸入
void apple2_handle_key(uint8_t ascii);
bool apple2_is_ready_for_key();
void apple2_set_paddle(uint8_t index, uint8_t value);
void apple2_set_button(uint8_t index, bool pressed);

// 顯示輸出 (新介面：零緩衝渲染)
uint8_t apple2_get_video_mode(); // Bit 0:Text, 1:Mixed, 2:Page2, 3:Hires
uint16_t apple2_get_beam_y();
const uint8_t* apple2_get_ram_ptr(); 
const uint8_t* apple2_get_char_rom_ptr(); 

// 磁碟狀態與 I/O
bool apple2_get_disk_motor_status();
bool apple2_is_track_dirty(); // 檢查當前磁軌是否被修改
uint8_t apple2_get_denibblized_track(uint8_t* out_buffer); // 獲取去編碼後的 4096 bytes 扇區資料，回傳有效扇區數
int32_t apple2_needs_disk_reload(); 
void apple2_load_track(uint8_t track, const uint8_t* data, uint32_t size);

// 兼容舊介面 (已廢棄)
void apple2_update_framebuffer();
const uint8_t* apple2_get_framebuffer();
void apple2_load_disk(const uint8_t* data, uint32_t size);

#ifdef __cplusplus
}
#endif

#endif
