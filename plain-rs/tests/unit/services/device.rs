use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn raw_battery_readings_preserve_unknown_and_percent_semantics() {
    let sample = |level, scale| BatterySample {
        level,
        scale,
        charging: false,
        temperature_tenths: None,
    };
    assert_eq!(sample(Some(3.0), Some(4.0)).percent(), Some(75));
    assert_eq!(sample(Some(0.729), Some(1.0)).percent(), Some(72));
    assert_eq!(sample(Some(200.0), Some(100.0)).percent(), Some(100));
    for reading in [
        sample(None, Some(100.0)),
        sample(Some(-1.0), Some(100.0)),
        sample(Some(1.0), None),
        sample(Some(1.0), Some(0.0)),
        sample(Some(f64::NAN), Some(1.0)),
        sample(Some(1.0), Some(f64::INFINITY)),
    ] {
        assert_eq!(reading.percent(), None);
    }
}

#[test]
fn direct_system_reads_are_selected_without_an_sdk_battery_exchange() {
    assert_eq!(
        Selection {
            battery: true,
            ..Selection::default()
        }
        .direct_fields(),
        !cfg!(any(target_os = "android", target_os = "ios"))
    );
    assert!(!Selection::default().direct_fields());
    assert!(
        !Selection {
            cpu: true,
            ..Selection::default()
        }
        .direct_fields()
    );
    assert!(
        Selection {
            storage: true,
            ..Selection::default()
        }
        .direct_fields()
    );
    assert!(
        Selection {
            uptime: true,
            ..Selection::default()
        }
        .direct_fields()
    );
}

#[tokio::test]
async fn overlapping_cpu_requests_reuse_the_sample_even_if_the_first_caller_cancels() {
    let status = Arc::new(Status::default());
    let requested = Instant::now();
    let count = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(tokio::sync::Notify::new());
    let (release, released) = std::sync::mpsc::channel();
    let first = {
        let status = status.clone();
        let count = count.clone();
        let entered = entered.clone();
        tokio::spawn(async move {
            status
                .sample_cpu(requested, move || {
                    count.fetch_add(1, Ordering::SeqCst);
                    entered.notify_one();
                    released.recv().unwrap();
                    42.0
                })
                .await
        })
    };
    entered.notified().await;
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    release.send(()).unwrap();
    let count_again = count.clone();
    assert_eq!(
        status
            .sample_cpu(requested, move || {
                count_again.fetch_add(1, Ordering::SeqCst);
                100.0
            })
            .await
            .unwrap(),
        42.0
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);

    let count_again = count.clone();
    assert_eq!(
        status
            .sample_cpu(Instant::now(), move || {
                count_again.fetch_add(1, Ordering::SeqCst);
                75.0
            })
            .await
            .unwrap(),
        75.0
    );
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_battery_selection_requires_no_host_and_no_cpu_sample() {
    let status = Status::default();
    let sample = status
        .collect(
            &Host::default(),
            Selection {
                battery: true,
                ..Selection::default()
            },
        )
        .await
        .unwrap();
    if let Some(level) = sample.battery_level {
        assert!((0..=100).contains(&level));
    } else {
        assert!(!sample.charging);
    }
    assert!(status.cpu.lock().await.is_none());
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn storage_uses_the_owned_data_volume_and_never_falls_back_to_other_disks() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing-volume");
    let host = Host::default();
    let status = Status::new(missing.clone(), &host);
    assert!(
        status
            .collect(
                &host,
                Selection {
                    storage: true,
                    ..Selection::default()
                }
            )
            .await
            .is_err()
    );
    assert!(status.collect(&host, Selection::default()).await.is_ok());
    assert!(
        super::info::collect(
            &host,
            missing,
            vec!["totalStorage".into()],
            status.fixed_info().unwrap()
        )
        .await
        .is_err()
    );
    assert!(storage_bytes(directory.path(), true).unwrap() > 0);
}

#[test]
fn data_volume_selection_uses_the_longest_component_prefix_and_never_an_unrelated_disk() {
    use std::path::Path;
    let mounts = [
        Path::new("/"),
        Path::new("/Volumes/Data"),
        Path::new("/Volumes/Data/projects"),
    ];
    for (path, expected) in [
        ("/Volumes/Data/projects/a/b", 2),
        ("/Volumes/Data/file", 1),
        ("/Volumes/Database", 0),
        ("/etc/hosts", 0),
    ] {
        assert_eq!(
            volume_index(mounts.into_iter(), Path::new(path)),
            Some(expected)
        );
    }
    assert_eq!(
        volume_index(
            [Path::new("/Volumes/Data")].into_iter(),
            Path::new("/other/file")
        ),
        None
    );
    assert_eq!(
        volume_index(std::iter::empty(), Path::new("/other/file")),
        None
    );
}

#[cfg(not(target_os = "ios"))]
#[tokio::test]
async fn available_memory_is_read_without_a_connected_host() {
    let selection = Selection {
        memory: true,
        ..Selection::default()
    };
    assert!(selection.direct_fields());
    let status = Status::default();
    let sample = status.collect(&Host::default(), selection).await.unwrap();
    assert!(sample.memory_available.is_some());
    assert!(status.cpu.lock().await.is_none());
}
