//! High-performance pure-Rust thumbnail engine.
//!
//! Pipeline per request (image files):
//!
//! 1. sniff the header (async read of ≤256 KiB) → format + dimensions,
//! 2. small-image passthrough: an image that already fits the target box
//!    (and is ≤1 MiB) is served byte-identical — zero decode, zero encode,
//! 3. hot LRU, then the deterministic file cache
//!    (`{cache_dir}/thumbs/XX/<sha512>.jpg`),
//! 4. single-flight per cache key — concurrent duplicate requests coalesce,
//! 5. two-tier admission (CPU permits + decoded-pixel byte budget),
//! 6. decode (IDCT-scaled JPEG for big→small, `image` crate otherwise)
//!    → box pre-scale → SIMD resample → EXIF orientation → JPEG encode,
//! 7. atomic cache write + LRU insert.
//!
//! Non-image files go through the cover-art path (sidecar/lofty); cover-less
//! MP4-family videos extract a keyframe via pure-Rust demux + H.264 decode
//! (`video.rs`). Other containers/codec combinations have no pure-Rust

pub mod admission;
pub mod decode;
pub mod encode;
pub mod exif;
pub mod lru;
pub mod pjpeg;
pub mod scale;
pub mod sniff;
pub mod video;
mod workers;

use anyhow::{Result, bail};
use bytes::Bytes;
use std::sync::LazyLock;
use tokio::io::AsyncReadExt;

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

/// Outcome of a thumbnail request.
#[derive(Debug)]
pub enum ThumbOutcome {
    /// Generated (or cache-hit) JPEG bytes, `image/jpeg`.
    Generated(Bytes),
    /// Source bytes served as-is with their natural MIME type.
    Original { data: Bytes, mime: String },
}

/// Everything the engine needs about one request. `mod_unix`/`file_size`
/// come from the stat the HTTP layer already performed.
#[derive(Debug, Clone)]
pub struct ThumbSpec {
    pub path: String,
    pub w: u32,
    pub h: u32,
    pub quality: u8,
    pub mod_unix: i64,
    pub file_size: i64,
    pub center_crop: bool,
    pub modified_nanos: u32,
}

impl ThumbSpec {
    /// Clamp hostile query params into a sane request.
    pub fn sanitize(path: &str, w: i32, h: i32, quality: i32, mod_unix: i64, size: i64) -> Self {
        ThumbSpec {
            path: path.to_string(),
            w: w.clamp(0, MAX_TARGET_DIM as i32) as u32,
            h: h.clamp(0, MAX_TARGET_DIM as i32) as u32,
            quality: quality.clamp(1, 100) as u8,
            mod_unix,
            file_size: size,
            center_crop: false,
            modified_nanos: 0,
        }
    }
}

static LOCKS: LazyLock<crate::utils::keyed_locks::KeyedLocks> =
    LazyLock::new(crate::utils::keyed_locks::KeyedLocks::new);

/// Test hook: cold generations keyed by cache path (incremented once per
/// actual decode+encode, not per request). Keyed so parallel tests cannot
/// pollute each other's coalescing assertions.
#[cfg(test)]
pub(crate) static GENERATIONS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, u64>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(test)]
pub(crate) fn generations_for(key: &str) -> u64 {
    *GENERATIONS.lock().unwrap().get(key).unwrap_or(&0)
}

#[cfg(test)]
fn bump_generations(key: &str) {
    *GENERATIONS
        .lock()
        .unwrap()
        .entry(key.to_string())
        .or_insert(0) += 1;
}

pub fn original_fits(
    width: u32,
    height: u32,
    target_width: u32,
    target_height: u32,
    center_crop: bool,
    bytes: i64,
) -> bool {
    u64::try_from(bytes).is_ok_and(|size| size <= PASSTHROUGH_MAX_BYTES)
        && (target_width == 0 || width <= target_width)
        && (target_height == 0 || height <= target_height)
        && (!center_crop
            || u64::from(width) * u64::from(target_height)
                == u64::from(height) * u64::from(target_width))
}

