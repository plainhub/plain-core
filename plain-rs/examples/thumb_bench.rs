//! Compare the production thumbnail pipeline stages on one local image.

use anyhow::Result;
use plain_rs::media::thumbnails::{compute_target_size, decode, encode, scale, sniff};
use std::{io::Read, path::Path, time::Instant};

fn measure(
    label: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<()>,
) -> Result<()> {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        operation()?;
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    let minimum = samples.iter().copied().fold(f64::INFINITY, f64::min);
    let mean = samples.iter().sum::<f64>() / iterations as f64;
    println!("{label:<28} min={minimum:.3}ms mean={mean:.3}ms");
    Ok(())
}

fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let path = arguments
        .next()
        .ok_or_else(|| anyhow::anyhow!("Usage: thumb_bench IMAGE [TARGET=512] [ITERS=5]"))?;
    let target = arguments
        .next()
        .map(|value| value.parse::<u32>())
        .transpose()?
        .unwrap_or(512);
    let iterations = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(5);
    anyhow::ensure!(
        target > 0 && iterations > 0,
        "Target and iterations must be positive"
    );
    let path = Path::new(&path);
    let mut head = Vec::new();
    std::fs::File::open(path)?
        .take(256 * 1024)
        .read_to_end(&mut head)?;
    let source = sniff::sniff_header(&head)?;
    let (width, height) = compute_target_size(source.width, source.height, target, target);
    println!(
        "{}x{} -> {width}x{height}; {iterations} iterations",
        source.width, source.height
    );
    let _ = decode::image_full(path)?;
    measure("Full image decode", iterations, || {
        std::hint::black_box(decode::image_full(path)?);
        Ok(())
    })?;
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg")
        })
    {
        measure("IDCT-scaled JPEG decode", iterations, || {
            std::hint::black_box(decode::jpeg_scaled(path, width, height)?);
            Ok(())
        })?;
    }
    measure("Decode and resize", iterations, || {
        std::hint::black_box(scale::resize_to(decode::image_full(path)?, width, height));
        Ok(())
    })?;
    measure("Decode, resize and JPEG", iterations, || {
        let bitmap = scale::resize_to(decode::image_full(path)?, width, height);
        std::hint::black_box(encode::encode_jpeg(bitmap, 75)?);
        Ok(())
    })?;
    Ok(())
}
