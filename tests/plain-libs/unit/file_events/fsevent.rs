use super::*;

#[test]
fn watcher_drop_drains_and_releases_the_native_context() {
    let handler: Arc<Mutex<dyn EventHandler>> = Arc::new(Mutex::new(|_| {}));
    let weak = Arc::downgrade(&handler);
    let directory = tempfile::tempdir().unwrap();
    let mut watcher = FsEventWatcher::from_event_handler(handler).unwrap();
    watcher
        .watch(directory.path(), RecursiveMode::Recursive)
        .unwrap();
    let context = Arc::downgrade(&watcher.stream.as_ref().unwrap().context);
    drop(watcher);
    assert!(context.upgrade().is_none());
    assert!(weak.upgrade().is_none());
}

#[test]
fn test_steam_context_info_send_and_sync() {
    fn check_send<T: Send + Sync>() {}
    check_send::<StreamContextInfo>();
}

#[test]
fn native_context_callbacks_balance_arc_ownership() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Handler(Arc<AtomicUsize>);
    impl EventHandler for Handler {
        fn handle_event(&mut self, _: Result<Event>) {}
    }
    impl Drop for Handler {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let context = Arc::new(StreamContextInfo {
        active: AtomicBool::new(true),
        event_handler: Arc::new(Mutex::new(Handler(drops.clone()))),
        recursive_info: HashMap::new(),
    });
    let callback_context = Arc::new(Arc::downgrade(&context));
    let weak = Arc::downgrade(&callback_context);
    let pointer = Arc::as_ptr(&callback_context).cast();
    assert_eq!(retain_context(pointer), pointer);
    assert_eq!(Arc::strong_count(&callback_context), 2);
    drop(callback_context);
    drop(context);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(weak.upgrade().unwrap().upgrade().is_none());
    release_context(pointer);
    assert!(weak.upgrade().is_none());
}

#[test]
fn deleted_watch_paths_keep_their_canonical_existing_ancestor() {
    let directory = tempfile::tempdir().unwrap();
    let alias = directory.path().join("alias");
    let actual = directory.path().join("actual");
    std::fs::create_dir(&actual).unwrap();
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    let child = alias.join("missing").join("nested");
    assert_eq!(
        canonical_watch_path(&child).unwrap(),
        actual.canonicalize().unwrap().join("missing/nested")
    );
}

#[test]
fn dropping_a_watcher_on_its_event_queue_defers_release_without_self_waiting() {
    struct Handler(std::sync::mpsc::Sender<()>);
    impl EventHandler for Handler {
        fn handle_event(&mut self, _: Result<Event>) {}
    }
    impl Drop for Handler {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    extern "C" fn drop_watcher(context: *mut libc::c_void) {
        drop(unsafe { Box::from_raw(context.cast::<FsEventWatcher>()) });
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let directory = tempfile::tempdir().unwrap();
    let mut watcher = FsEventWatcher::new(Handler(tx)).unwrap();
    watcher
        .watch(directory.path(), RecursiveMode::Recursive)
        .unwrap();
    let queue = watcher.stream.as_ref().unwrap().queue.0;
    unsafe {
        dispatch_async_f(queue, Box::into_raw(Box::new(watcher)).cast(), drop_watcher);
    }
    rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
}

#[test]
fn callback_keeps_context_alive_and_stops_emitting_when_deactivated() {
    use std::sync::atomic::AtomicUsize;
    let calls = Arc::new(AtomicUsize::new(0));
    let context = Arc::<StreamContextInfo>::new_cyclic(|weak| {
        let weak = weak.clone();
        let calls = calls.clone();
        StreamContextInfo {
            active: AtomicBool::new(true),
            recursive_info: HashMap::from([(PathBuf::from("/watch"), true)]),
            event_handler: Arc::new(Mutex::new(move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                weak.upgrade()
                    .unwrap()
                    .active
                    .store(false, Ordering::Release);
            })),
        }
    });
    let paths = [c"/watch/file".as_ptr(); 2];
    let flags =
        [fs::kFSEventStreamEventFlagItemCreated | fs::kFSEventStreamEventFlagItemModified; 2];
    let callback_context = Arc::downgrade(&context);
    callback(
        ptr::null_mut(),
        ptr::from_ref(&callback_context).cast_mut().cast(),
        2,
        paths.as_ptr().cast_mut().cast(),
        flags.as_ptr(),
        ptr::null(),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(Arc::strong_count(&context), 1);
}

#[test]
fn empty_native_batch_does_not_read_null_event_arrays() {
    let context = Arc::new(StreamContextInfo {
        active: AtomicBool::new(true),
        recursive_info: HashMap::new(),
        event_handler: Arc::new(Mutex::new(|_| {})),
    });
    let callback_context = Arc::downgrade(&context);
    callback(
        ptr::null_mut(),
        ptr::from_ref(&callback_context).cast_mut().cast(),
        0,
        ptr::null_mut(),
        ptr::null(),
        ptr::null(),
    );
    assert_eq!(Arc::strong_count(&context), 1);
}
