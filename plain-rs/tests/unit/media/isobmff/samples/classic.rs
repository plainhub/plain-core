use super::*;

#[test]
fn fixed_variable_sizes_and_both_chunk_offsets_return_the_same_samples() {
    for uniform in [false, true] {
        for co64 in [false, true] {
            let mut file = Cursor::new(classic_file(uniform, co64));
            let track = AvcTrack::read(&mut file).unwrap();
            assert_eq!(
                (
                    track.width,
                    track.height,
                    track.timescale,
                    track.duration_ticks,
                    track.sample_count
                ),
                (320, 240, 1000, 4000, 4)
            );
            assert_eq!(track.parameter_sets, [vec![0x67, 1], vec![0x68, 2]]);
            for id in 1..=4 {
                assert_eq!(
                    track.read_sample(&mut file, id, 2).unwrap().unwrap(),
                    vec![(id * 2 - 1) as u8, (id * 2) as u8]
                );
            }
            assert_eq!(track.read_sample(&mut file, 0, 2).unwrap(), None);
            assert_eq!(track.read_sample(&mut file, 5, 2).unwrap(), None);
            assert_eq!(track.keyframe_at(2500), 3);
            assert_eq!(track.keyframe_at(u64::MAX), 3);
        }
    }
}

#[test]
fn table_counts_runs_offsets_descriptions_and_timing_are_validated() {
    for (kind, offset, value) in [
        (b"stsz", 8, u32::MAX),
        (b"stts", 4, u32::MAX),
        (b"stsc", 4, u32::MAX),
        (b"stsc", 8, 0),
        (b"stsc", 12, 0),
        (b"stts", 8, 3),
        (b"stss", 8, 0),
        (b"stss", 12, 5),
        (b"stco", 4, 3),
        (b"mdhd", 12, 0),
    ] {
        let mut bytes = classic_file(false, false);
        patch_word(&mut bytes, kind, offset, value);
        assert!(
            AvcTrack::read(&mut Cursor::new(bytes)).is_err(),
            "{kind:?} {offset} {value}"
        );
    }
    for (kind, offset, value) in [(b"stco", 8, 0), (b"stco", 8, u32::MAX), (b"stsc", 16, 2)] {
        let mut bytes = classic_file(false, false);
        patch_word(&mut bytes, kind, offset, value);
        let mut file = Cursor::new(bytes);
        let track = AvcTrack::read(&mut file).unwrap();
        assert!(track.read_sample(&mut file, 1, 10).is_err());
    }
}

#[test]
fn sample_budget_is_checked_before_seeking_or_reading_payload() {
    let mut file = Cursor::new(classic_file(false, false));
    let track = AvcTrack::read(&mut file).unwrap();
    let position = file.position();
    assert_eq!(track.read_sample(&mut file, 1, 1).unwrap(), None);
    assert_eq!(file.position(), position);
    assert_eq!(
        track.read_sample(&mut file, 1, 2).unwrap(),
        Some(vec![1, 2])
    );
}

#[test]
fn truncated_boxes_and_parameter_sets_fail_without_panicking() {
    let full = classic_file(false, false);
    for end in 0..full.len() {
        assert!(
            AvcTrack::read(&mut Cursor::new(&full[..end])).is_err(),
            "end={end}"
        );
    }
    let mut bytes = full.clone();
    patch_word(&mut bytes, b"moov", 0, u32::MAX);
    assert!(AvcTrack::read(&mut Cursor::new(bytes)).is_err());
    let mut bytes = full;
    let avcc = bytes.windows(4).position(|w| w == b"avcC").unwrap() + 4;
    bytes[avcc + 6..avcc + 8].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(AvcTrack::read(&mut Cursor::new(bytes)).is_err());
}

#[test]
fn extended_movie_headers_and_zero_size_terminal_media_are_supported() {
    let bytes = classic_file(false, false);
    let at = bytes.windows(4).position(|w| w == b"moov").unwrap() - 4;
    let movie = bytes[at..].to_vec();
    let extended = [
        bytes[..at].to_vec(),
        words(&[1]),
        b"moov".to_vec(),
        ((movie.len() + 8) as u64).to_be_bytes().to_vec(),
        movie[8..].to_vec(),
    ]
    .concat();
    assert!(AvcTrack::read(&mut Cursor::new(extended)).is_ok());
    let mut movie = movie;
    let offset = ftyp().len() + movie.len() + 8;
    patch_word(&mut movie, b"stco", 8, offset as u32);
    patch_word(&mut movie, b"stco", 12, offset as u32 + 4);
    let mut terminal = atom(b"mdat", &[1, 2, 3, 4, 5, 6, 7, 8]);
    terminal[..4].fill(0);
    let mut file = Cursor::new([ftyp(), movie, terminal].concat());
    let track = AvcTrack::read(&mut file).unwrap();
    assert_eq!(
        track.read_sample(&mut file, 4, 2).unwrap(),
        Some(vec![7, 8])
    );
}

