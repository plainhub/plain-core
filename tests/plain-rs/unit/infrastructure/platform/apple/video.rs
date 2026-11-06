use super::*;
use std::fs;

const H264: &[u8] = include_bytes!("../../../../../../plain-rs/testdata/video-h264-baseline.mp4");
const HEVC: &[u8] = include_bytes!("../../../../../../plain-rs/testdata/video-h265-hvc1.mp4");
const HEV1: &[u8] = include_bytes!("../../../../../../plain-rs/testdata/video-h265.mp4");

#[test]
fn foundation_collection_copies_keep_their_objects_alive() {
    use objc2_foundation::NSArray;

    let value = NSNumber::new_u32(17);
    let array = NSArray::from_retained_slice(&[value.clone()]);
    let copied = array.to_vec();
    drop(array);
    assert_eq!(copied[0].as_u32(), 17);

    let key = NSString::from_str("value");
    let dictionary = NSDictionary::from_slices(&[&*key], &[&*value]);
    let (keys, values) = dictionary.to_vecs();
    drop(dictionary);
    drop(key);
    drop(value);
    assert_eq!(keys[0].to_string(), "value");
    assert_eq!(values[0].as_u32(), 17);
}

#[test]
fn foundation_value_copy_preserves_size_and_range_abi() {
    use objc2_foundation::{NSRange, NSValue};

    assert_eq!(
        std::mem::size_of::<NSRange>(),
        2 * std::mem::size_of::<usize>()
    );
    assert_eq!(
        std::mem::offset_of!(NSRange, length),
        std::mem::size_of::<usize>()
    );
    let value = NSValue::new(u64::MAX);
    assert_eq!(unsafe { value.get::<u64>() }, u64::MAX);
}

#[test]
fn dispatch_queue_clone_keeps_the_native_object_alive() {
    let queue = DispatchQueue::new("plain.video.test", DispatchQueueAttr::SERIAL);
    let retained = queue.clone();
    assert!(std::ptr::eq(&*queue, &*retained));
    drop(queue);
    std::thread::spawn(move || {
        let object: &AnyObject = retained.as_ref();
        assert!(!object.class().name().to_bytes().is_empty());
    })
    .join()
    .unwrap();
}

fn decoded_frames(path: &Path) -> (usize, f64, f64, i64, i32) {
    autoreleasepool(|_| unsafe {
        let asset = AVAsset::assetWithURL(&NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().unwrap(),
        )));
        let track = asset
            .tracksWithMediaType(AVMediaTypeVideo.unwrap())
            .firstObject()
            .unwrap();
        let size = track.naturalSize();
        let duration = asset.duration();
        let reader = AVAssetReader::assetReaderWithAsset_error(&asset).unwrap();
        let key = NSString::from_str(&kCVPixelBufferPixelFormatTypeKey.to_string());
        let format = NSNumber::new_u32(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange);
        let values: [&AnyObject; 1] = [&format];
        let settings = NSDictionary::from_slices(&[&*key], &values);
        let output = AVAssetReaderTrackOutput::initWithTrack_outputSettings(
            AVAssetReaderTrackOutput::alloc(),
            &track,
            Some(&settings),
        );
        reader.addOutput(&output);
        assert!(reader.startReading());
        let mut count = 0;
        while output.copyNextSampleBuffer().is_some() {
            count += 1;
        }
        assert_eq!(reader.status(), AVAssetReaderStatus::Completed);
        (
            count,
            size.width,
            size.height,
            duration.value,
            duration.timescale,
        )
    })
}

