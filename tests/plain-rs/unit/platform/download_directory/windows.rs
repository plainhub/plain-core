use super::*;
use std::os::windows::ffi::OsStrExt;

#[test]
fn known_folder_results_keep_redirected_paths_and_non_unicode_windows_names() {
    let wide = r"R:\relocated-downloads"
        .encode_utf16()
        .chain([0])
        .collect::<Vec<_>>();
    assert_eq!(
        unsafe { result_path(0, wide.as_ptr()) },
        Some(PathBuf::from(r"R:\relocated-downloads"))
    );
    let wide = [0xd800, 0];
    assert_eq!(
        unsafe { result_path(0, wide.as_ptr()) }
            .unwrap()
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>(),
        vec![0xd800]
    );
}

#[test]
fn unsuccessful_or_null_known_folder_results_are_never_dereferenced() {
    assert!(unsafe { result_path(-1, std::ptr::NonNull::<u16>::dangling().as_ptr()) }.is_none());
    assert!(unsafe { result_path(0, std::ptr::null()) }.is_none());
}

#[test]
fn shell_download_directory_is_absolute_when_available() {
    assert!(resolve().is_none_or(|path| path.is_absolute()));
}
