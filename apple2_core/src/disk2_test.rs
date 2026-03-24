#[cfg(test)]
mod tests {
    use crate::disk2::Disk2;

    #[test]
    fn write_path_commits_latch_while_q7_is_on() {
        let mut disk = Disk2::new();
        disk.is_disk_loaded = true;
        disk.current_track = 0;
        disk.tracks[0].length = 1;
        disk.tracks[0].raw_bytes[0] = 0x11;

        disk.write_io(0xC0E9, 0); // Motor on
        disk.write_io(0xC0EF, 0); // Q7 = 1
        disk.write_io(0xC0ED, 0xAA); // Q6 = 1 (write-load), latch = 0xAA
        disk.write_io(0xC0EC, 0); // Q6 = 0 (write-shift)
        disk.tick(32);
        assert_eq!(disk.tracks[0].raw_bytes[0], 0xAA);
    }

    #[test]
    fn write_protect_probe_reads_are_compatible_on_q6_on_and_q7_off() {
        let mut disk = Disk2::new();
        disk.write_io(0xC0E9, 0); // Motor on

        assert_eq!(disk.read_io(0xC0ED), 0x00); // Q6_ON probe path
        assert_eq!(disk.read_io(0xC0EE), 0x00); // Default non-sensed read value
    }

    #[test]
    fn only_q6_on_write_loads_latch() {
        let mut disk = Disk2::new();
        disk.write_io(0xC0EF, 0); // Q7 = 1
        disk.write_io(0xC0ED, 0xAA); // Q6 = 1 and load
        assert_eq!(disk.data_latch, 0xAA);

        disk.write_io(0xC0EF, 0x55); // Q7 write should not replace latch payload
        assert_eq!(disk.data_latch, 0xAA);
    }

    #[test]
    fn adjacent_dual_phase_state_lands_on_half_step() {
        let mut disk = Disk2::new();
        disk.current_qtr_track = 92;
        disk.current_track = 23;

        disk.write_io(0xC0E3, 0); // phase 1 on
        disk.write_io(0xC0E5, 0); // phase 2 on

        assert_eq!(disk.current_qtr_track, 91);
        assert_eq!(disk.current_track, 22);
    }

    #[test]
    fn dropping_to_single_phase_returns_to_even_quarter_track() {
        let mut disk = Disk2::new();
        disk.current_qtr_track = 91;
        disk.current_track = 22;
        disk.phases[1] = true;
        disk.phases[2] = true;

        disk.write_io(0xC0E2, 0); // phase 1 off, leave phase 2 on

        assert_eq!(disk.current_qtr_track, 92);
        assert_eq!(disk.current_track, 23);
    }

    #[test]
    fn read_sequencer_advances_one_bit_every_four_cycles() {
        let mut disk = Disk2::new();
        disk.is_disk_loaded = true;
        disk.current_track = 0;
        disk.tracks[0].length = 1;
        disk.tracks[0].raw_bytes[0] = 0xD5;

        disk.write_io(0xC0E9, 0); // Motor on
        disk.write_io(0xC0EC, 0); // Q6 = 0
        disk.write_io(0xC0EE, 0); // Q7 = 0 (read mode)

        disk.tick(4);
        assert_eq!(disk.read_bit_phase, 1);
        assert_eq!(disk.read_shift_register, 0x01);

        disk.tick(28);
        assert_eq!(disk.read_bit_phase, 0);
        assert_eq!(disk.byte_index, 0);
        assert_eq!(disk.data_latch, 0xD5);
    }

    #[test]
    fn read_sequencer_keeps_bit_level_state_but_publishes_on_byte_boundary() {
        let mut disk = Disk2::new();
        disk.is_disk_loaded = true;
        disk.current_track = 0;
        disk.tracks[0].length = 1;
        disk.tracks[0].raw_bytes[0] = 0xFF;

        disk.write_io(0xC0E9, 0); // Motor on
        disk.write_io(0xC0EC, 0); // Q6 = 0
        disk.write_io(0xC0EE, 0); // Q7 = 0 (read mode)

        disk.tick(32);
        assert_eq!(disk.read_io(0xC0EC), 0xFF);
        assert_eq!(disk.data_latch, 0x7F);

        disk.tick(4);
        assert_eq!(disk.read_bit_phase, 1);
        assert_eq!(disk.data_latch, 0x7F);

        disk.tick(28);
        assert_eq!(disk.data_latch, 0xFF);
    }

