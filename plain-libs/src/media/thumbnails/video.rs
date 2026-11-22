//! Extract an AVC or HEVC random-access frame from an MP4-family video.

use crate::anyhow::{Result, bail, ensure};
use std::ops::Range;

use crate::h264::common::YuvFrame;
use crate::h264::decoder::Decoder;
use crate::media::isobmff::VideoTrack;
use crate::media::thumbnails::admission::MAX_SOURCE_PIXELS;
use crate::media::thumbnails::encode;
use crate::media::thumbnails::scale;
use crate::media::thumbnails::scale::Bitmap;
use crate::media::thumbnails::still::rows::Rows;

/// How many samples past the located keyframe we feed the decoder at most
/// (covers multi-sample pictures and streams whose "sync" sample is not a
/// clean IDR).
const MAX_FOLLOW_SAMPLES: u32 = 16;

/// Cap on total sample bytes read for one thumbnail.
const MAX_SAMPLE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, PartialEq)]
struct Sample {
    bytes: Vec<u8>,
    units: Vec<Range<usize>>,
}

impl Sample {
    fn new(bytes: Vec<u8>, length_size: usize) -> Result<Self> {
        ensure!((1..=4).contains(&length_size), "invalid NAL length size");
        let mut units = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            let prefix = bytes
                .get(offset..offset + length_size)
                .ok_or_else(|| crate::anyhow::anyhow!("trailing bytes after last NAL"))?;
            let len = prefix
                .iter()
                .fold(0usize, |size, &byte| (size << 8) | usize::from(byte));
            ensure!(len != 0, "zero-length NAL in sample");
            let start = offset + length_size;
            ensure!(
                len <= bytes.len() - start,
                "NAL length exceeds remaining sample bytes"
            );
            offset = start + len;
            units.push(start..offset);
        }
        Ok(Self { bytes, units })
    }

    fn nals(&self) -> impl Iterator<Item = &[u8]> {
        self.units.iter().map(|range| &self.bytes[range.clone()])
    }
}

#[derive(Debug)]
enum EncodedFrame {
    Avc {
        parameter_sets: Vec<Vec<u8>>,
        samples: Vec<Sample>,
    },
    Hevc {
        parameter_sets: Vec<Vec<u8>>,
        sample: Sample,
    },
}

fn hevc_parameter_sets<'a>(sets: &'a [Vec<u8>], sample: &'a Sample) -> Vec<&'a [u8]> {
    sets.iter()
        .map(Vec::as_slice)
        .chain(
            sample
                .nals()
                .filter(|nal| (32..=34).contains(&hevc_nal_type(nal))),
        )
        .collect()
}

fn hevc_nal_type(nal: &[u8]) -> u8 {
    (nal[0] >> 1) & 63
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
    if matches!(&track.sample_entry, b"hvc1" | b"hev1") {
        let bytes = track
            .read_sample(&mut reader, start_id, MAX_SAMPLE_BYTES as usize)?
            .filter(|sample| !sample.is_empty())
            .ok_or_else(|| crate::anyhow::anyhow!("no readable HEVC keyframe"))?;
        let sample = Sample::new(bytes, track.nal_length_size)?;
        ensure!(
            sample.nals().all(|nal| nal.len() >= 2),
            "short HEVC NAL header"
        );
        let parameter_sets = track.parameter_sets;
        let coded_pixels =
            heic_rs::hevc::coded_pixels(&hevc_parameter_sets(&parameter_sets, &sample))?;
        ensure!(
            coded_pixels <= MAX_SOURCE_PIXELS,
            "HEVC keyframe exceeds pixel guard"
        );
        return Ok(FramePlan {
            frame: EncodedFrame::Hevc {
                parameter_sets,
                sample,
            },
            estimated_bytes: coded_pixels.max(pixels) * 16,
            #[cfg(test)]
            first_sample_id: start_id,
        });
    }

    let mut samples = Vec::new();
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
        samples.push(Sample::new(sample, track.nal_length_size)?);
    }
    if samples.is_empty() {
        bail!("no readable samples at id {start_id}");
    }
    Ok(FramePlan {
        frame: EncodedFrame::Avc {
            parameter_sets: track.parameter_sets,
            samples,
        },
        estimated_bytes: pixels * 5,
        #[cfg(test)]
        first_sample_id: start_id,
    })
}

enum DecodedFrame {
    Avc(YuvFrame),
    Hevc(heic_rs::hevc::Frame),
}

