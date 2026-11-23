//! High-performance pure-Rust thumbnail engine.
//!
//! Decodes and resizes images, embedded covers and video keyframes. The caller owns the cache.

pub mod admission;
mod bmp;
pub mod decode;
pub mod encode;
pub mod exif;
pub mod lru;
pub mod pjpeg;
pub mod scale;
mod thumb_hash;
pub mod sniff;
mod still;
pub mod video;
mod workers;

use crate::anyhow::{Result, bail};
use bytes::Bytes;

use exif::Orientation;
use scale::Bitmap;
use sniff::{ImageKind, Sniffed};

/// Largest thumbnail box side accepted from query params.
pub const MAX_TARGET_DIM: u32 = 16_384;

/// Files at or below this size that already fit the target box are served
/// as-is (passthrough) instead of being re-encoded.
pub const PASSTHROUGH_MAX_BYTES: u64 = 1024 * 1024;

/// How much of a file we read to sniff + parse EXIF orientation.
const HEAD_READ: usize = 256 * 1024;

/// Big-to-small JPEG requests (downscale ratio ≥ 2 on either axis) decode
/// via `jpeg-decoder` with IDCT scaling, so the full-resolution bitmap
/// never materializes — the same shrink-on-load libvips/ffmpeg use. Near
/// full-size requests go through the `image` crate (zune-jpeg), whose
/// full decode is faster when almost every decoded pixel is needed.
fn jpeg_use_scaled(w: u32, h: u32, tw: u32, th: u32) -> bool {
    (tw > 0 && w >= tw * 2) || (th > 0 && h >= th * 2)
}

// The DC grid must cover both target axes before discarding AC coefficient values.
fn dc_fast_eligible(s: Sniffed, tw: u32, th: u32) -> bool {
    tw > 0 && th > 0 && tw <= s.width.div_ceil(8) && th <= s.height.div_ceil(8)
}

#[derive(Clone)]
struct ThumbSpec {
    path: String,
    w: u32,
    h: u32,
    quality: u8,
    file_size: i64,
    center_crop: bool,
    short_edge: Option<u32>,
}

impl ThumbSpec {
    /// Clamp hostile query params into a sane request.
    fn sanitize(path: &str, w: i32, h: i32, quality: i32, size: i64) -> Self {
        ThumbSpec {
            path: path.to_string(),
            w: w.clamp(0, MAX_TARGET_DIM as i32) as u32,
            h: h.clamp(0, MAX_TARGET_DIM as i32) as u32,
            quality: quality.clamp(1, 100) as u8,
            file_size: size,
            center_crop: false,
            short_edge: None,
        }
    }
}

pub fn original_fits(
    width: u32,
    height: u32,
    target_width: u32,
    target_height: u32,
    _center_crop: bool,
    bytes: i64,
) -> bool {
    u64::try_from(bytes).is_ok_and(|size| size <= PASSTHROUGH_MAX_BYTES)
        && (target_width == 0 || width <= target_width)
        && (target_height == 0 || height <= target_height)
}

