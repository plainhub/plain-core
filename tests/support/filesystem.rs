pub fn public_directory() -> plain_libs::temporary::TempDir {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .expect("Public fixture home directory required");
    plain_libs::temporary::Builder::new()
        .prefix("plain-test-files-")
        .tempdir_in(home)
        .unwrap()
}
