use super::*;

fn running_state() -> (watch::Sender<bool>, watch::Receiver<bool>, AtomicBool) {
    let (sender, receiver) = watch::channel(false);
    (sender, receiver, AtomicBool::new(false))
}

fn archive(path: &Path, name: &str, bytes: &[u8]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    writer
        .start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
    writer.write_all(bytes).unwrap();
    writer.finish().unwrap();
}

#[test]
fn directory_round_trip_preserves_structure_bytes_and_modified_time() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir_all(source.join("empty")).unwrap();
    std::fs::create_dir_all(source.join("nested")).unwrap();
    let file = source.join("nested/note.txt");
    std::fs::write(&file, b"exact archive payload").unwrap();
    File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1700000000))
        .unwrap();
    let (_sender, stop, cancelled) = running_state();
    let (packed, _) = pack_files(
        vec![source.to_string_lossy().into_owned()],
        directory.path().join("packed.zip"),
        &stop,
        &cancelled,
    )
    .unwrap()
    .keep();
    let mut zip = zip::ZipArchive::new(File::open(&packed).unwrap()).unwrap();
    assert!(zip.by_name("source/empty/").unwrap().is_dir());
    assert!(zip.by_name("source/nested/note.txt").unwrap().is_file());
    let (extracted, files) =
        unpack_files(&packed, &directory.path().join("out"), &stop, &cancelled)
            .unwrap()
            .keep();
    assert_eq!(files.len(), 1);
    let file = extracted.join("source/nested/note.txt");
    assert_eq!(std::fs::read(&file).unwrap(), b"exact archive payload");
    assert!(extracted.join("source/empty").is_dir());
    assert_eq!(
        file.metadata()
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        1700000000
    );
}

#[test]
fn archive_destinations_never_overwrite_existing_files_or_directories() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("source.txt");
    std::fs::write(&file, b"source").unwrap();
    let target = directory.path().join("packed.zip");
    std::fs::write(&target, b"keep original").unwrap();
    let (_sender, stop, cancelled) = running_state();
    let (packed, _) = pack_files(
        vec![file.to_string_lossy().into_owned()],
        target.clone(),
        &stop,
        &cancelled,
    )
    .unwrap()
    .keep();
    assert_eq!(packed, directory.path().join("packed (1).zip"));
    assert_eq!(std::fs::read(&target).unwrap(), b"keep original");
    let target = directory.path().join("out");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep"), b"existing").unwrap();
    let (extracted, _) = unpack_files(&packed, &target, &stop, &cancelled)
        .unwrap()
        .keep();
    assert_eq!(extracted, directory.path().join("out (1)"));
    assert_eq!(std::fs::read(target.join("keep")).unwrap(), b"existing");
}

#[test]
fn traversal_is_rejected_before_creating_an_output_directory() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.zip");
    archive(&source, "../escape.txt", b"bad");
    let target = directory.path().join("out");
    let (_sender, stop, cancelled) = running_state();
    assert!(unpack_files(&source, &target, &stop, &cancelled).is_err());
    assert!(!target.exists());
    assert!(!directory.path().join("escape.txt").exists());
    assert_eq!(
        relative("nested/../note.txt").unwrap(),
        PathBuf::from("note.txt")
    );
    assert!(relative("../../escape").is_err());
}

#[test]
fn corrupted_archive_removes_all_new_outputs() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.zip");
    archive(&source, "note.txt", b"unique corruptible payload");
    let mut bytes = std::fs::read(&source).unwrap();
    let offset = bytes
        .windows(b"unique corruptible payload".len())
        .position(|value| value == b"unique corruptible payload")
        .unwrap();
    bytes[offset] ^= 1;
    std::fs::write(&source, bytes).unwrap();
    let (_sender, stop, cancelled) = running_state();
    let target = directory.path().join("out");
    assert!(unpack_files(&source, &target, &stop, &cancelled).is_err());
    assert!(!target.exists());
}

#[test]
fn cancellation_during_chunk_copy_removes_the_reserved_file() {
    struct Reader<'a>(&'a AtomicBool);
    impl Read for Reader<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            assert_eq!(buffer.len(), 64 * 1024);
            buffer[..3].copy_from_slice(b"abc");
            self.0.store(true, Ordering::Release);
            Ok(3)
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("partial.zip");
    let (_sender, stop, cancelled) = running_state();
    let (output, mut file) = reserve(&path, false).unwrap();
    assert!(
        copy(
            &mut Reader(&cancelled),
            file.as_mut().unwrap(),
            &stop,
            &cancelled
        )
        .is_err()
    );
    drop(file);
    drop(output);
    assert!(!path.exists());
}

#[test]
fn a_stopped_core_does_not_create_archive_outputs() {
    let directory = tempfile::tempdir().unwrap();
    let (sender, stop, cancelled) = running_state();
    sender.send_replace(true);
    assert!(
        pack_files(
            Vec::new(),
            directory.path().join("out.zip"),
            &stop,
            &cancelled
        )
        .is_err()
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn an_archive_inside_its_source_uses_the_snapshot_before_output_creation() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("note.txt"), b"source bytes").unwrap();
    let target = source.join("packed.zip");
    let (_sender, stop, cancelled) = running_state();
    let (output, _) = pack_files(
        vec![source.to_string_lossy().into_owned()],
        target.clone(),
        &stop,
        &cancelled,
    )
    .unwrap()
    .keep();
    assert_eq!(output, target);
    let mut archive = zip::ZipArchive::new(File::open(output).unwrap()).unwrap();
    assert!(archive.by_name("source/note.txt").is_ok());
    assert!(archive.by_name("source/packed.zip").is_err());
    assert_eq!(archive.len(), 2);
}

