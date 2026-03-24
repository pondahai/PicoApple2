extern crate alloc;

pub const NIBBLE_WRITE_TABLE: [u8; 64] = [
    0x96, 0x97, 0x9A, 0x9B, 0x9D, 0x9E, 0x9F, 0xA6, 0xA7, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB2, 0xB3,
    0xB4, 0xB5, 0xB6, 0xB7, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xCB, 0xCD, 0xCE, 0xCF, 0xD3,
    0xD6, 0xD7, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE5, 0xE6, 0xE7, 0xE9, 0xEA, 0xEB, 0xEC,
    0xED, 0xEE, 0xEF, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
];

pub const PHYS_TO_LOGICAL: [usize; 16] = [0, 7, 14, 6, 13, 5, 12, 4, 11, 3, 10, 2, 9, 1, 8, 15];

pub struct TrackData {
    pub raw_bytes: [u8; 6656],
    pub dirty_mask: [bool; 6656],
    pub length: usize,
}

impl TrackData {
    pub const fn new() -> Self {
        Self { 
            raw_bytes: [0; 6656], 
            dirty_mask: [false; 6656],
            length: 0 
        }
    }
    pub fn push(&mut self, val: u8) {
        if self.length < self.raw_bytes.len() {
            self.raw_bytes[self.length] = val;
            self.dirty_mask[self.length] = false; // 初始化產生的不計入 Dirty
            self.length += 1;
        }
    }
}

pub fn nibblize_single_track(track_num: usize, track_data: &[u8]) -> TrackData {
    let mut track_out = TrackData::new();
    for _ in 0..128 { track_out.push(0xFF); }

    for phys_pos in 0..16 {
        let logical_sector = PHYS_TO_LOGICAL[phys_pos];
        let sector_data = &track_data[logical_sector * 256..(logical_sector + 1) * 256];

        track_out.push(0xD5); track_out.push(0xAA); track_out.push(0x96);
        let vol: u8 = 254;
        let trk: u8 = track_num as u8;
        let sec: u8 = phys_pos as u8;
        let chk: u8 = vol ^ trk ^ sec;

        let encode4x4 = |val: u8| -> (u8, u8) { ((val >> 1) | 0xAA, val | 0xAA) };
        let (v1, v2) = encode4x4(vol); track_out.push(v1); track_out.push(v2);
        let (t1, t2) = encode4x4(trk); track_out.push(t1); track_out.push(t2);
        let (s1, s2) = encode4x4(sec); track_out.push(s1); track_out.push(s2);
        let (c1, c2) = encode4x4(chk); track_out.push(c1); track_out.push(c2);
        track_out.push(0xDE); track_out.push(0xAA); track_out.push(0xEB);

        for _ in 0..12 { track_out.push(0xFF); }

        track_out.push(0xD5); track_out.push(0xAA); track_out.push(0xAD);

        let swap2 = |b: u8| -> u8 { ((b & 0x01) << 1) | ((b & 0x02) >> 1) };
        let mut snib = [0u8; 86];
        for k in 0..86 {
            let b0 = swap2(sector_data[k] & 0x03);
            let b1 = swap2(sector_data[k + 86] & 0x03);
            let b2 = if k + 172 < 256 { swap2(sector_data[k + 172] & 0x03) } else { 0 };
            snib[k] = (b2 << 4) | (b1 << 2) | b0;
        }

        let mut last_val: u8 = 0;
        for i in 0..86 {
            let val6 = snib[i] & 0x3F;
            track_out.push(NIBBLE_WRITE_TABLE[(val6 ^ last_val) as usize]);
            last_val = val6;
        }
        for i in 0..256 {
            let val6 = sector_data[i] >> 2;
            track_out.push(NIBBLE_WRITE_TABLE[(val6 ^ last_val) as usize]);
            last_val = val6;
        }
        track_out.push(NIBBLE_WRITE_TABLE[last_val as usize]);
        track_out.push(0xDE); track_out.push(0xAA); track_out.push(0xEB);

        for _ in 0..32 { track_out.push(0xFF); }
    }
    while track_out.length < 6650 { track_out.push(0xFF); }
    track_out
}