pub fn read_original_thumbnail(
    path: &std::path::Path,
    width: u32,
    height: u32,
    short_edge: Option<u32>,
) -> std::io::Result<Option<Vec<u8>>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    if size > PASSTHROUGH_MAX_BYTES {
        return Ok(None);
    }
    let mut reader = std::io::BufReader::new(file);
    use std::io::BufRead;
    let head = reader.fill_buf()?;
    if let Ok(source) = sniff::sniff_header(head) {
        if !source.kind.can_passthrough() {
            return Ok(None);
        }
        // A square bounding box and a short-edge limit are independent of EXIF orientation.
        if width != 0 && width == height
            && (source.width.max(source.height) > width
                || short_edge.is_some_and(|limit| source.width.min(source.height) > limit))
        {
            return Ok(None);
        }
    }
    let mut bytes = Vec::new();
    reader.take(PASSTHROUGH_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > PASSTHROUGH_MAX_BYTES {
        return Ok(None);
    }
    let Ok(sniffed) = sniff::sniff_header(&bytes) else {
        return Ok(None);
    };
    if !sniffed.kind.can_passthrough() {
        return Ok(None);
    }
    let orientation = if sniffed.kind == ImageKind::Jpeg {
        exif::orientation(&bytes).unwrap_or(1)
    } else {
        1
    };
    let (source_width, source_height) = if Orientation(orientation).swaps_axes() {
        (sniffed.height, sniffed.width)
    } else {
        (sniffed.width, sniffed.height)
    };
    let (width, height) = match short_edge {
        Some(short) => crate::media::thumbnail_limits::short_edge_size(
            source_width, source_height, short, width,
        ),
        None => (width, height),
    };
    if !original_fits(source_width, source_height, width, height, false, size as i64) {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn image_dimensions(spec: &ThumbSpec, head: &[u8], s: Sniffed) -> (u8, u32, u32, u32, u32) {
    // EXIF orientation first: the *upright* dimensions decide the target box
    // so rotated photos produce correctly-proportioned thumbnails.
    let orientation = if s.kind == ImageKind::Jpeg {
        exif::orientation(head).unwrap_or(1)
    } else {
        s.orientation
    };
    let (uw, uh) = if Orientation(orientation).swaps_axes() {
        (s.height, s.width)
    } else {
        (s.width, s.height)
    };
    let (tw, th) = match spec.short_edge {
        Some(short) => crate::media::thumbnail_limits::short_edge_size(uw, uh, short, spec.w),
        None => compute_target_size(uw, uh, spec.w, spec.h),
    };

    (orientation, uw, uh, tw, th)
}

async fn generate_image(
    spec: &ThumbSpec,
    s: Sniffed,
    orientation: u8,
    tw: u32,
    th: u32,
    head: Vec<u8>,
    with_hash: bool,
) -> Result<Rendered> {
    let swaps = Orientation(orientation).swaps_axes();
    // The stored bitmap must be resized to the swapped target so the
    // post-resize rotation lands exactly on (tw, th).
    let (rw, rh) = if swaps { (th, tw) } else { (tw, th) };

    // Decompression-bomb guard for full decodes; scaled JPEG decodes only
    // materialize their scaled output, so they are priced by that instead.
    let scaled_jpeg = s.kind == ImageKind::Jpeg
        && (jpeg_use_scaled(s.width, s.height, rw, rh) || dc_fast_eligible(s, rw, rh));
    if !scaled_jpeg && u64::from(s.width) * u64::from(s.height) > admission::MAX_SOURCE_PIXELS {
        bail!("source {}x{} exceeds pixel guard", s.width, s.height);
    }
    let preview = if spec.short_edge.is_none() {
        crate::utils::image_metadata::thumbnail(&head).and_then(|bytes| {
            let dimensions = sniff::sniff_header(bytes).ok()?;
            (dimensions.kind == ImageKind::Jpeg && dimensions.width >= rw && dimensions.height >= rh)
                .then(|| (bytes.to_vec(), dimensions))
        })
    } else {
        None
    };
    let est = estimated_decoded_bytes(s, rw, rh).max(estimated_decoded_bytes(
        preview.as_ref().map_or(s, |(_, dimensions)| *dimensions),
        rw,
        rh,
    ));
    let est = est
        .saturating_mul(2)
        .saturating_add(u64::from(spec.w) * u64::from(spec.h) * 4);
    let est = est.saturating_add(if matches!(s.kind, ImageKind::Avif | ImageKind::Heif) {
        (spec.file_size.max(0) as u64).saturating_mul(2)
    } else {
        dc_input_bytes(s, rw, rh, spec.file_size)
    });

    let path = spec.path.clone();
    let quality = spec.quality;
    admission::global()
        .run(est, move || {
            let preview = preview.as_ref().map(|(bytes, _)| bytes.as_slice());
            let bm = decode_to_target(&path, &head, s, rw, rh, preview)?;
            let bm = scale::resize_to(bm, rw, rh);
            let bm = bm.apply_orientation(Orientation(orientation));
            encode_result(bm, quality, s.preserve_alpha, with_hash)
        })
        .await?
}

fn decode_to_target(
    path: &str,
    head: &[u8],
    s: Sniffed,
    rw: u32,
    rh: u32,
    preview: Option<&[u8]>,
) -> Result<Bitmap> {
    if let Some(bytes) = preview
        && let Ok(bitmap) = decode::jpeg_scaled_bytes(bytes, rw, rh)
    {
        return Ok(bitmap);
    }
    let p = std::path::Path::new(path);
    if matches!(s.kind, ImageKind::Tiff | ImageKind::Avif | ImageKind::Heif) {
        return still::thumbnail(p, s.kind, rw, rh);
    }
    if s.kind == ImageKind::Bmp {
        return bmp::thumbnail(p, rw, rh, None);
    }
    if s.kind == ImageKind::Jpeg
        && dc_fast_eligible(s, rw, rh)
        && let Ok(bm) = pjpeg::decode_thumbnail(head, p, None)
    {
        return Ok(bm);
    }
    let bm = if s.kind == ImageKind::Jpeg && jpeg_use_scaled(s.width, s.height, rw, rh) {
        decode::jpeg_scaled(p, rw, rh).or_else(|_| decode::image_full(p))?
    } else {
        decode::image_full(p)?
    };
    Ok(bm)
}

#[allow(dead_code)] // parity with the server: kept for parity tests
fn downscale_ratio(w: u32, h: u32, tw: u32, th: u32) -> f64 {
    let rw = if tw > 0 {
        f64::from(w) / f64::from(tw)
    } else {
        1.0
    };
    let rh = if th > 0 {
        f64::from(h) / f64::from(th)
    } else {
        1.0
    };
    rw.max(rh)
}

fn dc_input_bytes(s: Sniffed, rw: u32, rh: u32, file_size: i64) -> u64 {
    if s.kind == ImageKind::Jpeg && dc_fast_eligible(s, rw, rh) {
        file_size.max(0) as u64
    } else {
        0
    }
}

fn estimated_decoded_bytes(s: Sniffed, rw: u32, rh: u32) -> u64 {
    if s.work_bytes > 0 {
        let factor = (s.width / rw.max(1)).min(s.height / rh.max(1)).clamp(1, 64);
        return s.work_bytes.saturating_add(
            u64::from(s.width.div_ceil(factor)) * u64::from(s.height.div_ceil(factor)) * 4,
        );
    }
    let px: u64 = if s.kind == ImageKind::Jpeg && dc_fast_eligible(s, rw, rh) {
        u64::from(s.width.div_ceil(8)) * u64::from(s.height.div_ceil(8))
    } else if s.kind == ImageKind::Jpeg && jpeg_use_scaled(s.width, s.height, rw, rh) {
        let (sw, sh) = decode::predict_scaled(s.width, s.height, rw, rh);
        u64::from(sw) * u64::from(sh)
    } else {
        u64::from(s.width) * u64::from(s.height)
    };
    px.saturating_mul(4)
}

// ---------------------------------------------------------------------------
// Non-image dispatch: cover art, then pure-Rust video frame extraction
// ---------------------------------------------------------------------------

async fn generate_non_image(spec: &ThumbSpec, with_hash: bool) -> Result<Rendered> {
    let path = spec.path.clone();

    {
        let cover = admission::global()
            .run(0, move || crate::media::cover::extract_cover(&path))
            .await?;
        if let Some(cover) = cover {
            let dimensions = sniff::sniff_header(&cover.bytes)?;
            if u64::from(dimensions.width) * u64::from(dimensions.height)
                > admission::MAX_SOURCE_PIXELS
            {
                bail!("cover exceeds pixel guard");
            }
            let est = u64::from(dimensions.width) * u64::from(dimensions.height) * 8
                + u64::from(spec.w) * u64::from(spec.h) * 4;
            let request = spec.clone();
            return admission::global()
                .run(est, move || {
                    let bitmap = resize_bitmap(decode::image_from_bytes(&cover.bytes)?, &request);
                    encode_result(bitmap, request.quality, false, with_hash)
                })
                .await?;
        }
    }
    if is_video_ext(&spec.path) {
        let path = spec.path.clone();
        let plan = tokio::task::spawn_blocking(move || video::plan_keyframe(&path)).await??;
        let est = plan
            .estimated_bytes()
            .saturating_mul(2)
            .saturating_add(u64::from(spec.w) * u64::from(spec.h) * 4);
        let (width, height, center_crop, quality) =
            (spec.w, spec.h, spec.center_crop, spec.quality);
        return admission::global()
            .run(est, move || {
                let bitmap = video::thumbnail_bitmap(plan, width, height, center_crop)?;
                encode_result(bitmap, quality, false, with_hash)
            })
            .await?;
    }

    bail!("unsupported file type for thumbnail")
}

fn resize_bitmap(bm: Bitmap, spec: &ThumbSpec) -> Bitmap {
    if spec.center_crop {
        scale::resize_to_fill(bm, spec.w, spec.h)
    } else {
        let (w, h) = compute_target_size(bm.width(), bm.height(), spec.w, spec.h);
        scale::resize_to(bm, w, h)
    }
}

struct Rendered {
    bytes: Vec<u8>,
    thumb_hash: Option<Vec<u8>>,
}

fn encode_result(bm: Bitmap, quality: u8, png: bool, with_hash: bool) -> Result<Rendered> {
    let thumb_hash = with_hash.then(|| thumb_hash::from_bitmap(&bm));
    let bytes = if png { encode::encode_png(bm)? } else { encode::encode_jpeg(bm, quality)? };
    Ok(Rendered { bytes, thumb_hash })
}

fn is_video_ext(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "m4v"
    )
}

async fn inspect_image(path: &str) -> Result<(Vec<u8>, Option<Sniffed>)> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let Ok(mut file) = std::fs::File::open(&path) else {
            return Ok((Vec::new(), None));
        };
        let mut head = vec![0; 64];
        let Ok(length) = file.read(&mut head) else {
            return Ok((Vec::new(), None));
        };
        head.truncate(length);
        if let Some(kind) = still::kind(&head) {
            return Ok((head, Some(still::probe(std::path::Path::new(&path), kind)?)));
        }
        if let Ok(source) = sniff::sniff_header(&head)
            && matches!(source.kind, ImageKind::Png | ImageKind::Gif | ImageKind::WebP)
        {
            return Ok((head, Some(source)));
        }
        head.reserve(HEAD_READ - length);
        if file
            .take((HEAD_READ - length) as u64)
            .read_to_end(&mut head)
            .is_err()
        {
            return Ok((Vec::new(), None));
        }
        let facts = sniff::sniff_header(&head).ok();
        Ok((head, facts))
    })
    .await?
}

