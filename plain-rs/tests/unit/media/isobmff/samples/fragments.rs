use super::*;

fn fragmented(explicit_sizes: bool, base_is_moof: bool) -> Vec<u8> {
    let mut prefix = ftyp();
    let trex = atom(b"trex", &words(&[0, 1, 1, 1000, 2, 0x10000]));
    prefix.extend(atom(
        b"moov",
        &[track(&empty_tables(), 0), atom(b"mvex", &trex)].concat(),
    ));
    for number in 0..2 {
        let start = prefix.len() as u64;
        let tfhd = if base_is_moof {
            atom(b"tfhd", &words(&[0x020000, 1]))
        } else {
            atom(
                b"tfhd",
                &[words(&[1, 1]), start.to_be_bytes().to_vec()].concat(),
            )
        };
        let mut trun = words(&[if explicit_sizes { 0x205 } else { 5 }, 2, 0, 0]);
        if explicit_sizes {
            trun.extend(words(&[2, 2]));
        }
        let make = |trun: &[u8]| {
            atom(
                b"moof",
                &atom(
                    b"traf",
                    &[
                        tfhd.clone(),
                        atom(b"tfdt", &words(&[0, number * 2000])),
                        atom(b"trun", trun),
                    ]
                    .concat(),
                ),
            )
        };
        let moof_size = make(&trun).len();
        trun[8..12].copy_from_slice(&((moof_size + 8) as u32).to_be_bytes());
        prefix.extend(make(&trun));
        prefix.extend(atom(b"mdat", &[number as u8, 1, number as u8, 2]));
    }
    prefix
}

#[test]
fn fragments_apply_defaults_signed_offsets_times_and_sync_flags() {
    for explicit in [false, true] {
        for relative in [false, true] {
            let mut file = Cursor::new(fragmented(explicit, relative));
            let track = AvcTrack::read(&mut file).unwrap();
            assert_eq!((track.sample_count, track.duration_ticks), (4, 4000));
            assert_eq!(track.keyframe_at(0), 1);
            assert_eq!(track.keyframe_at(1900), 1);
            assert_eq!(track.keyframe_at(2500), 3);
            for id in 1..=4 {
                assert_eq!(
                    track.read_sample(&mut file, id, 2).unwrap(),
                    Some(vec![((id - 1) / 2) as u8, ((id - 1) % 2 + 1) as u8])
                );
            }
        }
    }
}

#[test]
fn malformed_fragment_counts_offsets_track_ids_and_sizes_are_rejected() {
    for (kind, offset, value) in [
        (b"trun", 4, u32::MAX),
        (b"tfhd", 4, 99),
        (b"trun", 8, i32::MIN as u32),
        (b"tfhd", 0, 0x020002),
    ] {
        let mut bytes = fragmented(true, true);
        patch_word(&mut bytes, kind, offset, value);
        assert!(AvcTrack::read(&mut Cursor::new(bytes)).is_err());
    }
    let mut bytes = fragmented(true, true);
    patch_word(&mut bytes, b"trun", 16, u32::MAX);
    let mut file = Cursor::new(bytes);
    let track = AvcTrack::read(&mut file).unwrap();
    assert!(track.read_sample(&mut file, 1, usize::MAX).is_err());
}

#[test]
fn multiple_runs_inherit_the_previous_traf_end_and_support_version_one_decode_time() {
    let mut audio = track(&empty_tables(), 0);
    patch_word(&mut audio, b"tkhd", 12, 2);
    let handler = audio.windows(4).position(|w| w == b"vide").unwrap();
    audio[handler..handler + 4].copy_from_slice(b"soun");
    let defaults = atom(
        b"mvex",
        &[
            atom(b"trex", &words(&[0, 1, 1, 1000, 2, 0])),
            atom(b"trex", &words(&[0, 2, 1, 1000, 1, 0])),
        ]
        .concat(),
    );
    let mut prefix = [
        ftyp(),
        atom(
            b"moov",
            &[track(&empty_tables(), 0), audio, defaults].concat(),
        ),
    ]
    .concat();
    let decode_time = u64::from(u32::MAX) + 100;
    let video = atom(
        b"traf",
        &[
            atom(b"tfhd", &words(&[0, 1])),
            atom(
                b"tfdt",
                &[words(&[0x01000000]), decode_time.to_be_bytes().to_vec()].concat(),
            ),
            atom(b"trun", &words(&[0, 1])),
            atom(b"trun", &words(&[0, 1])),
        ]
        .concat(),
    );
    let make = |offset| {
        atom(
            b"moof",
            &[
                atom(
                    b"traf",
                    &[
                        atom(b"tfhd", &words(&[0x020000, 2])),
                        atom(b"trun", &words(&[1, 2, offset])),
                    ]
                    .concat(),
                ),
                video.clone(),
            ]
            .concat(),
        )
    };
    let moof = make((make(0).len() + 8) as u32);
    prefix.extend(moof);
    prefix.extend(atom(b"mdat", &[1, 2, 3, 4, 5, 6]));
    let mut file = Cursor::new(prefix);
    let track = AvcTrack::read(&mut file).unwrap();
    assert_eq!(
        (track.sample_count, track.duration_ticks),
        (2, decode_time + 2000)
    );
    assert_eq!(
        track.read_sample(&mut file, 1, 2).unwrap(),
        Some(vec![3, 4])
    );
    assert_eq!(
        track.read_sample(&mut file, 2, 2).unwrap(),
        Some(vec![5, 6])
    );
    assert_eq!(track.keyframe_at(decode_time + 1000), 2);
}

#[test]
fn signed_data_offsets_can_reference_media_before_the_fragment() {
    let mut bytes = fragmented(true, false);
    let first_moof = bytes.windows(4).position(|w| w == b"moof").unwrap() - 4;
    let length = u32::from_be_bytes(bytes[first_moof..first_moof + 4].try_into().unwrap()) as usize;
    let mut moof = bytes[first_moof..first_moof + length].to_vec();
    let media = atom(b"mdat", &[0, 1, 0, 2]);
    let new_start = first_moof as u64 + media.len() as u64;
    let tfhd = moof.windows(4).position(|w| w == b"tfhd").unwrap() + 4;
    moof[tfhd + 8..tfhd + 16].copy_from_slice(&new_start.to_be_bytes());
    patch_word(&mut moof, b"trun", 8, (8i32 - media.len() as i32) as u32);
    bytes.truncate(first_moof);
    bytes.extend(media);
    bytes.extend(moof);
    let mut file = Cursor::new(bytes);
    let track = AvcTrack::read(&mut file).unwrap();
    assert_eq!(
        track.read_sample(&mut file, 1, 2).unwrap(),
        Some(vec![0, 1])
    );
    assert_eq!(
        track.read_sample(&mut file, 2, 2).unwrap(),
        Some(vec![0, 2])
    );
}
