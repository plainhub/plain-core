use super::*;
use sha2::{Digest, Sha256};

#[test]
fn existing_video_samples_preserve_bytes_metadata_and_keyframes() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../testdata/video/mp4-sample-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["fixture"], "video-sample-index");
    let mut count = 0;
    for case in reference["files"].as_array().unwrap() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(case["file"].as_str().unwrap());
        let mut file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
        let track = AvcTrack::read(&mut file).unwrap();
        for (actual, field) in [
            (track.width, "width"),
            (track.height, "height"),
            (track.timescale, "timescale"),
            (track.sample_count, "sample_count"),
            (track.nal_length_size as u32, "nal_length_size"),
        ] {
            assert_eq!(u64::from(actual), case[field].as_u64().unwrap(), "{field}");
        }
        assert_eq!(
            track.duration_ticks / u64::from(track.timescale),
            case["duration_secs"].as_u64().unwrap()
        );
        let parameter_sets: Vec<Vec<u8>> =
            serde_json::from_value(case["parameter_sets"].clone()).unwrap();
        assert_eq!(track.parameter_sets, parameter_sets);
        let samples = case["samples"].as_array().unwrap();
        for tick in [
            0,
            u64::from(track.timescale),
            track.duration_ticks / 10,
            track.duration_ticks / 2,
            track.duration_ticks,
            u64::MAX,
        ] {
            let candidate = samples
                .iter()
                .rposition(|sample| sample["time"].as_u64().unwrap() <= tick)
                .unwrap_or(0);
            let sync = samples[..=candidate]
                .iter()
                .rev()
                .find(|sample| sample["sync"].as_bool().unwrap())
                .or_else(|| {
                    samples
                        .iter()
                        .find(|sample| sample["sync"].as_bool().unwrap())
                })
                .unwrap();
            assert_eq!(track.keyframe_at(tick), sync["id"].as_u64().unwrap() as u32);
        }
        for sample in case["samples"].as_array().unwrap() {
            let id = sample["id"].as_u64().unwrap() as u32;
            let data = track
                .read_sample(&mut file, id, 4 * 1024 * 1024)
                .unwrap()
                .unwrap();
            assert_eq!(data.len() as u64, sample["size"].as_u64().unwrap());
            assert_eq!(
                crate::utils::base64::base64_encode(&Sha256::digest(&data)),
                sample["sha256_base64"].as_str().unwrap(),
                "id={id}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 664);
}
