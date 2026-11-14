#[test]
fn file_index_contract_uses_one_rebuild_and_explicit_scan_names() {
    let sdl = super::sdl();
    for field in [
        "rebuildFileIndex(volumeId: ID): Boolean!",
        "pauseFileIndexScan: Boolean!",
        "resumeFileIndexScan: Boolean!",
        "stopFileIndexScan: Boolean!",
        "fileIndexScanProgress: FileIndexScanProgress!",
        "fileIndexScanProgressUpdated: FileIndexScanProgress!",
        "setFileIndexVolume(id: ID!, indexMode: FileIndexMode!, includedDirs: [String!]!, excludedDirs: [String!]!): Boolean!",
        "state: FileIndexScanState!",
    ] {
        assert!(sdl.contains(field), "missing {field}");
    }
    for retired in [
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
            "FileIndexMode",
            vec!["UNCONFIGURED", "DISABLED", "ALLOWLIST", "BLOCKLIST"],
        ),
        (
            "FileIndexScanState",
            vec!["IDLE", "RUNNING", "PAUSED", "STOPPED", "FAILED"],
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
