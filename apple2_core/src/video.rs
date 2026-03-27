use crate::memory::Apple2Memory;

/// The Apple II Text Screen is 40 columns by 24 rows
pub const SCREEN_WIDTH: usize = 280; // 40 cols * 7 pixels per char
pub const SCREEN_HEIGHT: usize = 192; // 24 rows * 8 pixels per char

/// Apple II Text Video memory is interleaved in a strange way
/// This maps visual row (0-23) to memory row offset (0x0400 to 0x07F8)
#[allow(dead_code)]
const ROW_ADDRESSES: [u16; 24] = [
    0x0400,
    0x0480,
    0x0500,
    0x0580,
    0x0428,
    0x04A8,
    0x0528,
    0x05A8,
    0x0450,
    0x04D0,
    0x0550,
    0x05D0,
    0x0400 + 0x28,
    0x0480 + 0x28,
    0x0500 + 0x28,
    0x0580 + 0x28,
    0x0428 + 0x28,
    0x04A8 + 0x28,
    0x0528 + 0x28,
    0x05A8 + 0x28,
    0x0450 + 0x28,
    0x04D0 + 0x28,
    0x0550 + 0x28,
    0x05D0 + 0x28, // Wait, this logic for the last 1/3 is slightly off, let's use the explicit calculated list:
];

pub struct Video {
    pub frame_buffer: [u32; SCREEN_WIDTH * SCREEN_HEIGHT],
}