impl DecodedFrame {
    fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Avc(frame) => (frame.width as u32, frame.height as u32),
            Self::Hevc(frame) => (frame.width, frame.height),
        }
    }
}

fn decode_frame(plan: FramePlan) -> Result<DecodedFrame> {
    let (parameter_sets, samples) = match plan.frame {
        EncodedFrame::Avc {
            parameter_sets,
            samples,
        } => (parameter_sets, samples),
        EncodedFrame::Hevc {
            parameter_sets,
            sample,
        } => {
            let parameter_sets = hevc_parameter_sets(&parameter_sets, &sample);
            let slices: Vec<_> = sample
                .nals()
                .filter(|nal| hevc_nal_type(nal) <= 31)
                .collect();
            return Ok(DecodedFrame::Hevc(heic_rs::hevc::decode_still_limited(
                &parameter_sets,
                &slices,
                MAX_SOURCE_PIXELS,
            )?));
        }
    };
    let mut decoder = Decoder::new();
    let mut last_error = None;
    for (index, sample) in samples.iter().enumerate() {
        let sets = if index == 0 {
            parameter_sets.as_slice()
        } else {
            &[]
        };
        match decoder.decode_nals(sets.iter().map(Vec::as_slice).chain(sample.nals())) {
            Ok(Some(frame)) => return Ok(DecodedFrame::Avc(frame)),
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
    }
    match last_error {
        Some(error) => bail!("H.264 decode failed: {error}"),
        None => bail!("H.264 stream yielded no frame"),
    }
}

pub fn decode_plan_to_jpeg(
    plan: FramePlan,
    width: u32,
    height: u32,
    quality: u8,
) -> Result<Vec<u8>> {
    encode_thumbnail(plan, width, height, false, quality)
}

pub(super) fn encode_thumbnail(
    plan: FramePlan,
    width: u32,
    height: u32,
    center_crop: bool,
    quality: u8,
) -> Result<Vec<u8>> {
    encode::encode_jpeg(
        render_frame(&decode_frame(plan)?, width, height, center_crop)?,
        quality,
    )
}

fn render_frame(
    frame: &DecodedFrame,
    width: u32,
    height: u32,
    center_crop: bool,
) -> Result<Bitmap> {
    let (source_w, source_h) = frame.dimensions();
    let crop = center_crop.then_some((width, height));
    let (crop_w, crop_h) = scale::crop_size(source_w, source_h, crop);
    let (target_w, target_h) =
        crop.unwrap_or_else(|| super::compute_target_size(source_w, source_h, width, height));
    let left = (source_w - crop_w) / 2;
    let top = (source_h - crop_h) / 2;
    let mut rows = Rows::<3>::new(crop_w, crop_h, target_w, target_h);
    match frame {
        DecodedFrame::Avc(frame) => {
            let mut rgb = vec![0; crop_w as usize * 3];
            for y in top..top + crop_h {
                avc_rgb_row(frame, y as usize, left as usize, &mut rgb);
                rows.push(&rgb);
            }
        }
        DecodedFrame::Hevc(frame) => {
            heic_rs::color::rgb8_rows(
                frame,
                left..left + crop_w,
                top..top + crop_h,
                MAX_SOURCE_PIXELS,
                |row| rows.push(row),
            )?;
        }
    }
    let (rgb, width, height) = rows.finish();
    Ok(scale::fir_resize(
        Bitmap::new_rgb(rgb, width, height),
        target_w,
        target_h,
    ))
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

fn avc_rgb_row(frame: &YuvFrame, y: usize, left: usize, rgb: &mut [u8]) {
    let row_y = y * frame.width;
    let row_c = (y >> 1) * frame.chroma_width();
    for (x, pixel) in rgb.chunks_exact_mut(3).enumerate() {
        let x = left + x;
        let c = i32::from(frame.y[row_y + x]) - 16;
        let d = i32::from(frame.u[row_c + (x >> 1)]) - 128;
        let e = i32::from(frame.v[row_c + (x >> 1)]) - 128;
        pixel.copy_from_slice(&[
            clamp_byte((298 * c + 409 * e + 128) >> 8),
            clamp_byte((298 * c - 100 * d - 208 * e + 128) >> 8),
            clamp_byte((298 * c + 516 * d + 128) >> 8),
        ]);
    }
}

fn clamp_byte(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/video.rs"]
mod tests;
