// 監視器(F6) peek/poke 回歸測試：讀寫不得翻 soft switch，I/O 區不可寫。
#[cfg(test)]
mod tests {
    use crate::memory::Apple2Memory;

    #[test]
    fn peek_io_does_not_flip_switches() {
        let mem = Apple2Memory::new();
        assert!(mem.text_mode);
        assert_eq!(mem.peek(0xC050), None);   // TXTCLR：真讀會關掉文字模式
        assert_eq!(mem.peek(0xC030), None);   // 喇叭
        assert!(mem.text_mode);
        assert!(!mem.speaker);
    }

    #[test]
    fn poke_rejects_io_space() {
        let mut mem = Apple2Memory::new();
        assert!(!mem.poke(0xC051, 0x00));
        assert!(!mem.poke(0xC0E9, 0x00));     // 磁碟馬達
        assert!(mem.text_mode);
    }

    #[test]
    fn poke_high_memory_goes_to_selected_lc_bank() {
        let mut mem = Apple2Memory::new();
        mem.lc_bank2 = false;                 // bank 1
        mem.lc_write_enable = false;          // 寫保護也照寫
        assert!(mem.poke(0xD123, 0x5A));
        mem.lc_read_enable = true;
        assert_eq!(mem.peek(0xD123), Some(0x5A));
        mem.lc_bank2 = true;                  // 換到 bank 2 就看不到
        assert_ne!(mem.peek(0xD123), Some(0x5A));
    }
}
