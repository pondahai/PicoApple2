pub const NIBBLE_WRITE_TABLE: [u8; 64] = [
    0x96, 0x97, 0x9A, 0x9B, 0x9D, 0x9E, 0x9F, 0xA6, 0xA7, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB2, 0xB3,
    0xB4, 0xB5, 0xB6, 0xB7, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xCB, 0xCD, 0xCE, 0xCF, 0xD3,
    0xD6, 0xD7, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE5, 0xE6, 0xE7, 0xE9, 0xEA, 0xEB, 0xEC,
    0xED, 0xEE, 0xEF, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
];

pub const PHYS_TO_LOGICAL: [usize; 16] = [0, 7, 14, 6, 13, 5, 12, 4, 11, 3, 10, 2, 9, 1, 8, 15];

pub fn nibblize_into_track(track_num: usize, track_data: &[u8], raw_out: &mut [u8], dirty_out: &mut [bool]) {
    let mut length = 0;
    let mut push = |val: u8| {
        if length < raw_out.len() {
            raw_out[length] = val;
            dirty_out[length] = false;
            length += 1;
        }
    };

    // 增加 Lead-in 空間
    for _ in 0..128 { push(0xFF); }
    for phys_pos in 0..16 {
        let logical_sector = PHYS_TO_LOGICAL[phys_pos];
        let sector_data = &track_data[logical_sector * 256..(logical_sector + 1) * 256];
        
        // Address Header
        push(0xD5); push(0xAA); push(0x96);
        let vol: u8 = 254; let trk: u8 = track_num as u8; let sec: u8 = phys_pos as u8; let chk: u8 = vol ^ trk ^ sec;
        let encode4x4 = |val: u8| -> (u8, u8) { ((val >> 1) | 0xAA, val | 0xAA) };
        let (v1, v2) = encode4x4(vol); push(v1); push(v2);
        let (t1, t2) = encode4x4(trk); push(t1); push(t2);
        let (s1, s2) = encode4x4(sec); push(s1); push(s2);
        let (c1, c2) = encode4x4(chk); push(c1); push(c2);
        push(0xDE); push(0xAA); push(0xEB);
        
        // 減少間隙以獲得更多時序餘量 (28 -> 12)
        for _ in 0..12 { push(0xFF); }
        
        // Data Header
        push(0xD5); push(0xAA); push(0xAD);
        let swap2 = |b: u8| -> u8 { ((b & 0x01) << 1) | ((b & 0x02) >> 1) };
        let mut snib = [0u8; 86];
        for k in 0..86 {
            let b0 = swap2(sector_data[k] & 0x03);
            let b1 = swap2(sector_data[k + 86] & 0x03);
            let b2 = if k + 172 < 256 { swap2(sector_data[k + 172] & 0x03) } else { 0 };
            snib[k] = (b2 << 4) | (b1 << 2) | b0;
        }
        let mut last_val: u8 = 0;
        for i in 0..86 { let val6 = snib[i] & 0x3F; push(NIBBLE_WRITE_TABLE[(val6 ^ last_val) as usize]); last_val = val6; }
        for i in 0..256 { let val6 = sector_data[i] >> 2; push(NIBBLE_WRITE_TABLE[(val6 ^ last_val) as usize]); last_val = val6; }
        push(NIBBLE_WRITE_TABLE[last_val as usize]); 
        push(0xDE); push(0xAA); push(0xEB);
        
        // 減少磁區間隙 (28 -> 20) 以騰出空間給軟體寫入的時序抖動
        for _ in 0..20 { push(0xFF); }
    }
    while length < 6656 { raw_out[length] = 0xFF; length += 1; }
}

pub fn denibblize_track(raw: &[u8], dirty: &[bool], len: usize, _track: u8, out_buffer: &mut [u8; 4096]) -> u8 {
    let mut sector_status = [0u8; 16]; 
    let mut decode_table = [0xFFu8; 256];
    for (i, &val) in NIBBLE_WRITE_TABLE.iter().enumerate() { decode_table[val as usize] = i as u8; }
    if len == 0 { return 0; }
    
    let decode4x4 = |v1: u8, v2: u8| -> u8 { ((v1 << 1) | 1) & v2 };

    let mut i = 0;
    while i < len {
        // 搜尋 Address Header: D5 AA 96
        if raw[i] == 0xD5 && raw[(i + 1) % len] == 0xAA && raw[(i + 2) % len] == 0x96 {
            let s1 = raw[(i + 7) % len];
            let s2 = raw[(i + 8) % len];
            let phys_sec = decode4x4(s1, s2);
            
            if phys_sec < 16 {
                let logical_sec = PHYS_TO_LOGICAL[phys_sec as usize];
                let mut found_data = false;

                // 擴大搜尋範圍 (10..120) 以應對軟體寫入時的長 Gap
                for gap in 10..120 {
                    let d_idx = (i + gap) % len;
                    if raw[d_idx] == 0xD5 && raw[(d_idx + 1) % len] == 0xAA && raw[(d_idx + 2) % len] == 0xAD {
                        let mut snib = [0u8; 86];
                        let mut sector_data = [0u8; 256];
                        let mut last_val = 0u8;
                        let mut checksum_ok = true;

                        // 解碼 6&2 GCR 資料區塊
                        for k in 0..342 {
                            let nib = raw[(d_idx + 3 + k) % len];
                            let idx = decode_table[nib as usize];
                            if idx == 0xFF { checksum_ok = false; break; }
                            let val6 = idx ^ last_val;
                            if k < 86 { snib[k] = val6; } else { sector_data[k - 86] = val6 << 2; }
                            last_val = val6;
                        }

                        if checksum_ok {
                            let ck_nib = raw[(d_idx + 3 + 342) % len];
                            if decode_table[ck_nib as usize] == last_val {
                                let mut has_dirty = false;
                                for k in 0..350 { if dirty[(d_idx + k) % len] { has_dirty = true; break; } }
                                
                                if has_dirty || sector_status[phys_sec as usize] == 0 {
                                    let swap2 = |b: u8| -> u8 { ((b & 0x01) << 1) | ((b & 0x02) >> 1) };
                                    for k in 0..86 {
                                        sector_data[k] |= swap2(snib[k] & 0x03);
                                        sector_data[k + 86] |= swap2((snib[k] >> 2) & 0x03);
                                        if k + 172 < 256 { sector_data[k + 172] |= swap2((snib[k] >> 4) & 0x03); }
                                    }
                                    let offset = logical_sec * 256;
                                    out_buffer[offset..offset + 256].copy_from_slice(&sector_data);
                                    sector_status[phys_sec as usize] = 1;
                                }
                                found_data = true;
                                i = (d_idx + 345) % len; // 成功後直接跳過磁區，避免重複搜尋
                                if i <= (d_idx % len) { i = len; } // 處理跨越邊界的情況
                                break; 
                            }
                        }
                    }
                }
                if found_data { continue; }
            }
        }
        i += 1;
    }
    sector_status.iter().filter(|&&s| s > 0).count() as u8
}
