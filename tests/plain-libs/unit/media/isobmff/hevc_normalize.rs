use super::*;
use std::io::Cursor;

const SOURCE: &[u8] = include_bytes!("../../../../../plain-rs/testdata/video-h265.mp4");
const FRAGMENTED: &[u8] =
    include_bytes!("../../../../../plain-rs/testdata/video/video-h265-fragmented.mp4");

#[test]
fn normalization_preserves_every_video_sample_byte_offset_and_file_length() {
    for bytes in [SOURCE, FRAGMENTED] {
        let before = VideoTrack::read_hevc(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(&before.sample_entry, b"hev1");
        let mut writer = Cursor::new(Vec::new());
        canonicalize_hev1(
            &mut Cursor::new(bytes),
            &mut writer,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(writer.get_ref().len(), bytes.len());
        let after = VideoTrack::read_hevc(&mut writer).unwrap();
        assert_eq!(&after.sample_entry, b"hvc1");
        assert_eq!(
            (before.sample_count, before.timescale, before.duration_ticks),
            (after.sample_count, after.timescale, after.duration_ticks)
        );
        let mut visits = 0;
        before
            .visit_samples(|range| {
                visits += 1;
                assert_eq!(
                    &writer.get_ref()[range.start as usize..range.end as usize],
                    &bytes[range.start as usize..range.end as usize]
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(visits, before.sample_count);
        for id in 1..=before.sample_count {
            assert_eq!(
                before
                    .read_sample(&mut Cursor::new(bytes), id, 1 << 20)
                    .unwrap(),
                after.read_sample(&mut writer, id, 1 << 20).unwrap()
            );
        }
    }
}

#[test]
fn in_band_parameter_sets_and_invalid_lengths_are_rejected_before_retagging() {
    let track = VideoTrack::read_hevc(&mut Cursor::new(SOURCE)).unwrap();
    let mut first = None;
    track
        .visit_samples(|range| {
            if first.is_none() {
                first = Some(range);
            }
            Ok(())
        })
        .unwrap();
    let first = first.unwrap();
    for parameters in [false, true] {
        let mut bytes = SOURCE.to_vec();
        if parameters {
            bytes[first.start as usize + track.nal_length_size] = 32 << 1;
            bytes[first.start as usize + track.nal_length_size + 1] = 1;
        } else {
            bytes[first.start as usize..first.start as usize + track.nal_length_size].fill(0xff);
        }
        let mut output = Cursor::new(Vec::new());
        let error = canonicalize_hev1(
            &mut Cursor::new(&bytes),
            &mut output,
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            if parameters {
                io::ErrorKind::Unsupported
            } else {
                io::ErrorKind::InvalidData
            }
        );
        assert_eq!(
            &VideoTrack::read_hevc(&mut output).unwrap().sample_entry,
            b"hev1"
        );
        assert_eq!(output.get_ref(), &bytes);
    }
}

#[test]
fn pre_cancel_does_not_copy_or_modify_the_destination() {
    let mut output = Cursor::new(Vec::new());
    assert!(
        canonicalize_hev1(
            &mut Cursor::new(SOURCE),
            &mut output,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert!(output.get_ref().is_empty());
}
