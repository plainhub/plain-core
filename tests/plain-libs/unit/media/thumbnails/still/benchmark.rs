use super::super::scale;
use super::super::workers::Workers;
use super::*;
use std::{
    path::PathBuf,
    sync::{Arc, Barrier},
    time::Instant,
};

const WORKERS: usize = 4;
const IMAGES: usize = 12;
const ROUNDS: usize = 3;

fn render(path: &Path, kind: ImageKind, target: (u32, u32), thumbnail_path: bool) -> Bitmap {
    let bitmap = if thumbnail_path {
        thumbnail(path, kind, target.0, target.1).unwrap()
    } else {
        match kind {
            ImageKind::Tiff => decode_file(path, kind).unwrap(),
            _ => heif::decode_unoriented(&std::fs::read(path).unwrap()).unwrap(),
        }
    };
    scale::resize_to(bitmap, target.0, target.1)
}

fn batch(
    runtime: &tokio::runtime::Runtime,
    workers: &Arc<Workers>,
    path: &Path,
    kind: ImageKind,
    target: (u32, u32),
    thumbnail_path: bool,
) -> f64 {
    let start = Instant::now();
    runtime.block_on(async {
        let mut jobs = tokio::task::JoinSet::new();
        for _ in 0..IMAGES {
            let workers = workers.clone();
            let path = path.to_path_buf();
            jobs.spawn(async move {
                let bitmap = workers
                    .run(move || render(&path, kind, target, thumbnail_path))
                    .await
                    .unwrap();
                assert_eq!((bitmap.width(), bitmap.height()), target);
                std::hint::black_box(bitmap);
            });
        }
        while let Some(job) = jobs.join_next().await {
            job.unwrap();
        }
    });
    start.elapsed().as_secs_f64() * 1000.0
}

fn warm_workers(runtime: &tokio::runtime::Runtime, workers: &Arc<Workers>) {
    let barrier = Arc::new(Barrier::new(WORKERS));
    runtime.block_on(async {
        let mut jobs = tokio::task::JoinSet::new();
        for _ in 0..WORKERS {
            let workers = workers.clone();
            let barrier = barrier.clone();
            jobs.spawn(async move {
                workers
                    .run(move || {
                        barrier.wait();
                    })
                    .await
                    .unwrap();
            });
        }
        while let Some(job) = jobs.join_next().await {
            job.unwrap();
        }
    });
}

#[test]
#[ignore]
fn still_image_batch_benchmark() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let workers = Arc::new(Workers::new(WORKERS).unwrap());
    warm_workers(&runtime, &workers);
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/still-images");
    for (name, kind, bounds) in [
        ("grid.tiff", ImageKind::Tiff, (256, 256)),
        ("grid.avif", ImageKind::Avif, (256, 256)),
        ("grid.heic", ImageKind::Heif, (256, 256)),
        ("wide.heic", ImageKind::Heif, (256, 256)),
        ("rgb.avif", ImageKind::Avif, (256, 256)),
    ] {
        let path = directory.join(name);
        let dimensions = probe(&path, kind).unwrap();
        let target = super::super::compute_target_size(
            dimensions.width,
            dimensions.height,
            bounds.0,
            bounds.1,
        );
        assert!(target.0 <= dimensions.width && target.1 <= dimensions.height);
        let full = render(&path, kind, target, false);
        let thumbnail = render(&path, kind, target, true);
        assert_eq!(full.raw(), thumbnail.raw(), "{name}");
        let mut baseline = Vec::new();
        let mut optimized = Vec::new();
        for round in 0..ROUNDS {
            if round % 2 == 0 {
                baseline.push(batch(&runtime, &workers, &path, kind, target, false));
                optimized.push(batch(&runtime, &workers, &path, kind, target, true));
            } else {
                optimized.push(batch(&runtime, &workers, &path, kind, target, true));
                baseline.push(batch(&runtime, &workers, &path, kind, target, false));
            }
        }
        baseline.sort_by(f64::total_cmp);
        optimized.sort_by(f64::total_cmp);
        println!(
            "{}",
            crate::serde_json::json!({
                "input": name,
                "bounds": [bounds.0, bounds.1],
                "target": [target.0, target.1],
                "policy": "aspect fit, no crop, no upscaling",
                "workers": WORKERS,
                "workerLifetime": "one persistent pool shared by every baseline and optimized batch",
                "threadCreation": "outside timing",
                "imagesPerBatch": IMAGES,
                "rounds": ROUNDS,
                "cache": "warm OS pages, no thumbnail cache",
                "baseline": "full decode, box filter, aspect fit resample",
                "optimized": if kind==ImageKind::Tiff {"streamed TIFF rows, box filter, aspect fit resample"}else{"shared full decode, box filter, aspect fit resample"},
                "baselineMedianBatchMs": baseline[ROUNDS / 2],
                "optimizedMedianBatchMs": optimized[ROUNDS / 2],
                "speedup": baseline[ROUNDS / 2] / optimized[ROUNDS / 2],
                "pixels": "byte identical before timing"
            })
        );
    }
}
