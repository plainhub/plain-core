use crate::audio_tags::config::ParsingMode;
use crate::audio_tags::id3::v2::{
    ChannelInformation, ChannelType, FrameFlags, FrameHeader, RelativeVolumeAdjustmentFrame,
};

use std::collections::HashMap;
use std::io::Read;

fn expected() -> RelativeVolumeAdjustmentFrame<'static> {
    let mut channels = HashMap::new();

    channels.insert(
        ChannelType::MasterVolume,
        ChannelInformation {
            channel_type: ChannelType::MasterVolume,
            volume_adjustment: 15,
            bits_representing_peak: 4,
            peak_volume: Some(vec![4]),
        },
    );

    channels.insert(
        ChannelType::FrontLeft,
        ChannelInformation {
            channel_type: ChannelType::FrontLeft,
            volume_adjustment: 21,
            bits_representing_peak: 0,
            peak_volume: None,
        },
    );

    channels.insert(
        ChannelType::Subwoofer,
        ChannelInformation {
            channel_type: ChannelType::Subwoofer,
            volume_adjustment: 30,
            bits_representing_peak: 11,
            peak_volume: Some(vec![0xFF, 0x07]),
        },
    );

    RelativeVolumeAdjustmentFrame {
        header: FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        identification: String::from("Surround sound"),
        channels,
    }
}

#[test]
fn rva2_decode() {
    let cont = crate::audio_tags::tag::utils::read_path("tests/tags/assets/id3v2/test.rva2");

    let parsed_rva2 = RelativeVolumeAdjustmentFrame::parse(
        &mut &cont[..],
        FrameFlags::default(),
        ParsingMode::Strict,
    )
    .unwrap()
    .unwrap();

    assert_eq!(parsed_rva2, expected());
}

#[test]

fn rva2_encode() {
    let encoded = expected().as_bytes();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_path("tests/tags/assets/id3v2/test.rva2");

    // We have to check the output in fragments, as the order of channels is not guaranteed.
    assert_eq!(encoded.len(), expected_bytes.len());

    let mut needles = vec![
        &[1, 0, 15, 4, 4][..],       // Master volume configuration
        &[8, 0, 30, 11, 255, 7][..], // Front left configuration
        &[3, 0, 21, 0][..],          // Subwoofer configuration
    ];

    let encoded_reader = &mut &encoded[..];

    let mut ident = [0; 15];
    encoded_reader.read_exact(&mut ident).unwrap();
    assert_eq!(ident, b"Surround sound\0"[..]);

    loop {
        if needles.is_empty() {
            break;
        }

        let mut remove_idx = None;
        for (idx, needle) in needles.iter().enumerate() {
            if encoded_reader.starts_with(needle) {
                std::io::copy(
                    &mut encoded_reader.take(needle.len() as u64),
                    &mut std::io::sink(),
                )
                .unwrap();

                remove_idx = Some(idx);
                break;
            }
        }

        let Some(remove_idx) = remove_idx else {
            unreachable!("Unexpected data in RVA2 frame: {:?}", &encoded);
        };

        needles.remove(remove_idx);
    }

    assert!(needles.is_empty());
}