pub use crate::media::thumbnail_limits::compute_target_size;

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/mod.rs"]
mod tests;

pub async fn generate_still(
    path: &str,
    width: u32,
    height: u32,
    short_edge: Option<u32>,
) -> Result<Option<Bytes>> {
    Ok(still_result(path, width, height, short_edge, false)
        .await?
        .map(|output| Bytes::from(output.bytes)))
}

async fn still_result(
    path: &str,
    width: u32,
    height: u32,
    short_edge: Option<u32>,
    with_hash: bool,
) -> Result<Option<Rendered>> {
    let size = tokio::fs::metadata(path).await?.len();
    if size == 0 {
        return Ok(None);
    }
    let (head, source) = inspect_image(path).await?;
    let source = source.ok_or_else(|| crate::anyhow::anyhow!("unsupported file type for thumbnail"))?;
    let mut spec = ThumbSpec::sanitize(path, width.try_into()?, height.try_into()?, 85, size.try_into()?);
    spec.short_edge = short_edge;
    let (orientation, _, _, tw, th) = image_dimensions(&spec, &head, source);
    Ok(Some(generate_image(&spec, source, orientation, tw, th, head, with_hash).await?))
}

pub async fn render_with_thumb_hash(
    path: &str,
    width: u32,
    height: u32,
    video: bool,
    center_crop: bool,
    short_edge: Option<u32>,
) -> Result<Option<(Bytes, Vec<u8>)>> {
    let rendered = if video {
        let mut spec = ThumbSpec::sanitize(path, width.try_into()?, height.try_into()?, 85, 0);
        spec.center_crop = center_crop;
        Some(generate_non_image(&spec, true).await?)
    } else {
        still_result(path, width, height, short_edge, true).await?
    };
    Ok(rendered.map(|output| (Bytes::from(output.bytes), output.thumb_hash.unwrap())))
}

