
    use std::os::unix::ffi::OsStrExt;

    use super::*;

    #[test]
    fn invalid_path() {
        assert_eq!(NSURL::from_file_path(""), None);
        assert_eq!(NSURL::from_file_path("/\0/a"), None);
    }

    #[test]
    fn roundtrip() {
        let path = Path::new(OsStr::from_bytes(b"/abc/def"));
        let url = NSURL::from_file_path(path).unwrap();
        assert_eq!(url.to_file_path().unwrap(), path);

        let path = Path::new(OsStr::from_bytes(b"/\x08"));
        let url = NSURL::from_file_path(path).unwrap();
        assert_eq!(url.to_file_path().unwrap(), path);

        // Non-unicode
        let path = Path::new(OsStr::from_bytes(b"/\x08"));
        let url = NSURL::from_file_path(path).unwrap();
        assert_eq!(url.to_file_path().unwrap(), path);
    }

    #[test]
    #[cfg(all(feature = "NSData", any(), feature = "NSError"))]
    #[ignore = "needs HFS+ file system"]
    fn special_paths() {
        use crate::{NSData, NSFileManager};

        let manager = NSFileManager::defaultManager();

        let path = Path::new(OsStr::from_bytes(b"\xf8"));
        // Foundation is broken, needs a different encoding to work.
        let url = NSURL::from_file_path("%F8").unwrap();

        // Create, read and remove file, using different APIs.
        std::fs::write(path, "").unwrap();
        assert_eq!(NSData::dataWithContentsOfURL(&url), Some(NSData::new()));
        manager.removeItemAtURL_error(&url).unwrap();
    }

    // Useful when testing HFS+ and non-UTF-8:
    // echo > $(echo "0000000: f8" | xxd -r)