pub fn read_original_thumbnail(
    path: &std::path::Path,
    width: u32,
    height: u32,
    center_crop: bool,
) -> std::io::Result<Option<Vec<u8>>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    if size > PASSTHROUGH_MAX_BYTES {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(PASSTHROUGH_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > PASSTHROUGH_MAX_BYTES {
        return Ok(None);
    }
    let Ok(sniffed) = sniff::sniff_header(&bytes) else {
        return Ok(None);
    };
    if !sniffed.kind.decodable() {
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
    if !original_fits(
        source_width,
        source_height,
        width,
        height,
        center_crop,
        size as i64,
    ) {
        return Ok(None);
    }
    Ok(Some(bytes))
}

pub async fn get_thumbnail(spec: ThumbSpec, cache: &std::path::Path) -> Result<ThumbOutcome> {
    if spec.center_crop && (spec.w == 0 || spec.h == 0) {
        bail!("positive crop dimensions required");
    }
    let versioned = cache.join("thumbs").join(format!(
        "render-v2-{}-{}",
        spec.center_crop, spec.modified_nanos
    ));
    let cache = if spec.center_crop || spec.modified_nanos != 0 {
        versioned.as_path()
    } else {
        cache
    };
    let head = read_head(&spec.path).await;
    let sniffed = sniff::sniff_header(&head)
        .ok()
        .filter(|s| s.kind.decodable());

    match sniffed {
        Some(s) => get_image_thumbnail(&spec, head, s, cache).await,
        None => dispatch_non_image(&spec, cache).await,
    }
}

async fn get_image_thumbnail(
    spec: &ThumbSpec,
    head: Vec<u8>,
    s: Sniffed,
    cache: &std::path::Path,
) -> Result<ThumbOutcome> {
    // EXIF orientation first: the *upright* dimensions decide the target box
    // so rotated photos produce correctly-proportioned thumbnails.
    let orientation = if s.kind == ImageKind::Jpeg {
        exif::orientation(&head).unwrap_or(1)
    } else {
        1
    };
    let (uw, uh) = if Orientation(orientation).swaps_axes() {
        (s.height, s.width)
    } else {
        (s.width, s.height)
    };
    let (tw, th) = target_size(uw, uh, spec.w, spec.h, spec.center_crop);

    // Passthrough: already thumbnail-sized, browser-renderable, small.
    if original_fits(uw, uh, spec.w, spec.h, spec.center_crop, spec.file_size) {
        let data = tokio::fs::read(&spec.path).await?;
        return Ok(ThumbOutcome::Original {
            data: Bytes::from(data),
            mime: s.kind.mime().to_string(),
        });
    }

    let cache_path = thumb_cache_path(
        cache,
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    let key = cache_path.to_string_lossy().into_owned();

    // Hot LRU, then file cache.
    if let Some(d) = lru::global().get(&cache_path) {
        return Ok(ThumbOutcome::Generated(d));
    }
    if let Some(d) = read_cache_file(&cache_path).await {
        lru::global().put(cache_path.clone(), d.clone());
        return Ok(ThumbOutcome::Generated(d));
    }

    // Single-flight: identical in-flight requests wait for one generation.
    LOCKS
        .with_lock(key, async {
            if let Some(d) = lru::global().get(&cache_path) {
                return Ok(ThumbOutcome::Generated(d));
            }
            if let Some(d) = read_cache_file(&cache_path).await {
                lru::global().put(cache_path.clone(), d.clone());
                return Ok(ThumbOutcome::Generated(d));
            }
            generate_and_cache(spec, s, orientation, tw, th, cache_path, head).await
        })
        .await
}

async fn generate_and_cache(
    spec: &ThumbSpec,
    s: Sniffed,
    orientation: u8,
    tw: u32,
    th: u32,
    cache_path: std::path::PathBuf,
    head: Vec<u8>,
) -> Result<ThumbOutcome> {
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
    let preview = crate::utils::image_metadata::thumbnail(&head).and_then(|bytes| {
        let dimensions = sniff::sniff_header(bytes).ok()?;
        (dimensions.kind == ImageKind::Jpeg && dimensions.width >= rw && dimensions.height >= rh)
            .then(|| (bytes.to_vec(), dimensions))
    });
    let est = estimated_decoded_bytes(s, rw, rh).max(estimated_decoded_bytes(
        preview.as_ref().map_or(s, |(_, dimensions)| *dimensions),
        rw,
        rh,
    ));
    let est = est
        .saturating_mul(2)
        .saturating_add(u64::from(spec.w) * u64::from(spec.h) * 4);
    let est = est.saturating_add(dc_input_bytes(s, rw, rh, spec.file_size));

    let path = spec.path.clone();
    let quality = spec.quality;
    let cp = cache_path.clone();
    let crop = spec.center_crop.then_some(if swaps {
        (spec.h, spec.w)
    } else {
        (spec.w, spec.h)
    });
    let jpg = admission::global()
        .run(est, move || -> Result<Vec<u8>> {
            let preview = preview.as_ref().map(|(bytes, _)| bytes.as_slice());
            let bm = decode_to_target(&path, &head, s, rw, rh, crop, preview)?;
            let (w, h) = crop.unwrap_or((rw, rh));
            let bm = scale::resize_to(bm, w, h);
            let bm = bm.apply_orientation(Orientation(orientation));
            let jpg = encode::encode_jpeg(bm, quality)?;
            let _ = write_thumb_cache(&cp, &jpg);
            Ok(jpg)
        })
        .await??;

    #[cfg(test)]
    {
        let key = cache_path.to_string_lossy().into_owned();
        bump_generations(&key);
    }
    let data = Bytes::from(jpg);
    lru::global().put(cache_path, data.clone());
    Ok(ThumbOutcome::Generated(data))
}

fn decode_to_target(
    path: &str,
    head: &[u8],
    s: Sniffed,
    rw: u32,
    rh: u32,
    crop: Option<(u32, u32)>,
    preview: Option<&[u8]>,
) -> Result<Bitmap> {
    if let Some(bytes) = preview
        && let Ok(bitmap) = decode::jpeg_scaled_bytes(bytes, rw, rh)
    {
        return Ok(scale::crop_for_fill(bitmap, crop));
    }
    let p = std::path::Path::new(path);
    if s.kind == ImageKind::Jpeg
        && dc_fast_eligible(s, rw, rh)
        && let Ok(bm) = pjpeg::decode_thumbnail(head, p, crop)
    {
        return Ok(bm);
    }
    let bm = if s.kind == ImageKind::Jpeg && jpeg_use_scaled(s.width, s.height, rw, rh) {
        decode::jpeg_scaled(p, rw, rh).or_else(|_| decode::image_full(p))?
    } else {
        decode::image_full(p)?
    };
    Ok(scale::crop_for_fill(bm, crop))
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

async fn dispatch_non_image(spec: &ThumbSpec, cache: &std::path::Path) -> Result<ThumbOutcome> {
    let cache_path = thumb_cache_path(
        cache,
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    let key = cache_path.to_string_lossy().into_owned();

    if let Some(d) = lru::global().get(&cache_path) {
        return Ok(ThumbOutcome::Generated(d));
    }
    if let Some(d) = read_cache_file(&cache_path).await {
        lru::global().put(cache_path.clone(), d.clone());
        return Ok(ThumbOutcome::Generated(d));
    }

    LOCKS
        .with_lock(key, async {
            if let Some(d) = lru::global().get(&cache_path) {
                return Ok(ThumbOutcome::Generated(d));
            }
            if let Some(d) = read_cache_file(&cache_path).await {
                lru::global().put(cache_path.clone(), d.clone());
                return Ok(ThumbOutcome::Generated(d));
            }
            generate_non_image(spec, &cache_path).await
        })
        .await
}

async fn generate_non_image(
    spec: &ThumbSpec,
    cache_path: &std::path::Path,
) -> Result<ThumbOutcome> {
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
            let cache_path = cache_path.to_owned();
            return admission::global()
                .run(est, move || {
                    let jpg = encode_bitmap(decode::image_from_bytes(&cover.bytes)?, &request)?;
                    finish_generated(&cache_path, jpg)
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
        let request = spec.clone();
        let cache_path = cache_path.to_owned();
        return admission::global()
            .run(est, move || {
                let jpg = encode_bitmap(video::decode_plan(plan)?, &request)?;
                finish_generated(&cache_path, jpg)
            })
            .await?;
    }

    bail!("unsupported file type for thumbnail")
}

/// Cache + return a generated non-image thumbnail (cover art / video frame).
fn finish_generated(cache_path: &std::path::Path, jpg: Vec<u8>) -> Result<ThumbOutcome> {
    let data = Bytes::from(jpg);
    let _ = write_thumb_cache(cache_path, &data);
    lru::global().put(cache_path.to_path_buf(), data.clone());
    #[cfg(test)]
    {
        let key = cache_path.to_string_lossy().into_owned();
        bump_generations(&key);
    }
    Ok(ThumbOutcome::Generated(data))
}

fn encode_bitmap(bm: Bitmap, spec: &ThumbSpec) -> Result<Vec<u8>> {
    let bm = if spec.center_crop {
        scale::resize_to_fill(bm, spec.w, spec.h)
    } else {
        let (w, h) = compute_target_size(bm.width(), bm.height(), spec.w, spec.h);
        scale::resize_to(bm, w, h)
    };
    encode::encode_jpeg(bm, spec.quality)
}

fn target_size(sw: u32, sh: u32, w: u32, h: u32, crop: bool) -> (u32, u32) {
    if !crop || w == 0 || h == 0 {
        return compute_target_size(sw, sh, w, h);
    }
    if u64::from(w) * u64::from(sh) >= u64::from(h) * u64::from(sw) {
        (
            w.min(sw),
            (u64::from(sh) * u64::from(w))
                .div_ceil(u64::from(sw))
                .min(u64::from(sh)) as u32,
        )
    } else {
        (
            (u64::from(sw) * u64::from(h))
                .div_ceil(u64::from(sh))
                .min(u64::from(sw)) as u32,
            h.min(sh),
        )
    }
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

// ---------------------------------------------------------------------------
// Cache plumbing (deterministic path, atomic write, ETag)
// ---------------------------------------------------------------------------

/// Compute the deterministic cache file path for a thumbnail.
///
/// Layout: `{cache_dir}/thumbs/XX/<sha512>.jpg` where XX = first 2 hex chars
/// (256 shard directories). The hash covers source_path|w|h|q|mtime|size so
/// any source change produces a new path: file existence == valid cache.
pub fn thumb_cache_path(
    cache_dir: &std::path::Path,
    source_path: &str,
    w: u32,
    h: u32,
    quality: u8,
    mod_unix: i64,
    file_size: i64,
) -> std::path::PathBuf {
    let hex = crate::crypto::sha512_hex(&format!(
        "{source_path}|jpg1|{w}x{h}|q{quality}|m{mod_unix}|s{file_size}"
    ));
    cache_dir
        .join("thumbs")
        .join(&hex[..2])
        .join(format!("{hex}.jpg"))
}

/// Strong ETag for a thumbnail response. Derives from the cache filename,
/// which already hashes every invalidation input — computable before any
/// generation work, so conditional requests cost a stat only.
pub fn cache_etag(cache_path: &std::path::Path) -> String {
    let hex = cache_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .strip_suffix(".jpg")
        .unwrap_or_default();
    format!("\"t{hex}\"")
}

/// Write thumbnail bytes to the cache file atomically (temp + rename).
pub fn write_thumb_cache(cache_path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let dir = cache_path.parent().unwrap_or(std::path::Path::new("."));
    let mut tmp = tempfile::Builder::new().suffix(".tmp").tempfile_in(dir)?;
    tmp.write_all(data)?;
    tmp.persist(cache_path)?;
    Ok(())
}

async fn read_cache_file(cache_path: &std::path::Path) -> Option<Bytes> {
    match tokio::fs::read(cache_path).await {
        Ok(d) if !d.is_empty() => Some(Bytes::from(d)),
        _ => None,
    }
}

async fn read_head(path: &str) -> Vec<u8> {
    match tokio::fs::File::open(path).await {
        Ok(mut f) => {
            let mut buf = vec![0u8; HEAD_READ];
            match f.read(&mut buf).await {
                Ok(n) => {
                    buf.truncate(n);
                    buf
                }
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    }
}

/// Compute target dimensions preserving aspect ratio (no upscaling), based
/// on the *upright* source dimensions.
pub fn compute_target_size(src_w: u32, src_h: u32, w: u32, h: u32) -> (u32, u32) {
    let mut tw = src_w;
    let mut th = src_h;
    if w > 0 && h > 0 {
        let rw = f64::from(w) / f64::from(src_w);
        let rh = f64::from(h) / f64::from(src_h);
        let r = rw.min(rh);
        if r < 1.0 {
            tw = (f64::from(src_w) * r) as u32;
            th = (f64::from(src_h) * r) as u32;
        }
    } else if w > 0 && w < src_w {
        let r = f64::from(w) / f64::from(src_w);
        tw = w;
        th = (f64::from(src_h) * r) as u32;
    } else if h > 0 && h < src_h {
        let r = f64::from(h) / f64::from(src_h);
        th = h;
        tw = (f64::from(src_w) * r) as u32;
    }
    (tw.max(1), th.max(1))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Settings {
    pub mem_budget_mb: i64,
    pub lru_mb: i64,
}

pub fn initialize(settings: Settings) {
    admission::initialize(settings.mem_budget_mb);
    lru::initialize(settings.lru_mb);
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/mod.rs"]
mod tests;

pub async fn render(
    path: &str,
    width: u32,
    height: u32,
    center_crop: bool,
    cache: &std::path::Path,
) -> Result<Option<Bytes>, String> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|error| error.to_string())?;
    let modified = meta
        .modified()
        .map_err(|error| error.to_string())?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let mut spec = ThumbSpec::sanitize(
        path,
        i32::try_from(width).map_err(|error| error.to_string())?,
        i32::try_from(height).map_err(|error| error.to_string())?,
        85,
        modified,
        meta.len() as i64,
    );
    spec.center_crop = center_crop;
    spec.modified_nanos = meta
        .modified()
        .map_err(|error| error.to_string())?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let outcome = get_thumbnail(spec, cache)
        .await
        .map_err(|error| error.to_string())?;
    let bytes = match outcome {
        ThumbOutcome::Generated(bytes) => bytes,
        ThumbOutcome::Original { data, .. } => data,
    };
    Ok(Some(bytes))
}