pub async fn thumb_hash_from_encoded(bytes: Bytes) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let source = sniff::sniff_header(&bytes)?;
        if source.width > 1024 || source.height > 1024 {
            bail!("encoded thumbnail exceeds size limit");
        }
        Ok(thumb_hash::from_bitmap(&decode::image_from_bytes(&bytes)?))
    })
    .await?
}

pub async fn render(
    path: &str,
    width: u32,
    height: u32,
    center_crop: bool,
    short_edge: Option<u32>,
) -> Result<Bytes> {
    let size = tokio::fs::metadata(path).await?.len();
    let mut spec = ThumbSpec::sanitize(path, width.try_into()?, height.try_into()?, 85, size.try_into()?);
    spec.short_edge = short_edge;
    let (head, image) = inspect_image(path).await?;
    let rendered = if let Some(source) = image {
        let (orientation, _, _, tw, th) = image_dimensions(&spec, &head, source);
        generate_image(&spec, source, orientation, tw, th, head, false).await?
    } else {
        if center_crop && (spec.w == 0 || spec.h == 0) {
            bail!("positive crop dimensions required");
        }
        spec.center_crop = center_crop;
        generate_non_image(&spec, false).await?
    };
    Ok(Bytes::from(rendered.bytes))
}

pub async fn png_preview(path: &str) -> Result<Vec<u8>> {
    let owned = path.to_owned();
    let (facts, size) = tokio::task::spawn_blocking(move || -> Result<_> {
        let mut file = std::fs::File::open(&owned)?;
        let size = file.metadata()?.len();
        let mut head = [0; 64];
        let length = std::io::Read::read(&mut file, &mut head)?;
        let kind = still::kind(&head[..length])
            .ok_or_else(|| crate::anyhow::anyhow!("Unsupported preview image"))?;
        Ok((still::probe(std::path::Path::new(&owned), kind)?, size))
    })
    .await??;
    if u64::from(facts.width) * u64::from(facts.height) > admission::MAX_SOURCE_PIXELS {
        bail!("Preview exceeds pixel limit");
    }
    let pixels = u64::from(facts.width) * u64::from(facts.height);
    let cost = facts
        .work_bytes
        .saturating_add(pixels.saturating_mul(8))
        .saturating_add(size.saturating_mul(2));
    let path = path.to_owned();
    admission::global()
        .run(cost, move || {
            let image = still::thumbnail(
                std::path::Path::new(&path),
                facts.kind,
                facts.width,
                facts.height,
            )?;
            encode::encode_png(image.apply_orientation(Orientation(facts.orientation)))
        })
        .await?
}
