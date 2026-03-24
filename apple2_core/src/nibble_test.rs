#[cfg(test)]
mod tests {
    use crate::nibble::nibblize_dsk;

    #[test]
    fn dump_track_0() {
        let disk_data = std::fs::read("../roms/MASTER.DSK").unwrap();
        let tracks = nibblize_dsk(&disk_data);

        let track0 = &tracks[0];
        // Use track0.raw_bytes for indexing

        // Find the first Data Field Epilogue (DE AA EB)
        let mut found = false;
        let limit = track0.length - 3;
        for i in 0..limit {
            if track0.raw_bytes[i] == 0xDE
                && track0.raw_bytes[i + 1] == 0xAA
                && track0.raw_bytes[i + 2] == 0xEB
            {
                println!("Found Data Epilogue at index {}", i);
                found = true;
                break;
            }
        }
        assert!(found, "Failed to find any data epilogue!");
    }
}