impl Video {
    pub fn new() -> Self {
        Self {
            frame_buffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT], // XRGB 32-bit
        }
    }

    /// Calculate the memory address of the first character of a given row
    pub fn get_text_row_addr(row: usize) -> u16 {
        // Apple II text rows are weirdly interleaved:

        // Row 0 = 0x0400, Row 1 = 0x0480, Row 2 = 0x0500...
        // Row 8 = 0x0428, Row 9 = 0x04A8...
        // Row 16 = 0x0450...
        let base = 0x0400;
        let block = row / 8;
        let offset = row % 8;
        base + (offset * 128) as u16 + (block * 40) as u16
    }

    /// Render the Apple II Text Mode frame into the 32-bit framebuffer.
    /// Needs a character ROM font to actually draw the letters.
    pub fn render_text_frame(&mut self, _mem: &Apple2Memory, char_rom: &[u8; 2048]) {
        let white_color = 0x00_FF_FF_FF; // ARGB White
        let black_color = 0x00_00_00_00; // ARGB Black

        let is_blink_on = true;

        for row in 0..24 {
            let row_addr = Self::get_text_row_addr(row);

            for col in 0..40 {
                // Read the character code from Video RAM
                let char_code = unsafe { crate::RAM_48K[(row_addr + col as u16) as usize] };

                // Text Modes:
                // $00-$3F: Inverse (uppercase only)
                // $40-$7F: Flashing (uppercase only)
                // $80-$FF: Normal (upper and lowercase, etc.)
                let is_inverse = char_code < 0x40;
                let is_flashing = char_code >= 0x40 && char_code < 0x80;

                let invert_colors = is_inverse || (is_flashing && is_blink_on);

                // The Apple II character generator ROM is 2KB.
                // We map $00-$3F and $40-$7F to the same font data as $C0-$FF
                let char_index = match char_code {
                    0x00..=0x3F => char_code as usize + 0x40, // Map to uppercase
                    0x40..=0x7F => char_code as usize,        // Map to uppercase
                    _ => char_code as usize,                  // Use as is
                };

                // Render each row of the character (8 pixels high)
                for char_y in 0..8 {
                    let font_byte = char_rom[char_index * 8 + char_y];

                    let screen_y = row * 8 + char_y;

                    // Each character is nominally 7 pixels wide on screen
                    for char_x in 0..7 {
                        let screen_x = col * 7 + char_x;

                        // Apple II characters form their pixels from MSB to LSB usually,
                        // so bit 6 is the leftmost pixel, bit 0 is the rightmost part.
                        let mut pixel_on = (font_byte & (1 << (6 - char_x))) != 0;

                        if invert_colors {
                            pixel_on = !pixel_on;
                        }

                        let color = if pixel_on { white_color } else { black_color };
                        self.frame_buffer[screen_y * SCREEN_WIDTH + screen_x] = color;
                    }
                }
            }
        }
    }

    /// Convert Apple II 4-bit Lo-Res color index to 32-bit ARGB
    fn get_lores_color(color_index: u8) -> u32 {
        match color_index & 0x0F {
            0x0 => 0xFF_00_00_00, // Black
            0x1 => 0xFF_DD_0B_41, // Deep Red
            0x2 => 0xFF_00_00_8A, // Dark Blue
            0x3 => 0xFF_D3_59_D6, // Purple
            0x4 => 0xFF_00_75_00, // Dark Green
            0x5 => 0xFF_60_60_60, // Dark Gray
            0x6 => 0xFF_1F_3A_FF, // Medium Blue
            0x7 => 0xFF_63_AF_FF, // Light Blue
            0x8 => 0xFF_51_44_00, // Brown
            0x9 => 0xFF_ED_79_00, // Orange
            0xA => 0xFF_A0_A0_A0, // Light Gray
            0xB => 0xFF_FF_91_FF, // Pink
            0xC => 0xFF_1E_D6_00, // Light Green
            0xD => 0xFF_D6_D6_00, // Yellow
            0xE => 0xFF_65_FF_A2, // Aquamarine
            0xF => 0xFF_FF_FF_FF, // White
            _ => 0xFF_00_00_00,
        }
    }

    /// Render the Apple II Low-Res Graphics Mode frame into the 32-bit framebuffer.
    /// In mixed mode, the bottom 4 lines of text (rows 20-23) are rendered as text.
    pub fn render_lores_frame(&mut self, mem: &Apple2Memory, char_rom: &[u8; 2048]) {
        let is_blink_on = true;
        let white_color = 0x00_FF_FF_FF; // ARGB White
        let black_color = 0x00_00_00_00; // ARGB Black

        for row in 0..24 {
            let row_addr = Self::get_text_row_addr(row);

            // Handle Mixed Mode (bottom 4 rows are text)
            if mem.mixed_mode && row >= 20 {
                // Render as text
                for col in 0..40 {
                    let char_code = unsafe { crate::RAM_48K[(row_addr + col as u16) as usize] };
                    let is_inverse = char_code < 0x40;
                    let is_flashing = char_code >= 0x40 && char_code < 0x80;
                    let invert_colors = is_inverse || (is_flashing && is_blink_on);

                    let char_index = match char_code {
                        0x00..=0x3F => char_code as usize + 0x40,
                        0x40..=0x7F => char_code as usize,
                        _ => char_code as usize,
                    };

                    for char_y in 0..8 {
                        let font_byte = char_rom[char_index * 8 + char_y];
                        let screen_y = row * 8 + char_y;

                        for char_x in 0..7 {
                            let screen_x = col * 7 + char_x;
                            let mut pixel_on = (font_byte & (1 << (6 - char_x))) != 0;
                            if invert_colors {
                                pixel_on = !pixel_on;
                            }
                            let color = if pixel_on { white_color } else { black_color };
                            self.frame_buffer[screen_y * SCREEN_WIDTH + screen_x] = color;
                        }
                    }
                }
                continue;
            }

            // Render as Lo-Res Graphics
            for col in 0..40 {
                // Read the Lo-Res color byte
                let color_byte = unsafe { crate::RAM_48K[(row_addr + col as u16) as usize] };

                // Top block is the lower 4 bits (pixels 0-3 of the character cell)
                let top_color = Self::get_lores_color(color_byte & 0x0F);
                // Bottom block is the upper 4 bits (pixels 4-7 of the character cell)
                let bottom_color = Self::get_lores_color((color_byte & 0xF0) >> 4);

                // A text cell is 8 pixels high and 7 contiguous pixels wide
                for screen_y in (row * 8)..(row * 8 + 4) {
                    for screen_x in (col * 7)..(col * 7 + 7) {
                        self.frame_buffer[screen_y * SCREEN_WIDTH + screen_x] = top_color;
                    }
                }

                for screen_y in (row * 8 + 4)..(row * 8 + 8) {
                    for screen_x in (col * 7)..(col * 7 + 7) {
                        self.frame_buffer[screen_y * SCREEN_WIDTH + screen_x] = bottom_color;
                    }
                }
            }
        }
    }

    /// Calculate the memory address of the first byte of a given Hi-Res row
    pub fn get_hires_row_addr(row: usize, page2: bool) -> u16 {
        // Hi-Res memory is also interleaved, similar to text but spread over 8KB
        // Page 1: 0x2000 - 0x3FFF
        // Page 2: 0x4000 - 0x5FFF
        let base = if page2 { 0x4000 } else { 0x2000 };

        let block = row / 64; // 0, 1, 2
        let sub_block = (row % 64) / 8; // 0..7
        let offset = row % 8; // 0..7

        base + (offset * 1024) as u16 + (sub_block * 128) as u16 + (block * 40) as u16
    }

    /// Render the Apple II High-Res Graphics Mode frame into the 32-bit framebuffer.
    pub fn render_hires_frame(&mut self, mem: &Apple2Memory, char_rom: &[u8; 2048]) {
        let is_blink_on = true;
        let white_color = 0x00_FF_FF_FF; // ARGB White
        let black_color = 0x00_00_00_00; // ARGB Black

        // Hi-Res Colors (approximate ARGB)
        let black_color_hr = 0xFF_00_00_00;
        let green_color_hr = 0xFF_1E_D6_00; // Light Green
        let purple_color_hr = 0xFF_D3_59_D6; // Purple
        let orange_color_hr = 0xFF_ED_79_00; // Orange
        let blue_color_hr = 0xFF_63_AF_FF; // Light Blue

        for row in 0..192 {
            // Handle Mixed Mode (bottom 32 scanlines / 4 text rows are text)
            if mem.mixed_mode && row >= 160 {
                let text_row = row / 8;
                let char_y = row % 8;
                let row_addr = Self::get_text_row_addr(text_row);

                for col in 0..40 {
                    let char_code = unsafe { crate::RAM_48K[(row_addr + col as u16) as usize] };
                    let is_inverse = char_code < 0x40;
                    let is_flashing = char_code >= 0x40 && char_code < 0x80;
                    let invert_colors = is_inverse || (is_flashing && is_blink_on);

                    let char_index = match char_code {
                        0x00..=0x3F => char_code as usize + 0x40,
                        0x40..=0x7F => char_code as usize,
                        _ => char_code as usize,
                    };

                    let font_byte = char_rom[char_index * 8 + char_y];
                    let screen_y = row;

                    for char_x in 0..7 {
                        let screen_x = col * 7 + char_x;
                        let mut pixel_on = (font_byte & (1 << (6 - char_x))) != 0;
                        if invert_colors {
                            pixel_on = !pixel_on;
                        }
                        let color = if pixel_on { white_color } else { black_color };
                        self.frame_buffer[screen_y * SCREEN_WIDTH + screen_x] = color;
                    }
                }
                continue;
            }

            // Render as Hi-Res Graphics
            let row_addr = Self::get_hires_row_addr(row, mem.page2);
            let mut prev_bit = false;

            for col in 0..40 {
                let byte = unsafe { crate::RAM_48K[(row_addr + col as u16) as usize] };

                // Bit 7 is the palette shift/artifact bit
                let shift_bit = (byte & 0x80) != 0;

                // Bits 0-6 are the 7 pixels (LSB to MSB)
                for bit_idx in 0..7 {
                    let current_bit = (byte & (1 << bit_idx)) != 0;
                    let next_bit = if bit_idx < 6 {
                        (byte & (1 << (bit_idx + 1))) != 0
                    } else if col < 39 {
                        (unsafe { crate::RAM_48K[(row_addr + col as u16 + 1) as usize] } & 0x01) != 0
                    } else {
                        false
                    };

                    let screen_x = col * 7 + bit_idx;
                    let screen_y = row;

                    // NTSC Artifact color approximation rules:
                    // If current bit is OFF:
                    //   If bordered by ON bits -> depends on odd/even column, but usually just black unless bleeding.
                    // If current bit is ON:
                    //   If prev AND next are ON -> White
                    //   If just ON -> Color depends on even/odd column AND the shift bit (bit 7)
                    let color = if current_bit {
                        if prev_bit || next_bit {
                            white_color
                        } else {
                            // Single pixel -> Color
                            let is_even_col = (screen_x % 2) == 0;
                            match (is_even_col, shift_bit) {
                                (true, false) => purple_color_hr, // Even col, shift=0 -> Purple
                                (false, false) => green_color_hr, // Odd col, shift=0 -> Green
                                (true, true) => blue_color_hr,    // Even col, shift=1 -> Blue
                                (false, true) => orange_color_hr, // Odd col, shift=1 -> Orange
                            }
                        }
                    } else {
                        black_color_hr
                    };

                    self.frame_buffer[screen_y * SCREEN_WIDTH + screen_x] = color;
                    prev_bit = current_bit;
                }
            }
        }
    }
}
