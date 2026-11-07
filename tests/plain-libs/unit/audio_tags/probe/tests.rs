use crate::audio_tags::config::{GlobalOptions, ParseOptions};
use crate::audio_tags::file::FileType;
use crate::audio_tags::probe::Probe;

use std::fs::File;

#[test]
fn mp3_id3v2_trailing_junk() {
    // test data that contains 4 bytes of junk (0x20) between the ID3 portion and the first MP3 frame
    let data: [&[u8]; 4] = [
        // ID3v2.3 header (10 bytes)
        &[0x49, 0x44, 0x33, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23],
        // TALB frame
        &[
            0x54, 0x41, 0x4C, 0x42, 0x00, 0x00, 0x00, 0x19, 0x00, 0x00, 0x01, 0xFF, 0xFE, 0x61,
            0x00, 0x61, 0x00, 0x61, 0x00, 0x61, 0x00, 0x61, 0x00, 0x61, 0x00, 0x61, 0x00, 0x61,
            0x00, 0x61, 0x00, 0x61, 0x00, 0x61, 0x00,
        ],
        // 4 bytes of junk
        &[0x20, 0x20, 0x20, 0x20],
        // start of MP3 frame (not all bytes are shown in this slice)
        &[
            0xFF, 0xFB, 0x50, 0xC4, 0x00, 0x03, 0xC0, 0x00, 0x01, 0xA4, 0x00, 0x00, 0x00, 0x20,
            0x00, 0x00, 0x34, 0x80, 0x00, 0x00, 0x04,
        ],
    ];
    let data: Vec<u8> = data.into_iter().flatten().copied().collect();
    let data = std::io::Cursor::new(&data);
    let probe = Probe::new(data).guess_file_type().unwrap();
    assert_eq!(probe.file_type(), Some(FileType::Mpeg));
}

#[test]
fn parse_options_allocation_limit() {
    // In this test, we read a partial MP3 file that has an ID3v2 tag containing a frame outside
    // of the allocation limit. We'll be testing with an encrypted frame, since we immediately read those into memory.

    use crate::audio_tags::id3::v2::util::synchsafe::SynchsafeInteger;

    fn create_encrypted_frame(size: usize) -> Vec<u8> {
        // Encryption method (1 byte) + encryption method data length indicator (4 bytes)
        // This is required and goes before the data.
        let flag_data = vec![0; 5];

        let bytes = vec![0; size];

        let frame_length_synch = ((bytes.len() + flag_data.len()) as u32)
            .synch()
            .unwrap()
            .to_be_bytes();
        let frame_header = vec![
            b'S',
            b'M',
            b'T',
            b'H',
            frame_length_synch[0],
            frame_length_synch[1],
            frame_length_synch[2],
            frame_length_synch[3],
            0x00,
            0b0000_0101, // Encrypted, Has data length indicator
        ];

        [frame_header, flag_data, bytes].concat()
    }

    fn create_fake_mp3(frame_size: u32) -> Vec<u8> {
        let id3v2_tag_length = (frame_size + 5 + 10).synch().unwrap().to_be_bytes();
        [
            // ID3v2.4 header (10 bytes)
            vec![
                0x49,
                0x44,
                0x33,
                0x04,
                0x00,
                0x00,
                id3v2_tag_length[0],
                id3v2_tag_length[1],
                id3v2_tag_length[2],
                id3v2_tag_length[3],
            ],
            // Random encrypted frame
            create_encrypted_frame(frame_size as usize),
            // start of MP3 frame (not all bytes are shown in this slice)
            vec![
                0xFF, 0xFB, 0x50, 0xC4, 0x00, 0x03, 0xC0, 0x00, 0x01, 0xA4, 0x00, 0x00, 0x00, 0x20,
                0x00, 0x00, 0x34, 0x80, 0x00, 0x00, 0x04,
            ],
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<u8>>()
    }

    let parse_options = ParseOptions::new().read_properties(false);

    let mut global_options = GlobalOptions::new().allocation_limit(50);
    crate::audio_tags::config::apply_global_options(global_options);

    // An allocation with a size of 40 bytes should be ok
    let within_limits = create_fake_mp3(40);
    let probe = Probe::new(std::io::Cursor::new(&within_limits))
        .set_file_type(FileType::Mpeg)
        .options(parse_options);
    assert!(probe.read().is_ok());

    // An allocation with a size of 60 bytes should fail
    let too_big = create_fake_mp3(60);
    let probe = Probe::new(std::io::Cursor::new(&too_big))
        .set_file_type(FileType::Mpeg)
        .options(parse_options);
    assert!(probe.read().is_err());

    // Now test the default allocation limit (16MB), which should of course be ok with 60 bytes
    global_options.allocation_limit = GlobalOptions::DEFAULT_ALLOCATION_LIMIT;
    crate::audio_tags::config::apply_global_options(global_options);

    let probe = Probe::new(std::io::Cursor::new(&too_big))
        .set_file_type(FileType::Mpeg)
        .options(parse_options);
    assert!(probe.read().is_ok());
}

fn test_probe(path: &str, expected_file_type_guess: FileType) {
    test_probe_file(path, expected_file_type_guess);
    test_probe_path(path, expected_file_type_guess);
}

// Test from file contents
fn test_probe_file(path: &str, expected_file_type_guess: FileType) {
    let mut f = File::open(path).unwrap();
    let probe = Probe::new(&mut f).guess_file_type().unwrap();
    assert_eq!(probe.file_type(), Some(expected_file_type_guess));
}

// Test from file extension
fn test_probe_path(path: &str, expected_file_type_guess: FileType) {
    let probe = Probe::open(path).unwrap();
    assert_eq!(probe.file_type(), Some(expected_file_type_guess));
}

#[test]
fn probe_aac() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/untagged.aac"
        ),
        FileType::Aac,
    );
}

#[test]
fn probe_aac_with_id3v2() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.aac"
        ),
        FileType::Aac,
    );
}

#[test]
fn probe_aiff() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.aiff"
        ),
        FileType::Aiff,
    );
}

#[test]
fn probe_ape_with_id3v2() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.ape"
        ),
        FileType::Ape,
    );
}

#[test]
fn probe_flac() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.flac"
        ),
        FileType::Flac,
    );
}

#[test]
fn probe_flac_with_id3v2() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/flac_with_id3v2.flac"
        ),
        FileType::Flac,
    );
}

#[test]
fn probe_mp3_with_id3v2() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.mp3"
        ),
        FileType::Mpeg,
    );
}

#[test]
fn probe_mp3_with_lots_of_junk() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/junk.mp3"
        ),
        FileType::Mpeg,
    );
}

#[test]
fn probe_vorbis() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.ogg"
        ),
        FileType::Vorbis,
    );
}

#[test]
fn probe_opus() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.opus"
        ),
        FileType::Opus,
    );
}

#[test]
fn probe_speex() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.spx"
        ),
        FileType::Speex,
    );
}

#[test]
fn probe_mp4() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/m4a_codec_aac.m4a"
        ),
        FileType::Mp4,
    );
}

#[test]
fn probe_wav() {
    test_probe(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/wav_format_pcm.wav"
        ),
        FileType::Wav,
    );
}