#[cfg(unix)]
#[test]
fn links_preserve_the_alias_name_and_can_reference_the_same_target_twice() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    let real = directory.path().join("real");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&real).unwrap();
    std::fs::write(real.join("note.txt"), b"linked bytes").unwrap();
    std::os::unix::fs::symlink(&real, source.join("one")).unwrap();
    std::os::unix::fs::symlink(&real, source.join("two")).unwrap();
    let (_sender, stop, cancelled) = running_state();
    let entries = collect_entries(
        vec![source.to_string_lossy().into_owned()],
        &stop,
        &cancelled,
    )
    .unwrap();
    let paths = entries
        .iter()
        .filter(|entry| !entry.directory)
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], paths[1]);
    let output = pack_entries(
        entries,
        directory.path().join("packed.zip"),
        &stop,
        &cancelled,
    )
    .unwrap();
    let mut archive = zip::ZipArchive::new(File::open(&output.path).unwrap()).unwrap();
    for name in ["source/one/note.txt", "source/two/note.txt"] {
        let mut bytes = Vec::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"linked bytes");
    }
}

#[cfg(unix)]
#[test]
fn a_directory_link_cycle_is_rejected_before_reserving_output() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::os::unix::fs::symlink(&source, source.join("cycle")).unwrap();
    let target = directory.path().join("packed.zip");
    let (_sender, stop, cancelled) = running_state();
    let error = pack_files(
        vec![source.to_string_lossy().into_owned()],
        target.clone(),
        &stop,
        &cancelled,
    )
    .err()
    .unwrap();
    assert!(error.contains("directory cycle"));
    assert!(!target.exists());
}

#[cfg(unix)]
#[test]
fn a_literal_backslash_filename_round_trips_on_filesystems_that_support_it() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("note\\name.txt");
    std::fs::write(&source, b"literal filename").unwrap();
    let (_sender, stop, cancelled) = running_state();
    let (packed, _) = pack_files(
        vec![source.to_string_lossy().into_owned()],
        directory.path().join("packed.zip"),
        &stop,
        &cancelled,
    )
    .unwrap()
    .keep();
    let (output, _) = unpack_files(&packed, &directory.path().join("out"), &stop, &cancelled)
        .unwrap()
        .keep();
    assert_eq!(
        std::fs::read(output.join("note\\name.txt")).unwrap(),
        b"literal filename"
    );
}

#[tokio::test]
async fn named_internal_archives_reject_invalid_aliases_and_stop_before_creating_output() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("log");
    std::fs::write(&source, b"log bytes").unwrap();
    let output = directory.path().join("output/logs.zip");
    let (_stop, stopped) = watch::channel(false);
    for name in ["", "../escape", "/absolute", "a/../escape", "a\\escape"] {
        assert!(
            pack_named_files(
                vec![(source.clone(), name.into())],
                output.clone(),
                stopped.clone()
            )
            .await
            .is_err()
        );
        assert!(!output.parent().unwrap().exists());
    }
    assert!(
        pack_named_files(
            vec![
                (source.clone(), "log".into()),
                (source.clone(), "log".into())
            ],
            output.clone(),
            stopped
        )
        .await
        .is_err()
    );
    let (stop, stopped) = watch::channel(false);
    stop.send_replace(true);
    assert!(
        pack_named_files(vec![(source, "log".into())], output.clone(), stopped)
            .await
            .is_err()
    );
    assert!(!output.parent().unwrap().exists());
}

#[test]
fn special_entries_are_rejected_before_output_reservation() {
    for mode in [0o120777u32, 0o010644, 0o020600, 0o060600, 0o140600] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.zip");
        archive(&source, "entry", b"../neighbor.txt");
        let mut bytes = std::fs::read(&source).unwrap();
        let central = bytes
            .windows(4)
            .position(|value| value == b"PK\x01\x02")
            .unwrap();
        bytes[central + 5] = 3;
        bytes[central + 38..central + 42].copy_from_slice(&(mode << 16).to_le_bytes());
        std::fs::write(&source, bytes).unwrap();
        let neighbor = directory.path().join("neighbor.txt");
        std::fs::write(&neighbor, b"preserved neighbor").unwrap();
        let target = directory.path().join("out");
        let (_sender, stop, cancelled) = running_state();
        let result = unpack_files(&source, &target, &stop, &cancelled);
        assert!(
            matches!(result, Err(ref error) if error == "Unsupported archive entry type"),
            "mode {mode:o}"
        );
        assert!(!target.exists());
        assert_eq!(std::fs::read(neighbor).unwrap(), b"preserved neighbor");
    }
}

#[cfg(unix)]
#[test]
fn extraction_does_not_restore_privileged_archive_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.zip");
    archive(&source, "entry", b"plain file");
    let mut bytes = std::fs::read(&source).unwrap();
    let central = bytes
        .windows(4)
        .position(|value| value == b"PK\x01\x02")
        .unwrap();
    bytes[central + 5] = 3;
    bytes[central + 38..central + 42].copy_from_slice(&(0o107777u32 << 16).to_le_bytes());
    std::fs::write(&source, bytes).unwrap();
    let (_sender, stop, cancelled) = running_state();
    let (output, _) = unpack_files(&source, &directory.path().join("out"), &stop, &cancelled)
        .unwrap()
        .keep();
    let file = output.join("entry");
    assert_eq!(std::fs::read(&file).unwrap(), b"plain file");
    assert_eq!(file.metadata().unwrap().permissions().mode() & 0o7000, 0);
}
