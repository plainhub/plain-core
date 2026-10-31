use std::sync::Mutex;

static RETAINED: Mutex<Vec<tempfile::TempDir>> = Mutex::new(Vec::new());

pub fn retain(dir: tempfile::TempDir) {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| unsafe {
        libc::atexit(clear);
    });
    RETAINED.lock().unwrap().push(dir);
}

extern "C" fn clear() {
    // Dropping the vec runs each TempDir's recursive delete; bail
    // out if a thread still holds the lock at exit.
    if let Ok(mut dirs) = RETAINED.try_lock() {
        dirs.clear();
    }
}
