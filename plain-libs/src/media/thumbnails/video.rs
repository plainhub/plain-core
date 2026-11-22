//! Extract an AVC or HEVC random-access frame from an MP4-family video.

use crate::anyhow::{Result, bail, ensure};

use crate::h264::common::YuvFrame;
use crate::h264::decoder::Decoder;
use crate::media::isobmff::VideoTrack;
use crate::media::thumbnails::admission::MAX_SOURCE_PIXELS;
use crate::media::thumbnails::encode;
use crate::media::thumbnails::scale;
use crate::media::thumbnails::scale::Bitmap;

/// How many samples past the located keyframe we feed the decoder at most
/// (covers multi-sample pictures and streams whose "sync" sample is not a
/// clean IDR).
const MAX_FOLLOW_SAMPLES: u32 = 16;

/// Cap on total sample bytes read for one thumbnail.
const MAX_SAMPLE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug)]
enum EncodedFrame {
    Avc(Vec<Vec<u8>>),
    Hevc {
        parameter_sets: Vec<Vec<u8>>,
        slices: Vec<Vec<u8>>,
    },
}

#[derive(Debug)]
pub struct FramePlan {
    frame: EncodedFrame,
    estimated_bytes: u64,
    #[cfg(test)]
    first_sample_id: u32,
}

impl FramePlan {
    pub fn estimated_bytes(&self) -> u64 {
        self.estimated_bytes
    }
}

/// One-shot extraction (tests, benchmarks). The HTTP layer uses
/// [`plan_keyframe`] + [`decode_plan_to_jpeg`] so admission control can price
/// the decode (YUV+RGB) by the real frame size between the two phases.
pub fn generate_video_thumbnail(path: &str, w: u32, h: u32, quality: u8) -> Result<Vec<u8>> {
    let plan = plan_keyframe(path)?;
    decode_plan_to_jpeg(plan, w, h, quality)
}

/// Phase 1 (cheap I/O): open the container, pick the video track, locate the
/// keyframe at the heuristic timepoint, read its bytes. Reads the moov box
/// and the target samples only — the file is never loaded whole.
pub fn plan_keyframe(path: &str) -> Result<FramePlan> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let track = VideoTrack::read_thumbnail(&mut reader)?;

    let frame_w = track.width;
    let frame_h = track.height;
    let pixels = u64::from(frame_w) * u64::from(frame_h);
    ensure!(
        pixels <= MAX_SOURCE_PIXELS,
        "video frame {frame_w}x{frame_h} exceeds pixel guard"
    );

    // Real duration straight from the track (mdhd) — the old CLI-based
    // call site passed a hard-coded 0, so every video thumb was frame 0.
    let duration_secs = (track.duration_ticks / u64::from(track.timescale)) as u32;
    let timepoint = compute_video_timepoint(duration_secs);
    let timescale = u64::from(track.timescale);
    let target_tick = (timepoint * timescale as f64) as u64;

    let sample_count = track.sample_count;
    if sample_count == 0 {
        bail!("video track has no samples");
    }

    let start_id = track.keyframe_at(target_tick);
    if track.sample_entry != *b"avc1" {
        let sample = track
            .read_sample(&mut reader, start_id, MAX_SAMPLE_BYTES as usize)?
            .filter(|sample| !sample.is_empty())
            .ok_or_else(|| crate::anyhow::anyhow!("no readable HEVC keyframe"))?;
        let mut parameter_sets = track.parameter_sets;
        let mut slices = Vec::new();
        visit_length_prefixed_nals(&sample, track.nal_length_size, |nal| {
            ensure!(nal.len() >= 2, "short HEVC NAL header");
            let kind = (nal[0] >> 1) & 63;
            if (32..=34).contains(&kind) {
                parameter_sets.push(nal.to_vec());
            } else if kind <= 31 {
                slices.push(nal.to_vec());
            }
            Ok(())
        })?;
        let references: Vec<_> = parameter_sets.iter().map(Vec::as_slice).collect();
        let coded_pixels = heic_rs::hevc::coded_pixels(&references)?;
        ensure!(
            coded_pixels <= MAX_SOURCE_PIXELS,
            "HEVC keyframe exceeds pixel guard"
        );
        return Ok(FramePlan {
            frame: EncodedFrame::Hevc {
                parameter_sets,
                slices,
            },
            estimated_bytes: coded_pixels.max(pixels) * 16,
            #[cfg(test)]
            first_sample_id: start_id,
        });
    }

    let mut access_units = Vec::new();
    let mut first = true;
    let mut total = 0u64;
    let last_id = start_id
        .saturating_add(MAX_FOLLOW_SAMPLES)
        .min(sample_count);
    for id in start_id..=last_id {
        let Some(sample) =
            track.read_sample(&mut reader, id, (MAX_SAMPLE_BYTES - total) as usize)?
        else {
            break;
        };
        total += sample.len() as u64;
        let mut au = Vec::with_capacity(sample.len() + 64);
        if first {
            for nal in &track.parameter_sets {
                au.extend_from_slice(&[0, 0, 0, 1]);
                au.extend_from_slice(nal);
            }
            first = false;
        }
        length_prefixed_to_annexb(&sample, track.nal_length_size, &mut au)?;
        access_units.push(au);
    }
    if access_units.is_empty() {
        bail!("no readable samples at id {start_id}");
    }
    Ok(FramePlan {
        frame: EncodedFrame::Avc(access_units),
        estimated_bytes: pixels * 5,
        #[cfg(test)]
        first_sample_id: start_id,
    })
}

