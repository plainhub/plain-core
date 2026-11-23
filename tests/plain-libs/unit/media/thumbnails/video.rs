//! Unit tests for `src/media/thumb_engine/video.rs` — moved out-of-line; compiled
//! as the `tests` child module via `#[cfg(test)] #[path]` there.
use super::*;

fn testdata(name: &str) -> String {
    format!(
        "{}/../plain-rs/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    )
}

#[test]
fn compute_video_timepoint_matrix() {
    assert_eq!(compute_video_timepoint(2), 0.0);
    assert_eq!(compute_video_timepoint(30), 1.0);
    assert_eq!(compute_video_timepoint(120), 12.0);
    assert_eq!(compute_video_timepoint(0), 0.0);
}

#[test]
fn length_prefixed_nals_borrow_the_sample() {
    for (bytes, length_size, expected) in [
        (
            vec![0, 0, 0, 3, 1, 2, 3, 0, 0, 0, 1, 0xaa],
            4,
            vec![4..7, 11..12],
        ),
        (vec![0, 2, 0xbb, 0xcc, 0, 1, 0xdd], 2, vec![2..4, 6..7]),
        (vec![0, 0, 1, 0xee], 3, vec![3..4]),
    ] {
        let ptr = bytes.as_ptr();
        let sample = Sample::new(bytes, length_size).unwrap();
        assert_eq!(sample.bytes.as_ptr(), ptr);
        assert_eq!(sample.units, expected);
        for (nal, offset) in sample.nals().zip(expected) {
            assert_eq!(nal.as_ptr(), ptr.wrapping_add(offset.start));
        }
    }
    for (bytes, length_size) in [
        (vec![0, 0, 0, 9, 1, 2], 4),
        (vec![0, 0, 0, 0, 1], 4),
        (vec![0, 0, 0, 1, 9, 7], 4),
        (vec![1], 5),
    ] {
        assert!(Sample::new(bytes, length_size).is_err());
    }
}

fn full_rgb(frame: &DecodedFrame) -> Bitmap {
    match frame {
        DecodedFrame::Avc(frame) => {
            let mut pixels = vec![0; frame.width * frame.height * 3];
            for (y, row) in pixels.chunks_exact_mut(frame.width * 3).enumerate() {
                avc_rgb_row(frame, y, 0, row);
            }
            Bitmap::new_rgb(pixels, frame.width as u32, frame.height as u32)
        }
        DecodedFrame::Hevc(frame) => {
            let image = heic_rs::color::convert(
                frame,
                None,
                frame.color.unwrap_or_default(),
                heic_rs::PixelLayout::Rgb8,
                MAX_SOURCE_PIXELS,
                Some(1),
            )
            .unwrap();
            Bitmap::new_rgb(image.data, image.width, image.height)
        }
    }
}

#[test]
fn yuv_to_rgb_known_colors() {
    // Limited-range BT.601 anchors (see yuv420_to_rgb). 4×2 frame: left
    // half red, right half white — luma per pixel, chroma per 2×2 block.
    let mut f = YuvFrame {
        width: 4,
        height: 2,
        y: vec![81, 81, 235, 235, 81, 81, 235, 235],
        u: vec![90, 128],
        v: vec![240, 128],
    };
    f.y.shrink_to_fit();
    let bm = full_rgb(&DecodedFrame::Avc(f));
    assert_eq!((bm.width(), bm.height(), bm.channels()), (4, 2, 3));
    let (d, _, _) = bm.raw();
    let px = |i: usize| (d[i * 3], d[i * 3 + 1], d[i * 3 + 2]);
    let red_px = px(0);
    assert!(
        red_px.0 > 240 && red_px.1 < 16 && red_px.2 < 16,
        "anchor red, got {red_px:?}"
    );
    assert!(px(4).0 > 240 && px(4).2 < 16, "row 2 left is red too");
    let white_px = px(2);
    assert!(
        white_px.0 > 230 && white_px.1 > 230 && white_px.2 > 230,
        "anchor white, got {white_px:?}"
    );
    // Black anchor in a separate 2×2 frame.
    let black = YuvFrame {
        width: 2,
        height: 2,
        y: vec![16, 16, 16, 16],
        u: vec![128],
        v: vec![128],
    };
    let bm = full_rgb(&DecodedFrame::Avc(black));
    let (d, _, _) = bm.raw();
    assert!(d.iter().all(|&v| v < 16), "anchor black, got {:?}", &d[..6]);
}

#[test]
fn error_paths_do_not_panic() {
    // Garbage bytes with an mp4 extension.
    let tmp = crate::temporary::tempdir().unwrap();
    let garbage = tmp.path().join("garbage.mp4");
    std::fs::write(&garbage, b"definitely not an mp4 file at all....").unwrap();
    assert!(plan_keyframe(garbage.to_str().unwrap()).is_err());
    // Truncated real MP4: moov survives, sample read fails or NAL parse
    // fails — either way an Err, never a panic.
    let full = std::fs::read(testdata("video-h264-baseline.mp4")).unwrap();
    let trunc = tmp.path().join("trunc.mp4");
    std::fs::write(&trunc, &full[..full.len() / 3]).unwrap();
    let _ = plan_keyframe(trunc.to_str().unwrap());
    // Empty file.
    let empty = tmp.path().join("empty.mp4");
    std::fs::write(&empty, b"").unwrap();
    assert!(plan_keyframe(empty.to_str().unwrap()).is_err());
}

#[test]
fn end_to_end_h264_mp4_frame() {
    for name in ["video-h264-baseline.mp4", "video-h264-high.mp4"] {
        let path = testdata(name);
        let jpg = generate_video_thumbnail(&path, 256, 256, 75).unwrap();
        assert!(jpg.starts_with(&[0xFF, 0xD8]), "{name}: JPEG magic");
        let decoded = crate::image::load_from_memory(&jpg).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (256, 192),
            "{name}: 320x240 → 256 box"
        );
        // The frame must carry real content (testsrc2 gradients), not a
        // blank/garbage decode.
        let img = decoded.to_rgb8();
        let n = f64::from(img.width() * img.height());
        let mut mean = 0f64;
        for p in img.pixels() {
            mean += (f64::from(p[0]) + f64::from(p[1]) + f64::from(p[2])) / 3.0;
        }
        mean /= n;
        let mut var = 0f64;
        for p in img.pixels() {
            let m = (f64::from(p[0]) + f64::from(p[1]) + f64::from(p[2])) / 3.0;
            var += (m - mean) * (m - mean);
        }
        let std_dev = (var / n).sqrt();
        assert!(
            std_dev > 20.0,
            "{name}: decoded frame looks blank (stddev {std_dev:.1})"
        );
    }
}

