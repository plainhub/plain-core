//! Video thumbnail: pure-Rust MP4 demux + H.264 keyframe decode.
//!
//! MP4 family files (mp4/m4v/mov) with an AVC (`avc1`) video track: locate
//! the keyframe sample nearest the heuristic timepoint via the sample tables
//! (stts decode timestamps + stss sync samples), read only that sample plus
//! a bounded number of successors, convert the avcC length-prefixed NALs to
//! Annex-B, decode one frame, convert planar YUV 4:2:0 → RGB, and hand the
//! bitmap to the shared resize/encode pipeline.
//!
//! Everything else (mkv/avi/flv/webm containers, HEVC/VP9 tracks) has no
//! pure-Rust decoder and fails cleanly — the engine still tries cover art
//! first, and a cover-less video then answers `204 No Content`.

use anyhow::{Result, bail};

use crate::media::thumbnails::encode;
use crate::media::thumbnails::scale;
use crate::media::thumbnails::scale::Bitmap;
use rusty_h264_common::YuvFrame;
use rusty_h264_decoder::Decoder;

/// How many samples past the located keyframe we feed the decoder at most
/// (covers multi-sample pictures and streams whose "sync" sample is not a
/// clean IDR).
const MAX_FOLLOW_SAMPLES: u32 = 16;

/// Cap on total sample bytes read for one thumbnail.
const MAX_SAMPLE_BYTES: u64 = 4 * 1024 * 1024;

/// Demuxed input for one frame extraction: Annex-B access units (first one
/// carries the avcC parameter sets and the located keyframe) plus the coded
/// frame size for admission pricing.
#[derive(Debug)]
pub struct FramePlan {
    access_units: Vec<Vec<u8>>,
    frame_w: u32,
    frame_h: u32,
    /// 1-based sample id the plan starts at (diagnostics / tests).
    #[allow(dead_code)] // asserted by the frame-plan tests
    first_sample_id: u32,
}

impl FramePlan {
    /// Admission pricing: decoded YUV420 (1.5 B/px) + RGB bitmap (3 B/px),
    /// rounded up.
    pub fn estimated_bytes(&self) -> u64 {
        u64::from(self.frame_w) * u64::from(self.frame_h) * 5
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
    let track = plain_rs::media::isobmff::AvcTrack::read(&mut reader)?;

    let frame_w = track.width;
    let frame_h = track.height;
    if u64::from(frame_w) * u64::from(frame_h)
        > crate::media::thumbnails::admission::MAX_SOURCE_PIXELS
    {
        bail!("video frame {frame_w}x{frame_h} exceeds pixel guard");
    }

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
    let nal_length_size = track.nal_length_size;

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
        length_prefixed_to_annexb(&sample, nal_length_size, &mut au)?;
        access_units.push(au);
    }
    if access_units.is_empty() {
        bail!("no readable samples at id {start_id}");
    }
    Ok(FramePlan {
        access_units,
        frame_w,
        frame_h,
        first_sample_id: start_id,
    })
}

/// Phase 2 (CPU-heavy): decode until the first frame comes out, YUV→RGB.
pub fn decode_plan(plan: FramePlan) -> Result<Bitmap> {
    let mut dec = Decoder::new();
    let mut frame: Option<YuvFrame> = None;
    let mut last_err: Option<rusty_h264_decoder::DecodeError> = None;
    for au in &plan.access_units {
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
        Some(e) => anyhow::anyhow!("H.264 decode failed: {e}"),
        None => anyhow::anyhow!("H.264 stream yielded no frame"),
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
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&rest[..len]);
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
#[path = "../../../tests/unit/media/thumbnails/video.rs"]
mod tests;