/// Phase 2 (CPU-heavy): decode until the first frame comes out, YUV→RGB.
pub fn decode_plan(plan: FramePlan) -> Result<Bitmap> {
    let access_units = match plan.frame {
        EncodedFrame::Avc(access_units) => access_units,
        EncodedFrame::Hevc {
            parameter_sets,
            slices,
        } => {
            let parameter_sets: Vec<_> = parameter_sets.iter().map(Vec::as_slice).collect();
            let slices: Vec<_> = slices.iter().map(Vec::as_slice).collect();
            let frame =
                heic_rs::hevc::decode_still_limited(&parameter_sets, &slices, MAX_SOURCE_PIXELS)?;
            let image = heic_rs::color::convert(
                &frame,
                None,
                frame.color.unwrap_or_default(),
                heic_rs::PixelLayout::Rgb8,
                MAX_SOURCE_PIXELS,
                Some(1),
            )?;
            return Ok(Bitmap::new_rgb(image.data, image.width, image.height));
        }
    };
    let mut dec = Decoder::new();
    let mut frame: Option<YuvFrame> = None;
    let mut last_err: Option<crate::h264::decoder::DecodeError> = None;
    for au in &access_units {
        match dec.decode(au) {
            Ok(Some(f)) => {
                frame = Some(f);
                break;
            }
            Ok(None) => continue, // AU carried parameter sets / no picture
            Err(e) => {
                // A non-clean sync sample may fail without references; the
                // following samples still have a chance.
                last_err = Some(e);
            }
        }
    }
    let frame = frame.ok_or_else(|| match last_err {
        Some(e) => crate::anyhow::anyhow!("H.264 decode failed: {e}"),
        None => crate::anyhow::anyhow!("H.264 stream yielded no frame"),
    })?;
    Ok(yuv420_to_rgb(&frame))
}

/// Decode → resize into the target box → JPEG (the engine's shared tail).
pub fn decode_plan_to_jpeg(plan: FramePlan, w: u32, h: u32, quality: u8) -> Result<Vec<u8>> {
    let bm = decode_plan(plan)?;
    let (tw, th) = crate::media::thumbnails::compute_target_size(bm.width(), bm.height(), w, h);
    let bm = scale::resize_to(bm, tw, th);
    encode::encode_jpeg(bm, quality)
}

/// Frame timepoint heuristic (unchanged from the original pipeline).
fn compute_video_timepoint(duration_secs: u32) -> f64 {
    if duration_secs < 4 {
        0.0
    } else if duration_secs < 60 {
        1.0
    } else {
        f64::from(duration_secs) * 0.1
    }
}

// ---------------------------------------------------------------------------
// Bitstream plumbing: avcC length-prefixed NALs → Annex-B
// ---------------------------------------------------------------------------

/// Convert an avcC-style length-prefixed NAL stream (as stored in MP4
/// samples) to Annex-B: each NAL gets a 4-byte start code. In-tree because
/// neither the mp4 crate nor the decoder ships this conversion.
pub fn length_prefixed_to_annexb(data: &[u8], length_size: usize, out: &mut Vec<u8>) -> Result<()> {
    visit_length_prefixed_nals(data, length_size, |nal| {
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(nal);
        Ok(())
    })
}

fn visit_length_prefixed_nals(
    data: &[u8],
    length_size: usize,
    mut visit: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    if length_size == 0 || length_size > 4 {
        bail!("NAL length size {length_size} out of range");
    }
    let mut rest = data;
    while rest.len() >= length_size {
        let mut len = 0usize;
        for b in &rest[..length_size] {
            len = (len << 8) | *b as usize;
        }
        if len == 0 {
            bail!("zero-length NAL in sample");
        }
        rest = &rest[length_size..];
        if rest.len() < len {
            bail!("NAL length {len} exceeds remaining sample bytes");
        }
        visit(&rest[..len])?;
        rest = &rest[len..];
    }
    if !rest.is_empty() {
        bail!("trailing bytes after last NAL");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// YUV 4:2:0 → RGB (BT.601 limited range, the H.264 default without VUI)
// ---------------------------------------------------------------------------

fn yuv420_to_rgb(f: &YuvFrame) -> Bitmap {
    let (w, h) = (f.width, f.height);
    let (cw, _ch) = (f.chroma_width(), f.chroma_height());
    let stride_y = w;
    let stride_c = cw;
    debug_assert_eq!(f.y.len(), w * h);
    debug_assert_eq!(f.u.len(), cw * _ch);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        let row_y = y * stride_y;
        let row_c = (y >> 1) * stride_c;
        for x in 0..w {
            let c = i32::from(f.y[row_y + x]) - 16;
            let d = i32::from(f.u[row_c + (x >> 1)]) - 128;
            let e = i32::from(f.v[row_c + (x >> 1)]) - 128;
            rgb.push(clamp_byte((298 * c + 409 * e + 128) >> 8));
            rgb.push(clamp_byte((298 * c - 100 * d - 208 * e + 128) >> 8));
            rgb.push(clamp_byte((298 * c + 516 * d + 128) >> 8));
        }
    }
    Bitmap::new_rgb(rgb, w as u32, h as u32)
}

fn clamp_byte(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/video.rs"]
mod tests;
