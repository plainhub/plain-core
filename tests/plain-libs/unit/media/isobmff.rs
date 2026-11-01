use super::*;

fn atom(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut result = ((data.len() + 8) as u32).to_be_bytes().to_vec();
    result.extend(kind);
    result.extend(data);
    result
}
fn probe(data: &[u8]) -> Option<i64> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.mp4");
    std::fs::write(&path, data).unwrap();
    mp4_milliseconds(&path)
}
fn time(scale: u32, ticks: u32) -> Vec<u8> {
    let mut data = vec![0; 12];
    data.extend(scale.to_be_bytes());
    data.extend(ticks.to_be_bytes());
    data
}
fn movie_track(handler: &[u8; 4], id: u32, scale: u32, default: u32) -> Vec<u8> {
    let mut tkhd = vec![0; 12];
    tkhd.extend(id.to_be_bytes());
    let mut hdlr = vec![0; 8];
    hdlr.extend(handler);
    let mdia = atom(
        b"mdia",
        &[atom(b"mdhd", &time(scale, 0)), atom(b"hdlr", &hdlr)].concat(),
    );
    let trak = atom(b"trak", &[atom(b"tkhd", &tkhd), mdia].concat());
    let mut trex = vec![0; 4];
    trex.extend(id.to_be_bytes());
    trex.extend(1u32.to_be_bytes());
    trex.extend(default.to_be_bytes());
    atom(
        b"moov",
        &[trak, atom(b"mvex", &atom(b"trex", &trex))].concat(),
    )
}
fn fragment(
    id: u32,
    default: Option<u32>,
    samples: &[u32],
    optional_header_fields: bool,
) -> Vec<u8> {
    let flags = u32::from(default.is_some()) * 8 + if optional_header_fields { 3 } else { 0 };
    let mut tfhd = flags.to_be_bytes().to_vec();
    tfhd.extend(id.to_be_bytes());
    if optional_header_fields {
        tfhd.extend(9u64.to_be_bytes());
        tfhd.extend(1u32.to_be_bytes());
    }
    if let Some(default) = default {
        tfhd.extend(default.to_be_bytes());
    }
    let mut trun = 0x000f05u32.to_be_bytes().to_vec();
    trun.extend((samples.len() as u32).to_be_bytes());
    trun.extend(0u32.to_be_bytes());
    trun.extend(0u32.to_be_bytes());
    for duration in samples {
        trun.extend(duration.to_be_bytes());
        trun.extend(17u32.to_be_bytes());
        trun.extend(0u32.to_be_bytes());
        trun.extend(0u32.to_be_bytes());
    }
    atom(
        b"moof",
        &atom(
            b"traf",
            &[atom(b"tfhd", &tfhd), atom(b"trun", &trun)].concat(),
        ),
    )
}

#[test]
fn normal_movie_preserves_milliseconds_and_handles_movie_at_the_tail() {
    let data = [
        atom(b"mdat", &[0; 16]),
        atom(b"moov", &atom(b"mvhd", &time(1000, 3211))),
    ]
    .concat();
    assert_eq!(probe(&data), Some(3211));
    let mut extended = 1u32.to_be_bytes().to_vec();
    extended.extend(b"moov");
    let payload = atom(b"mvhd", &time(1000, 4321));
    extended.extend(((payload.len() + 16) as u64).to_be_bytes());
    extended.extend(payload);
    assert_eq!(probe(&extended), Some(4321));
}

#[test]
fn version_one_movie_keeps_values_beyond_32_bits() {
    let mut data = vec![1, 0, 0, 0];
    data.extend([0; 16]);
    data.extend(1000u32.to_be_bytes());
    data.extend(5_000_000_001u64.to_be_bytes());
    assert_eq!(
        probe(&atom(b"moov", &atom(b"mvhd", &data))),
        Some(5_000_000_001)
    );
}

#[test]
fn fragments_match_the_video_track_and_skip_optional_fields_correctly() {
    let data = [
        movie_track(b"vide", 7, 1000, 0),
        fragment(9, None, &[9000], false),
        fragment(7, Some(3), &[100, 200, 300], true),
        fragment(7, None, &[50], false),
    ]
    .concat();
    assert_eq!(probe(&data), Some(650));
}

#[test]
fn audio_fragments_use_inherited_default_sample_duration_without_a_sample_array() {
    let mut tfhd = 0u32.to_be_bytes().to_vec();
    tfhd.extend(4u32.to_be_bytes());
    let mut trun = vec![0; 4];
    trun.extend(100u32.to_be_bytes());
    let fragment = atom(
        b"moof",
        &atom(
            b"traf",
            &[atom(b"tfhd", &tfhd), atom(b"trun", &trun)].concat(),
        ),
    );
    assert_eq!(
        probe(&[movie_track(b"soun", 4, 48000, 1024), fragment].concat()),
        Some(2133)
    );
}

