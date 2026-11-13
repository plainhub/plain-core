use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn entry(url: &str, name: &str, is_dir: bool) -> Entry {
    Entry {
        url: url.into(),
        name: name.into(),
        is_dir,
        size_bytes: 0,
    }
}

#[tokio::test]
async fn traversal_and_duplicate_destinations_fail_before_any_transfer_is_planned() {
    for name in ["../escape", ".", "..", "", "a/b", "a\\b", "nul\0x"] {
        assert!(
            build(&["root".into()], |_| async move {
                Ok(Metadata {
                    entry: entry("root", name, false),
                    children: vec![],
                })
            })
            .await
            .is_err()
        );
    }
    let error = build(&["one".into(), "two".into()], |url| async move {
        Ok(Metadata {
            entry: entry(&url, "same", false),
            children: vec![],
        })
    })
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("Duplicate download destination"));
}

#[tokio::test]
async fn recursive_directory_depth_is_bounded_without_repeated_root_metadata_requests() {
    let calls = AtomicUsize::new(0);
    let result = build(&["0".into()], |url| {
        calls.fetch_add(1, Ordering::Relaxed);
        async move {
            let next = url.parse::<usize>().unwrap() + 1;
            Ok(Metadata {
                entry: entry(&url, "child", true),
                children: vec![entry(&next.to_string(), "child", true)],
            })
        }
    })
    .await;
    assert!(result.err().unwrap().to_string().contains("capacity"));
    assert_eq!(calls.load(Ordering::Relaxed), 64);
}

#[tokio::test]
async fn the_total_traversal_node_limit_includes_the_selected_root() {
    let result = build(&["root".into()], |_| async move {
        Ok(Metadata {
            entry: entry("root", "Root", true),
            children: (0..100_000)
                .map(|index| entry(&index.to_string(), &index.to_string(), false))
                .collect(),
        })
    })
    .await;
    assert!(result.err().unwrap().to_string().contains("capacity"));
}

#[tokio::test]
async fn metadata_is_read_once_per_directory_and_paths_preserve_empty_nested_directories() {
    let calls = AtomicUsize::new(0);
    let plan = build(&["root".into()], |url| {
        calls.fetch_add(1, Ordering::Relaxed);
        async move {
            Ok(if url == "root" {
                Metadata {
                    entry: entry("root", "Root", true),
                    children: vec![
                        entry("nested", "nested", true),
                        entry("file", "文件.bin", false),
                    ],
                }
            } else {
                Metadata {
                    entry: entry("nested", "nested", true),
                    children: vec![],
                }
            })
        }
    })
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(
        plan.directories,
        [PathBuf::from("Root"), PathBuf::from("Root/nested")]
    );
    assert_eq!(plan.targets[0].relative, PathBuf::from("Root/文件.bin"));
}
