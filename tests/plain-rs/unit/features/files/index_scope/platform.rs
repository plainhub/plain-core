use super::*;

#[test]
fn profile_rules_preserve_exception_ancestors_for_every_profile() {
    let rule = ProfileDir {
        profiles_dir: "/profiles/",
        excluded_dir: "AppData",
        except_dirs: &["AppData/Documents"],
    };
    for profile in ["first", "second"] {
        assert!(rule.excludes(&format!("/profiles/{profile}/AppData/Cache/a.jpg")));
        for relative in [
            "AppData",
            "AppData/Documents",
            "AppData/Documents/report.pdf",
            "AppData-backup/a.jpg",
        ] {
            assert!(
                !rule.excludes(&format!("/profiles/{profile}/{relative}")),
                "{relative}"
            );
        }
        assert!(rule.excludes(&format!(
            "/profiles/{profile}/AppData/Documents-backup/a.jpg"
        )));
    }
    assert!(!rule.excludes("/profiles-other/first/AppData/a.jpg"));
    assert!(!rule.excludes("/elsewhere/profiles/first/AppData/a.jpg"));
}

#[test]
fn root_matching_preserves_component_boundaries_and_case() {
    for (path, root, expected) in [
        ("/media", "/media", true),
        ("/media/a.jpg", "/media", true),
        ("/media-backup/a.jpg", "/media", false),
        ("/Media/a.jpg", "/media", false),
        ("/media/🖼.jpg", "/media", true),
        ("/medía/a.jpg", "/media", false),
        ("/media/a.jpg", "/media/", false),
        ("/media//a.jpg", "/media/", true),
    ] {
        assert_eq!(under_root(path, root), expected, "{path} {root}");
    }
    assert!(under_root_case_insensitive("C:/MEDIA/a.jpg", "c:/media"));
    assert!(!under_root_case_insensitive(
        "C:/MEDIA-backup/a.jpg",
        "c:/media"
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn media_suffixes_keep_ascii_case_and_unicode_boundary_behavior() {
    for name in [
        "Photos.photoslibrary",
        "Photos.PHOTOSLIBRARY",
        "tool.APP",
        "🖼.bundle",
        "x.framework",
    ] {
        assert!(media_package_component(name), "{name}");
    }
    for name in [
        "Photos.photoslibrary-backup",
        "tool.app.txt",
        "framework",
        "é",
        "🖼",
        "",
    ] {
        assert!(!media_package_component(name), "{name}");
    }
}

#[test]
fn index_prefix_matching_preserves_roots_unicode_and_directory_boundaries() {
    for (path, root, expected) in [
        ("/", "/", true),
        ("/media/a.jpg", "/", true),
        ("/media", "/media", true),
        ("/media/a.jpg", "/media/", true),
        ("/media", "/media/", false),
        ("/media-backup/a.jpg", "/media", false),
        ("/Media/a.jpg", "/media", cfg!(windows)),
        ("/médi/a.jpg", "/médi", true),
        ("/médi/a.jpg", "/méd", false),
        ("/🖼/a.jpg", "/🖼", true),
        ("/🖼-backup/a.jpg", "/🖼", false),
    ] {
        assert_eq!(
            super::super::paths::under(path, root),
            expected,
            "{path} {root}"
        );
    }
}

#[test]
#[ignore]
fn file_index_rule_matching_benchmark() {
    let defaults = super::super::rules::defaults();
    let paths = [
        "/DATA/Gallery/a.jpg",
        "/Volumes/Media/Photos.PHOTOSLIBRARY/originals/a.jpg",
        "/usr/share/a.jpg",
        "/Users/first/Library/Caches/a.jpg",
        "/Users/second/Library/Mobile Documents/com~apple~CloudDocs/report.pdf",
        "/DATA/project/node_modules/image.png",
        "/Volumes/Media/🖼.APP/image.jpg",
    ];
    let start = std::time::Instant::now();
    let mut matched = 0;
    for _ in 0..100_000 {
        for path in paths {
            matched += usize::from(std::hint::black_box(
                defaults.excluded(std::hint::black_box(path), false),
            ));
        }
    }
    println!(
        "paths=700000 matched={matched} elapsed_ns={}",
        start.elapsed().as_nanos()
    );
}