#[test]
fn malformed_and_truncated_input_never_produces_a_duration() {
    assert_eq!(probe(b"tiny"), None);
    assert_eq!(probe(&atom(b"moov", &atom(b"mvhd", &time(0, 99)))), None);
    assert_eq!(probe(&[0, 0, 0, 4, b'm', b'o', b'o', b'v']), None);
    let valid = [
        movie_track(b"vide", 7, 1000, 0),
        fragment(7, None, &[100, 200], false),
    ]
    .concat();
    for length in 0..valid.len() {
        assert_eq!(probe(&valid[..length]), None, "length={length}");
    }
    let mut invalid = vec![0; 4];
    invalid.extend(u32::MAX.to_be_bytes());
    assert_eq!(
        run_ticks(&invalid, u32::MAX),
        Some(u64::from(u32::MAX) * u64::from(u32::MAX))
    );
    invalid[0..4].copy_from_slice(&0x100u32.to_be_bytes());
    assert_eq!(run_ticks(&invalid, 0), None);
}

fn codec_track(codec: Option<&[u8; 4]>, handler: &[u8; 4]) -> Vec<u8> {
    let mut hdlr = vec![0; 8];
    hdlr.extend(handler);
    let mut media = atom(b"hdlr", &hdlr);
    if let Some(codec) = codec {
        let mut description = vec![0, 0, 0, 0, 0, 0, 0, 1];
        description.extend(atom(codec, &[0; 20]));
        media.extend(atom(b"minf", &atom(b"stbl", &atom(b"stsd", &description))));
    }
    atom(b"trak", &atom(b"mdia", &media))
}
#[test]
fn track_probe_reads_video_codecs_and_metadata_tracks_without_confusing_audio() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("codec.mp4");
    for codec in [*b"avc1", *b"hvc1", *b"hev1"] {
        for metadata in [false, true] {
            let mut data = codec_track(Some(b"mp4a"), b"soun");
            data.extend(codec_track(Some(&codec), b"vide"));
            if metadata {
                data.extend(codec_track(Some(b"mebx"), b"meta"));
            }
            std::fs::write(
                &path,
                [
                    atom(b"ftyp", &[0; 8]),
                    atom(b"mdat", &[0; 16]),
                    atom(b"moov", &data),
                ]
                .concat(),
            )
            .unwrap();
            assert_eq!(
                tracks(&path),
                Some(Tracks {
                    first_video_codec: Some(String::from_utf8(codec.to_vec()).unwrap()),
                    needs_remux: metadata
                })
            );
        }
    }
    std::fs::write(&path, atom(b"moov", &codec_track(Some(b"mp4a"), b"soun"))).unwrap();
    assert_eq!(tracks(&path).unwrap().first_video_codec, None);
    std::fs::write(&path, atom(b"moov", &codec_track(None, b"vide"))).unwrap();
    assert_eq!(tracks(&path).unwrap().first_video_codec, None);
    std::fs::write(&path, b"truncated").unwrap();
    assert_eq!(tracks(&path), None);
    assert_eq!(tracks(&dir.path().join("missing")), None);
}

#[test]
fn descriptor_asset_duration_reads_only_the_declared_region() {
    let prefix = atom(b"moov", &atom(b"mvhd", &time(1000, 9999)));
    let movie = atom(b"moov", &atom(b"mvhd", &time(1000, 3211)));
    let suffix = atom(b"moov", &atom(b"mvhd", &time(1000, 8888)));
    let mut file = tempfile::tempfile().unwrap();
    use std::io::Write;
    file.write_all(&[prefix.clone(), movie.clone(), suffix].concat())
        .unwrap();
    assert_eq!(
        mp4_file_milliseconds(&mut file, prefix.len() as u64, movie.len() as u64),
        Some(3211)
    );
    assert_eq!(
        mp4_file_milliseconds(&mut file, prefix.len() as u64, movie.len() as u64 - 1),
        None
    );
    assert_eq!(
        mp4_file_milliseconds(&mut file, prefix.len() as u64, 0),
        None
    );
    assert_eq!(mp4_file_milliseconds(&mut file, u64::MAX, 1), None);
}

#[test]
fn fragmented_descriptor_asset_restarts_at_the_declared_offset() {
    let prefix = b"outside the asset";
    let movie = [
        movie_track(b"vide", 1, 1000, 0),
        fragment(1, Some(1000), &[1234, 2345], true),
    ]
    .concat();
    let mut file = tempfile::tempfile().unwrap();
    use std::io::Write;
    file.write_all(&[prefix.as_slice(), movie.as_slice()].concat())
        .unwrap();
    assert_eq!(
        mp4_file_milliseconds(&mut file, prefix.len() as u64, movie.len() as u64),
        Some(3579)
    );
}

#[test]
fn extended_header_never_reads_past_the_asset_boundary() {
    use std::io::Write;
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(
        &[
            1u32.to_be_bytes().as_slice(),
            b"moov",
            &24u64.to_be_bytes(),
            &[0; 8],
        ]
        .concat(),
    )
    .unwrap();
    assert_eq!(mp4_file_milliseconds(&mut file, 0, 8), None);
    assert_eq!(file.stream_position().unwrap(), 8);
}
