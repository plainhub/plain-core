use super::*;

#[test]
fn local_file_uris_decode_once_and_preserve_literal_plus_signs() {
    assert_eq!(
        local_path("file:///tmp/a%20b+c%2B%2520.txt").unwrap(),
        "/tmp/a b+c+%20.txt"
    );
    assert_eq!(
        local_path("FILE://LOCALHOST/tmp/file.txt").unwrap(),
        "/tmp/file.txt"
    );
    assert_eq!(
        local_path("content://provider/file%20name").unwrap(),
        "content://provider/file%20name"
    );
    assert_eq!(
        local_path("/tmp/literal%20path").unwrap(),
        "/tmp/literal%20path"
    );
    assert!(local_path("file://remote-host/private").is_err());
}
#[cfg(windows)]
#[test]
fn drive_letter_uris_resolve_to_absolute_windows_paths() {
    let path = local_path("file:///C:/data/file%20name.txt").unwrap();
    assert_eq!(path, "C:/data/file name.txt");
    assert!(std::path::Path::new(&path).is_absolute());
}