    #[test]
    fn deferred_read_latch_update_waits_one_byte_boundary_before_publish() {
        let mut disk = Disk2::new();
        disk.is_disk_loaded = true;
        disk.current_track = 0;
        disk.tracks[0].length = 2;
        disk.tracks[0].raw_bytes[0] = 0xD5;
        disk.tracks[0].raw_bytes[1] = 0xAA;
        disk.set_defer_read_latch_update(true);

        disk.write_io(0xC0E9, 0); // Motor on
        disk.write_io(0xC0EC, 0); // Q6 = 0
        disk.write_io(0xC0EE, 0); // Q7 = 0 (read mode)

        disk.tick(32);
        assert_eq!(disk.data_latch, 0x00);
        assert_eq!(disk.pending_read_latch, Some(0xD5));

        disk.tick(32);
        assert_eq!(disk.data_latch, 0xD5);
        assert_eq!(disk.pending_read_latch, Some(0xAA));
    }

    #[test]
    fn bitstream_read_mode_can_skip_a_source_bit_when_effective_length_is_shorter() {
        let mut disk = Disk2::new();
        disk.is_disk_loaded = true;
        disk.current_track = 0;
        disk.tracks[0].length = 2;
        disk.tracks[0].read_length = 1;
        disk.tracks[0].raw_bytes[0] = 0b1010_1010;
        disk.tracks[0].raw_bytes[1] = 0b0101_0101;
        disk.set_bitstream_read_mode(true);

        disk.write_io(0xC0E9, 0); // Motor on
        disk.write_io(0xC0EC, 0); // Q6 = 0
        disk.write_io(0xC0EE, 0); // Q7 = 0 (read mode)

        disk.tick(4);
        assert_eq!(disk.read_shift_register, 0b0000_0001);

        disk.tick(4);
        assert_eq!(disk.read_shift_register, 0b0000_0011);
        assert_eq!(disk.byte_index, 0);

        disk.tick(24);
        assert_eq!(disk.data_latch, 0xF0);
    }

    #[test]
    fn read_only_geometry_can_stretch_rotation_without_changing_track_length() {
        let mut disk = Disk2::new();
        disk.is_disk_loaded = true;
        disk.current_track = 0;
        disk.tracks[0].length = 2;
        disk.tracks[0].read_length = 4;
        disk.tracks[0].raw_bytes[0] = 0xD5;
        disk.tracks[0].raw_bytes[1] = 0xAA;

        disk.write_io(0xC0E9, 0); // Motor on
        disk.write_io(0xC0EC, 0); // Q6 = 0
        disk.write_io(0xC0EE, 0); // Q7 = 0 (read mode)

        disk.tick(32);
        assert_eq!(disk.byte_index, 0);

        disk.tick(32);
        assert_eq!(disk.byte_index, 1);
    }

    #[test]
    fn seek_to_new_track_resets_read_rotation_phase() {
        let mut disk = Disk2::new();
        disk.is_disk_loaded = true;
        disk.current_qtr_track = 3;
        disk.current_track = 0;
        disk.phases[1] = true;
        disk.phases[2] = true;
        disk.byte_index = 1;
        disk.read_bit_phase = 3;
        disk.read_shift_register = 0x5A;
        disk.read_rotation_accumulator = 2;
        disk.write_bit_phase = 4;

        disk.write_io(0xC0E2, 0); // phase 1 off, leave phase 2 on -> quarter track 4

        assert_eq!(disk.current_track, 1);
        assert_eq!(disk.byte_index, 0);
        assert_eq!(disk.read_bit_phase, 0);
        assert_eq!(disk.read_shift_register, 0);
        assert_eq!(disk.read_rotation_accumulator, 0);
        assert_eq!(disk.write_bit_phase, 0);
    }
}
