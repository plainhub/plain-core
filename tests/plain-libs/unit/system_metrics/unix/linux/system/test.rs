use super::*;

#[test]
fn memory_snapshot_uses_kernel_available_bytes_including_zero() {
    let mut system = SystemInner::new();
    system.update_memory("MemTotal: 100 kB\nMemFree: 20 kB\nMemAvailable: 0 kB\nCached: 30 kB\n");
    assert_eq!(system.total_memory(), 100 * 1024);
    assert_eq!(system.available_memory(), 0);
    system.update_memory("MemAvailable: 55 kB\n");
    assert_eq!(system.available_memory(), 55 * 1024);
}

#[test]
fn older_memory_snapshots_preserve_cache_and_shared_memory_accounting() {
    let mut system = SystemInner::new();
    system.update_memory("MemTotal: 100 kB\nMemFree: 10 kB\nBuffers: 2 kB\nCached: 20 kB\nSReclaimable: 3 kB\nShmem: 5 kB\nSwapTotal: 1000 kB\n");
    assert_eq!(system.available_memory(), 30 * 1024);
    system.update_memory("Shmem: 100 kB\n");
    assert_eq!(system.available_memory(), 0);
}

#[test]
fn malformed_memory_values_are_skipped_and_byte_conversion_saturates() {
    let mut system = SystemInner::new();
    system.update_memory("MemTotal: invalid kB\nMemAvailable: -1 kB\nNoSeparator\n");
    assert_eq!(system.total_memory(), 0);
    assert_eq!(system.available_memory(), 0);
    system.update_memory(&format!(
        "MemTotal: {} kB\nMemAvailable: {} kB\n",
        u64::MAX,
        u64::MAX
    ));
    assert_eq!(system.total_memory(), u64::MAX);
    assert_eq!(system.available_memory(), u64::MAX);
}

#[cfg(not(target_os = "android"))]
#[test]
fn release_file_parsing_preserves_version_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let release = directory.path().join("os-release");
    let fallback = directory.path().join("lsb-release");
    std::fs::write(&release, "NAME=Example OS\nVERSION_ID=42\n").unwrap();
    std::fs::write(&fallback, "DISTRIB_RELEASE=41\n").unwrap();
    assert_eq!(
        get_system_info_linux(InfoType::Name, &release, &fallback).as_deref(),
        Some("Example OS")
    );
    assert_eq!(
        get_system_info_linux(InfoType::OsVersion, &release, &fallback).as_deref(),
        Some("42")
    );
    assert_eq!(
        get_system_info_linux(
            InfoType::OsVersion,
            &directory.path().join("absent"),
            &fallback
        )
        .as_deref(),
        Some("41")
    );
}