pub fn denibblize_track(track_data: &TrackData, target_track: u8, out_buffer: &mut [u8; 4096]) -> u8 {
    let mut sector_status = [0u8; 16]; 
    let mut best_dirty_count = [0usize; 16]; // 記錄每個物理扇區發現過的最大 Dirty 位元組數
    let mut decode_table = [0u8; 256];
    for (i, &val) in NIBBLE_WRITE_TABLE.iter().enumerate() {
        decode_table[val as usize] = i as u8;
    }

    let raw = &track_data.raw_bytes;
    let dirty = &track_data.dirty_mask;
    let len = track_data.length;
    if len == 0 { return 0; }

    let decode4x4 = |v1: u8, v2: u8| -> u8 { (v1 << 1 | 1) & v2 };

    // 輔助函式：從任意位元偏移量獲取一個位元組
    let get_byte_bit_aligned = |bit_idx: usize| -> u8 {
        let byte_pos = (bit_idx / 8) % len;
        let shift = bit_idx % 8;
        if shift == 0 {
            raw[byte_pos]
        } else {
            let b1 = raw[byte_pos];
            let b2 = raw[(byte_pos + 1) % len];
            ((b1 << shift) | (b2 >> (8 - shift)))
        }
    };

    // 位元級掃描：總共有 len * 8 個可能的起始位元
    let mut b_idx = 0;
    while b_idx < len * 8 {
        // 搜尋標頭 D5 AA 96 (在當前位元偏移量下)
        if get_byte_bit_aligned(b_idx) == 0xD5 && 
           get_byte_bit_aligned(b_idx + 8) == 0xAA && 
           get_byte_bit_aligned(b_idx + 16) == 0x96 {
            
            // 雖然我們放寬檢查，但還是要解碼出物理扇區號碼
            let _hdr_trk = decode4x4(get_byte_bit_aligned(b_idx + 40), get_byte_bit_aligned(b_idx + 48));
            let phys_sec = decode4x4(get_byte_bit_aligned(b_idx + 56), get_byte_bit_aligned(b_idx + 64));
            
            if phys_sec < 16 {
                let logical_sec = PHYS_TO_LOGICAL[phys_sec as usize];
                
                // 在標頭之後尋找資料標記 D5 AA AD
                for j in 10..45 {
                    let d_idx = b_idx + (j * 8);
                    if get_byte_bit_aligned(d_idx) == 0xD5 && 
                       get_byte_bit_aligned(d_idx + 8) == 0xAA && 
                       get_byte_bit_aligned(d_idx + 16) == 0xAD {
                        
                        let mut snib = [0u8; 86];
                        let mut sector_data = [0u8; 256];
                        let mut last_val = 0u8;

                        // 讀取 342 個位元組的資料欄位
                        for k in 0..86 {
                            let nib = get_byte_bit_aligned(d_idx + 24 + (k * 8));
                            let val6 = decode_table[nib as usize] ^ last_val;
                            snib[k] = val6;
                            last_val = val6;
                        }
                        for k in 0..256 {
                            let nib = get_byte_bit_aligned(d_idx + 24 + 86 * 8 + (k * 8));
                            let val6 = decode_table[nib as usize] ^ last_val;
                            sector_data[k] = val6 << 2;
                            last_val = val6;
                        }

                        let disk_checksum = decode_table[get_byte_bit_aligned(d_idx + 24 + (86 + 256) * 8) as usize];
                        if last_val == disk_checksum {
                            // 計算此扇區範圍內的 Dirty 位元組數 (包含 Header 和 Data)
                            let mut current_dirty_count = 0;
                            // 檢查 Header 區域 (~11 bytes)
                            for k in 0..11 {
                                if dirty[((b_idx + k * 8) / 8) % len] { current_dirty_count += 1; }
                            }
                            // 檢查 Data 區域 (~345 bytes)
                            for k in 0..345 {
                                if dirty[((d_idx + k * 8) / 8) % len] { current_dirty_count += 1; }
                            }

                            // 優先級邏輯：如果這次發現的 Dirty 數量大於等於之前發現的，則採用。
                            // 這能確保軟體寫入的 (Dirty > 0) 覆蓋初始化產生的 (Dirty = 0)。
                            if current_dirty_count >= best_dirty_count[phys_sec as usize] {
                                let swap2 = |b: u8| -> u8 { ((b & 0x01) << 1) | ((b & 0x02) >> 1) };
                                for k in 0..86 {
                                    let b0 = swap2(snib[k] & 0x03);
                                    let b1 = swap2((snib[k] >> 2) & 0x03);
                                    let b2 = swap2((snib[k] >> 4) & 0x03);
                                    sector_data[k] |= b0;
                                    sector_data[k+86] |= b1;
                                    if k+172 < 256 { sector_data[k+172] |= b2; }
                                }
                                let offset = logical_sec * 256;
                                out_buffer[offset..offset+256].copy_from_slice(&sector_data);
                                sector_status[phys_sec as usize] = 1;
                                best_dirty_count[phys_sec as usize] = current_dirty_count;
                            }
                        }
                        b_idx = d_idx + (342 * 8);
                        break;
                    }
                }
            }
        }
        b_idx += 1; // 每次前進 1 位元
    }
    sector_status.iter().filter(|&&s| s > 0).count() as u8
}

