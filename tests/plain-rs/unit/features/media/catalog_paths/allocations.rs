use super::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

std::thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct CountingAllocator;

fn record_allocation() {
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get().map(|value| value + 1)));
}

// Delegate allocation ownership and layout requirements to the system allocator.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn excluded(path: &str) -> bool {
    is_media_excluded_name_or_root(path, Path::new("/app-data"), Path::new("/app-cache"), &[])
}

#[cfg(target_os = "macos")]
#[test]
fn media_rule_matching_allocates_only_for_existing_data_and_cache_normalization() {
    for (path, expected, allocations) in [
        (
            "/Volumes/Media/Photos.PHOTOSLIBRARY/originals/a.jpg",
            true,
            0,
        ),
        ("/Volumes/Media/🖼.APP/image.jpg", true, 0),
        ("/usr/share/a.jpg", true, 0),
        ("/Users/first/Library/Caches/a.jpg", true, 0),
        ("/DATA/Gallery/a.jpg", false, 2),
    ] {
        ALLOCATIONS.with(|count| count.set(Some(0)));
        let result = excluded(std::hint::black_box(path));
        let count = ALLOCATIONS.with(|count| count.replace(None).unwrap());
        assert_eq!(result, expected, "{path}");
        assert_eq!(count, allocations, "{path}");
    }
}

#[test]
#[ignore]
fn media_rule_matching_benchmark() {
    let paths = [
        "/DATA/Gallery/a.jpg",
        "/Volumes/Media/Photos.PHOTOSLIBRARY/originals/a.jpg",
        "/usr/share/a.jpg",
        "/Users/first/Library/Caches/a.jpg",
        "/Users/second/Library/Mobile Documents/com~apple~CloudDocs/report.pdf",
        "/DATA/project/node_modules/image.png",
    ];
    let start = std::time::Instant::now();
    let mut matched = 0;
    for _ in 0..100_000 {
        for path in paths {
            matched += usize::from(std::hint::black_box(excluded(std::hint::black_box(path))));
        }
    }
    println!(
        "paths=600000 matched={matched} elapsed_ns={}",
        start.elapsed().as_nanos()
    );
}
