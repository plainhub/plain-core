#[test]
fn file_index_contract_uses_volume_controls_and_one_configuration_input() {
    let sdl = super::sdl();
    for field in [
        "rebuildFileIndex(volumeId: ID): Boolean!",
        "pauseFileIndex(volumeId: ID): Boolean!",
        "resumeFileIndex(volumeId: ID): Boolean!",
        "stopFileIndex(volumeId: ID): Boolean!",
        "fileIndexScanProgress(volumeId: ID): FileIndexScanProgress!",
        "fileIndexScanProgressUpdated: FileIndexScanProgress!",
        "setFileIndexVolume(input: FileIndexVolumeInput!): Boolean!",
        "state: FileIndexScanState!",
        "lastScanAt: Instant",
        "lastScanStatus: FileIndexScanStatus",
        "lastScanCount: Long",
        "lastScanTotal: Long",
    ] {
        assert!(sdl.contains(field), "missing {field}");
    }
    for retired in [
        "pauseFileIndexScan",
        "resumeFileIndexScan",
        "stopFileIndexScan",
        "rebuildMediaIndex(",
        "rebuildFileIndexVolume(",
        "pauseMediaScan:",
        "resumeMediaScan:",
        "stopMediaScan:",
        "scanProgress:",
        "mediaScanProgress:",
        "type ScanProgress {",
        "enum ScanState {",
        "enum IndexMode {",
    ] {
        assert!(!sdl.contains(retired), "retained {retired}");
    }
    for (name, expected) in [
        (
            "FileIndexScanStatus",
            vec!["COMPLETED", "CANCELED", "FAILED"],
        ),
        (
            "FileIndexMode",
            vec!["UNCONFIGURED", "DISABLED", "ALLOWLIST", "BLOCKLIST"],
        ),
        (
            "FileIndexScanState",
            vec!["IDLE", "QUEUED", "RUNNING", "PAUSED", "STOPPED", "FAILED"],
        ),
    ] {
        let values = sdl
            .split(&format!("enum {name} {{"))
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        assert_eq!(values, expected, "{name}");
    }
}

#[test]
fn volume_scan_fields_are_nullable_with_named_scalar_and_enum_types() {
    let sdl = super::sdl();
    let fields = sdl
        .split("type FileIndexVolume {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    for field in [
        "lastScanAt: Instant",
        "lastScanStatus: FileIndexScanStatus",
        "lastScanCount: Long",
        "lastScanTotal: Long",
    ] {
        assert!(
            fields.lines().any(|line| line.trim() == field),
            "wrong type for {field}"
        );
    }
}

#[test]
fn volume_configuration_input_has_one_required_volume_id_and_explicit_lists() {
    let sdl = super::sdl();
    let fields = sdl
        .split("input FileIndexVolumeInput {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    for field in [
        "volumeId: ID!",
        "indexMode: FileIndexMode!",
        "includedDirs: [String!]!",
        "excludedDirs: [String!]!",
    ] {
        assert!(
            fields.lines().any(|line| line.trim() == field),
            "missing {field}"
        );
    }
}
