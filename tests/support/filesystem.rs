pub fn public_directory() -> tempfile::TempDir {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .expect("Public fixture home directory required");
    tempfile::Builder::new()
        .prefix("plain-test-files-")
        .tempdir_in(home)
        .unwrap()
}