#[test]
fn long_video_targets_ten_percent_timepoint() {
    // 65 s video → timepoint 6.5 s, well past the first GOP: the plan
    // must start at a mid-stream keyframe, not sample 1.
    let plan = plan_keyframe(&testdata("video-h264-65s.mp4")).unwrap();
    assert!(plan.first_sample_id > 1, "expected a mid-stream keyframe");
    assert!(plan.estimated_bytes() > 0);
    let jpg = decode_plan_to_jpeg(plan, 256, 256, 75).unwrap();
    assert!(crate::image::load_from_memory(&jpg).is_ok());
}

#[test]
fn fragmented_mp4_preserves_the_existing_frame_access_units_and_decoded_pixels() {
    let classic = plan_keyframe(&testdata("video-h264-baseline.mp4")).unwrap();
    let fragmented = plan_keyframe(&testdata("video/video-h264-fragmented.mp4")).unwrap();
    assert_eq!(fragmented.first_sample_id, classic.first_sample_id);
    assert_eq!(fragmented.estimated_bytes(), classic.estimated_bytes());
    let (
        EncodedFrame::Avc {
            samples: fragmented_units,
            ..
        },
        EncodedFrame::Avc {
            samples: classic_units,
            ..
        },
    ) = (&fragmented.frame, &classic.frame)
    else {
        panic!("expected AVC frame plans");
    };
    assert_eq!(fragmented_units, classic_units);
    assert!(fragmented_units.len() <= MAX_FOLLOW_SAMPLES as usize + 1);
    let classic = full_rgb(&decode_frame(classic).unwrap());
    let fragmented = full_rgb(&decode_frame(fragmented).unwrap());
    assert_eq!(fragmented.raw(), classic.raw());
}

#[test]
fn end_to_end_hevc_mp4_keyframes() {
    for file in [
        "video-h265.mp4",
        "video-h265-hvc1.mp4",
        "video/video-h265-fragmented.mp4",
    ] {
        let path = testdata(file);
        let frame = generate_video_thumbnail(&path, 256, 256, 75).unwrap();
        let image = crate::image::load_from_memory(&frame).unwrap().to_rgb8();
        assert_eq!(image.width().max(image.height()), 256, "{file}");
        assert!(
            image.pixels().any(|pixel| pixel != image.get_pixel(0, 0)),
            "{file}"
        );
    }
}

