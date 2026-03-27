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
    // 增加初始同步位元
    for _ in 0..128 { track_out.push(0xFF); }

    for phys_pos in 0..16 {
        let logical_sector = PHYS_TO_LOGICAL[phys_pos];
        let sector_data = &track_data[logical_sector * 256..(logical_sector + 1) * 256];

        // --- 扇區標頭 (Header Field) ---
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

        // 間隔 (Gap 2)
        for _ in 0..12 { track_out.push(0xFF); }

        // --- 資料欄位 (Data Field) ---
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

        // 間隔 (Gap 3)
        for _ in 0..28 { track_out.push(0xFF); }
    }
    // 填滿至最大可用長度 (6656)，給予寫入時序極大的寬放空間
    while track_out.length < 6650 { track_out.push(0xFF); }
    track_out
}

pub fn denibblize_track(track_data: &TrackData, _target_track: u8, out_buffer: &mut [u8; 4096]) -> u8 {
    let mut sector_status = [0u8; 16]; 
    let mut decode_table = [0u8; 256];
    for (i, &val) in NIBBLE_WRITE_TABLE.iter().enumerate() {
        decode_table[val as usize] = i as u8;
    }

    let raw = &track_data.raw_bytes;
    let dirty = &track_data.dirty_mask;
    let len = track_data.length;
    if len == 0 { return 0; }

    let decode4x4 = |v1: u8, v2: u8| -> u8 { ((v1 << 1) | 1) & v2 };

    let get_byte_bit_aligned = |bit_idx: usize| -> u8 {
        let byte_pos = (bit_idx / 8) % len;
        let shift = bit_idx % 8;
        if shift == 0 {
            raw[byte_pos]
        } else {
            let b1 = raw[byte_pos];
            let b2 = raw[(byte_pos + 1) % len];
            (b1 << shift) | (b2 >> (8 - shift))
        }
    };

    let mut b_idx = 0;
    while b_idx < len * 8 {
        // 搜尋標頭 D5 AA 96
        if get_byte_bit_aligned(b_idx) == 0xD5 && 
           get_byte_bit_aligned(b_idx + 8) == 0xAA && 
           get_byte_bit_aligned(b_idx + 16) == 0x96 {
            
            let phys_sec = decode4x4(get_byte_bit_aligned(b_idx + 56), get_byte_bit_aligned(b_idx + 64));
            
            if phys_sec < 16 {
                let logical_sec = PHYS_TO_LOGICAL[phys_sec as usize];
                
                // 極大幅度放寬資料標記搜尋範圍 (應對 INIT 時的長同步區)
                for j in 5..120 {
                    let d_idx = b_idx + (j * 8);
                    if get_byte_bit_aligned(d_idx) == 0xD5 && 
                       get_byte_bit_aligned(d_idx + 8) == 0xAA && 
                       get_byte_bit_aligned(d_idx + 16) == 0xAD {
                        
                        let mut snib = [0u8; 86];
                        let mut sector_data = [0u8; 256];
                        let mut last_val = 0u8;

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
                            let mut has_dirty = false;
                            for k in 0..11 {
                                if dirty[((b_idx + k * 8) / 8) % len] { has_dirty = true; break; }
                            }
                            if !has_dirty {
                                for k in 0..345 {
                                    if dirty[((d_idx + k * 8) / 8) % len] { has_dirty = true; break; }
                                }
                            }

                            if has_dirty || sector_status[phys_sec as usize] == 0 {
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
                            }
                        }
                        b_idx = d_idx + (342 * 8);
                        break;
                    }
                }
            }
        }
        b_idx += 1; 
    }
    sector_status.iter().filter(|&&s| s > 0).count() as u8
}