#[test]
fn native_conversion_preserves_frames_dimensions_duration_and_source() {
    let directory = tempfile::tempdir().unwrap();
    for (name, bytes) in [("h264", H264), ("hevc", HEVC)] {
        let source = directory.path().join(format!("{name}.mp4"));
        let destination = directory.path().join(format!("{name}-output.mp4"));
        fs::write(&source, bytes).unwrap();
        fs::write(&destination, []).unwrap();
        let requests = READY_REQUESTS.get();
        let normalized = NORMALIZED_SOURCES.get();
        assert!(transcode(&source, &destination, &AtomicBool::new(false)).unwrap());
        assert_eq!(READY_REQUESTS.get() - requests, 1);
        assert_eq!(NORMALIZED_SOURCES.get(), normalized);
        let before = decoded_frames(&source);
        let after = decoded_frames(&destination);
        assert!(before.0 > 0);
        assert_eq!((after.0, after.1, after.2), (before.0, before.1, before.2));
        assert_eq!(
            i128::from(after.3) * i128::from(before.4),
            i128::from(before.3) * i128::from(after.4)
        );
        assert_eq!(fs::read(source).unwrap(), bytes);
    }
}

#[test]
fn hev1_conversion_uses_validated_hvc1_without_changing_the_original() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    let destination = directory.path().join("output.mp4");
    fs::write(&source, HEV1).unwrap();
    fs::write(&destination, []).unwrap();
    let original =
        plain_libs::media::isobmff::VideoTrack::read_hevc(&mut fs::File::open(&source).unwrap())
            .unwrap();
    let normalized = NORMALIZED_SOURCES.get();
    assert!(transcode(&source, &destination, &AtomicBool::new(false)).unwrap());
    assert_eq!(NORMALIZED_SOURCES.get() - normalized, 1);
    let after = decoded_frames(&destination);
    assert_eq!(after.0, original.sample_count as usize);
    assert_eq!(
        (after.1, after.2),
        (f64::from(original.width), f64::from(original.height))
    );
    assert_eq!(
        i128::from(after.3) * i128::from(original.timescale),
        i128::from(original.duration_ticks) * i128::from(after.4)
    );
    assert_eq!(fs::read(&source).unwrap(), HEV1);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn invalid_hev1_configuration_leaves_no_intermediate_or_output_changes() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    let destination = directory.path().join("output.mp4");
    let mut bytes = HEV1.to_vec();
    let configuration = bytes.windows(4).position(|word| word == b"hvcC").unwrap() + 4;
    bytes[configuration] = 0;
    fs::write(&source, &bytes).unwrap();
    fs::write(&destination, b"reserved").unwrap();
    assert!(transcode(&source, &destination, &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert_eq!(fs::read(destination).unwrap(), b"reserved");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn cancellation_and_invalid_input_preserve_the_reserved_destination() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    let destination = directory.path().join("output.mp4");
    fs::write(&source, H264).unwrap();
    fs::write(&destination, b"reserved").unwrap();
    let error = transcode(&source, &destination, &AtomicBool::new(true)).unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert_eq!(fs::read(&destination).unwrap(), b"reserved");
    fs::write(&source, b"invalid media").unwrap();
    assert!(transcode(&source, &destination, &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(&destination).unwrap(), b"reserved");
}

#[test]
#[ignore = "Offline native video throughput benchmark"]
fn native_video_throughput() {
    let directory = tempfile::tempdir().unwrap();
    let source = match std::env::var_os("PLAIN_VIDEO_BENCH_INPUT") {
        Some(path) => std::path::PathBuf::from(path),
        None => {
            let source = directory.path().join("source.mp4");
            fs::write(&source, HEVC).unwrap();
            source
        }
    };
    let original = fs::read(&source).unwrap();
    let before = decoded_frames(&source);
    for index in 0..6 {
        let destination = directory.path().join(format!("output-{index}.mp4"));
        fs::write(&destination, []).unwrap();
        let started = std::time::Instant::now();
        assert!(transcode(&source, &destination, &AtomicBool::new(false)).unwrap());
        let elapsed = started.elapsed();
        let after = decoded_frames(&destination);
        assert_eq!((after.0, after.1, after.2), (before.0, before.1, before.2));
        assert_eq!(
            i128::from(after.3) * i128::from(before.4),
            i128::from(before.3) * i128::from(after.4)
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        println!(
            "native_video_bench sample={index} elapsed_ns={} output_bytes={}",
            elapsed.as_nanos(),
            fs::metadata(destination).unwrap().len()
        );
    }
}