#[test]
fn streamed_video_thumbnail_pixels_match_full_rgb_for_fit_and_crop() {
    for file in [
        "video-h265.mp4",
        "video-h265-hvc1.mp4",
        "video-h264-high.mp4",
    ] {
        let plan = plan_keyframe(&testdata(file)).unwrap();
        let frame = decode_frame(plan).unwrap();
        let (source_w, source_h) = frame.dimensions();
        for (w, h) in [(256, 256), (73, 97), (31, 29), (640, 480)] {
            let (fit_w, fit_h) = super::super::compute_target_size(source_w, source_h, w, h);
            let expected = scale::resize_to(full_rgb(&frame), fit_w, fit_h);
            assert_eq!(
                render_frame(&frame, (w, h), false, None).unwrap().raw(),
                expected.raw(),
                "{file}, fit {w}x{h}"
            );
            let expected = scale::resize_to_fill(full_rgb(&frame), w, h);
            assert_eq!(
                render_frame(&frame, (w, h), true, None).unwrap().raw(),
                expected.raw(),
                "{file}, crop {w}x{h}"
            );
        }
    }
}

#[test]
fn hevc_color_conversion_streams_only_cropped_rows_from_a_reused_band() {
    let plan = plan_keyframe(&testdata("video-h265.mp4")).unwrap();
    let decoded = decode_frame(plan).unwrap();
    let full_rgb = full_rgb(&decoded);
    let (pixels, _, _) = full_rgb.raw();
    let DecodedFrame::Hevc(frame) = decoded else {
        panic!("expected HEVC frame");
    };
    let columns = 3..20;
    let rows = 5..38;
    let mut addresses = std::collections::HashSet::new();
    let mut y = rows.start;
    heic_rs::color::rgb8_rows(
        &frame,
        columns.clone(),
        rows.clone(),
        MAX_SOURCE_PIXELS,
        |row| {
            let offset = y * frame.width;
            let range =
                ((offset + columns.start) * 3) as usize..((offset + columns.end) * 3) as usize;
            assert_eq!(row, &pixels[range]);
            addresses.insert(row.as_ptr());
            y += 1;
        },
    )
    .unwrap();
    assert_eq!(y, rows.end);
    assert!(addresses.len() <= rows.len() / 2);
}

#[test]
#[ignore]
fn hevc_4k_thumbnail_post_decode_benchmark() {
    use heic_rs::hevc::{ChromaFormat, Frame};
    let (width, height) = (3840, 2160);
    let (chroma_w, chroma_h) = (width / 2, height / 2);
    let sample = |i: usize| ((i ^ (i / 1531)) % 876 + 64) as u16;
    let frame = DecodedFrame::Hevc(Frame {
        width,
        height,
        bit_depth: 10,
        chroma: ChromaFormat::Yuv420,
        y: (0..(width * height) as usize).map(sample).collect(),
        cb: (0..(chroma_w * chroma_h) as usize).map(sample).collect(),
        cr: (0..(chroma_w * chroma_h) as usize)
            .map(|i| sample(i + 511))
            .collect(),
        y_stride: width,
        c_stride: chroma_w,
        color: None,
    });
    let original = || scale::resize_to_fill(full_rgb(&frame), 256, 256);
    let streamed = || render_frame(&frame, (256, 256), true, None).unwrap();
    assert_eq!(original().raw(), streamed().raw());
    let mut baseline = Vec::new();
    let mut optimized = Vec::new();
    let measure = |func: &dyn Fn() -> Bitmap| {
        let start = std::time::Instant::now();
        for _ in 0..8 {
            std::hint::black_box(func());
        }
        start.elapsed().as_secs_f64() * 1000.
    };
    for round in 0..7 {
        if round % 2 == 0 {
            baseline.push(measure(&original));
            optimized.push(measure(&streamed));
        } else {
            optimized.push(measure(&streamed));
            baseline.push(measure(&original));
        }
    }
    baseline.sort_by(f64::total_cmp);
    optimized.sort_by(f64::total_cmp);
    println!(
        "4k_frames_per_batch=8 rounds=7 baseline_median_ms={:.3} streamed_median_ms={:.3}",
        baseline[3], optimized[3]
    );
}
