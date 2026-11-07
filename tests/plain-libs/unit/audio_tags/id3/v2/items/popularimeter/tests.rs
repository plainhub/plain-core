use crate::audio_tags::id3::v2::items::popularimeter::PopularimeterFrame;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader};

fn test_popm(popm: &PopularimeterFrame<'_>) {
    let email = popm.email.clone();
    let rating = popm.rating;
    let counter = popm.counter;

    let popm_bytes = popm.as_bytes().unwrap();
    assert_eq!(&popm_bytes[..email.len()], email.as_bytes());
    assert_eq!(popm_bytes[email.len()], 0);
    assert_eq!(popm_bytes[email.len() + 1], rating);

    let counter_len = if u32::try_from(counter).is_ok() {
        4
    } else {
        let counter_bytes = counter.to_be_bytes();
        let i = counter_bytes.iter().position(|b| *b != 0).unwrap_or(4);
        counter_bytes.len() - i
    };

    assert_eq!(popm_bytes[email.len() + 2..].len(), counter_len);
}

#[test]
fn write_popm() {
    let popm_u32_boundary = PopularimeterFrame {
        header: FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        email: String::from("unit\x40x.test"),
        rating: 255,
        counter: u64::from(u32::MAX),
    };

    let popm_u40 = PopularimeterFrame {
        header: FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        email: String::from("unit\x40x.test"),
        rating: 196,
        counter: u64::from(u32::MAX) + 1,
    };

    test_popm(&popm_u32_boundary);
    test_popm(&popm_u40);
}
