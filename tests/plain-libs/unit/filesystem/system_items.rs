use super::*;

#[test]
fn unix_system_names_only_group_the_actual_root_entries() {
    assert!(UNIX_DIRECTORIES.contains(&"/usr"));
    assert!(UNIX_DIRECTORIES.contains(&"/bin.usr-is-merged"));
    for path in [
        "/home",
        "/media",
        "/mnt",
        "/data",
        "/home/me/usr",
        "/usr/bin",
        "/backup/etc",
    ] {
        assert!(!UNIX_DIRECTORIES.contains(&path));
    }
}

#[test]
fn android_groups_only_system_roots_and_shared_storage_app_directory() {
    for path in [
        "/system",
        "/data",
        "/storage/emulated/0/Android",
        "/storage/emulated/10/Android",
        "/storage/ABCD-1234/Android",
        "/sdcard/Android",
        "/mnt/sdcard/Android",
    ] {
        assert!(android(path), "{path}");
    }
    for path in [
        "/storage/emulated/0/DCIM",
        "/storage/emulated/0/Documents/Android",
        "/storage/emulated/0/Android/data",
        "/storage/emulated/0/system",
        "/storage/emulated/me/Android",
    ] {
        assert!(!android(path), "{path}");
    }
}

#[test]
fn windows_uses_redirected_known_paths_and_does_not_group_descendants_or_other_drives() {
    let directories = vec![
        "R:/OS".into(),
        "R:/Programs".into(),
        "S:/Application Data".into(),
    ];
    assert!(windows(r"r:\os", &directories));
    assert!(windows("S:/application data", &directories));
    for path in [
        "C:/Windows",
        "D:/Programs",
        "R:/Programs/MyApp",
        "R:/Users",
        "R:/OS-copy",
    ] {
        assert!(!windows(path, &directories), "{path}");
    }
}

#[test]
fn swap_files_only_group_registered_file_backed_swap_and_decode_kernel_paths() {
    assert_eq!(
        swap_files(
            "Filename Type Size Used Priority\n/swap.img file 123 0 -2\n/dev/sda2 partition 123 0 -3\n/swap\\040file file 123 0 -4\n"
        ),
        vec!["/swap.img", "/swap file"]
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_classifies_system_roots_through_the_public_predicate() {
    for path in MACOS_DIRECTORIES {
        assert!(contains(path).unwrap(), "{path}");
    }
    assert!(!contains("/Users").unwrap());
}

#[test]
fn macos_groups_only_the_three_system_root_entries() {
    for path in ["/Applications", "/Library", "/System"] {
        assert!(MACOS_DIRECTORIES.contains(&path), "{path}");
    }
    for path in [
        "/Users",
        "/Volumes",
        "/usr",
        "/Users/person/Applications",
        "/Users/person/Library",
        "/System/Library",
        "/Library/Application Support",
        "/Applications/Utilities",
        "/System-copy",
    ] {
        assert!(!MACOS_DIRECTORIES.contains(&path), "{path}");
    }
}