#[test]
fn sample_io_errors_are_propagated() {
    struct Reader {
        data: Cursor<Vec<u8>>,
        fail: bool,
    }
    impl Read for Reader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.fail {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            self.data.read(buffer)
        }
    }
    impl Seek for Reader {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.data.seek(position)
        }
    }
    let mut file = Reader {
        data: Cursor::new(classic_file(false, false)),
        fail: false,
    };
    let track = AvcTrack::read(&mut file).unwrap();
    file.fail = true;
    assert_eq!(
        track.read_sample(&mut file, 1, 2).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
fn changing_samples_per_chunk_preserves_each_sample_offset() {
    let prefix = ftyp();
    let start = prefix.len() as u32 + 8;
    let stbl = [
        description(),
        atom(b"stsz", &words(&[0, 0, 4, 2, 2, 2, 2])),
        table_box(b"stsc", 2, &[1, 2, 1, 2, 1, 1]),
        table_box(b"stco", 3, &[start, start + 4, start + 6]),
        table_box(b"stts", 1, &[4, 1000]),
    ]
    .concat();
    let mut file = Cursor::new(
        [
            prefix,
            atom(b"mdat", &[1, 2, 3, 4, 5, 6, 7, 8]),
            atom(b"moov", &track(&stbl, 4000)),
        ]
        .concat(),
    );
    let track = AvcTrack::read(&mut file).unwrap();
    for id in 1..=4 {
        assert_eq!(
            track.read_sample(&mut file, id, 2).unwrap(),
            Some(vec![(2 * id - 1) as u8, (2 * id) as u8])
        );
        assert_eq!(track.keyframe_at(u64::from(id - 1) * 1000), id);
    }
}

use super::super::timing::locate_keyframe_sample;
/// Hand-built sample tables: 12 samples, stts runs + stss syncs.
#[test]
fn keyframe_location_matrix() {
    // Uniform 1000-tick deltas, syncs at {1, 5, 9}.
    let stts = [(6u64, 1000u64), (6, 1000)];
    let syncs = [1u32, 5, 9];
    // t=0 → sample 1 (sync).
    assert_eq!(locate_keyframe_sample(&stts, Some(&syncs), 0), 1);
    // t=1.5s → time sample 2, previous sync = 1.
    assert_eq!(locate_keyframe_sample(&stts, Some(&syncs), 1_500), 1);
    // t=4.9s → time sample 5 (itself sync) → 5.
    assert_eq!(locate_keyframe_sample(&stts, Some(&syncs), 4_900), 5);
    // t=7s → time sample 8, previous sync = 5.
    assert_eq!(locate_keyframe_sample(&stts, Some(&syncs), 7_000), 5);
    // t=9.999s → sample 10, sync 9.
    assert_eq!(locate_keyframe_sample(&stts, Some(&syncs), 9_999), 9);
    // Beyond the end → walk clamps past the last sample → sync 9.
    assert_eq!(locate_keyframe_sample(&stts, Some(&syncs), 999_999), 9);

    // No stss → every sample is sync: plain timestamp walk.
    assert_eq!(locate_keyframe_sample(&stts, None, 7_000), 8);
    assert_eq!(locate_keyframe_sample(&stts, None, 0), 1);

    // Variable-delta runs: 2 samples @500 then 10 @1250.
    let var = [(2u64, 500u64), (10, 1250)];
    // t=600 → inside the first run → sample 2.
    assert_eq!(locate_keyframe_sample(&var, None, 600), 2);
    // t=1300: first run covers ticks [0,1000) samples 1-2; second run
    // starts at sample 3 tick 1000, delta 1250 → (1300-1000)/1250=0 → 3.
    assert_eq!(locate_keyframe_sample(&var, None, 1_300), 3);
    // t=10000 → (10000-1000)/1250 = 7 → sample 10.
    assert_eq!(locate_keyframe_sample(&var, None, 10_000), 10);

    // Syncs not starting at 1 (open-GOP-ish): target before first sync.
    let late_syncs = [5u32, 9];
    assert_eq!(locate_keyframe_sample(&stts, Some(&late_syncs), 900), 5);
}
