use super::*;

fn server() -> ReadDirectoryChangesServer {
    let (_, rx) = unbounded();
    let (cmd_tx, _) = unbounded();
    ReadDirectoryChangesServer {
        rx,
        cmd_tx,
        event_handler: Arc::new(Mutex::new(|_| {})),
        watches: HashMap::new(),
        wakeup: Arc::new(create_wakeup().unwrap()),
    }
}

#[test]
fn repeated_registration_drains_the_old_request_before_releasing_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut server = server();
    let path = directory.path().to_path_buf();
    server.add_watch(path.clone(), true).unwrap();
    let old = Rc::downgrade(server.watches.get(&path).unwrap());
    server.add_watch(path.clone(), false).unwrap();
    assert!(old.upgrade().is_none());
    assert_eq!(server.watches.len(), 1);
    let current = Rc::downgrade(server.watches.get(&path).unwrap());
    server.remove_watch(path).unwrap();
    assert!(current.upgrade().is_none());
    assert!(server.watches.is_empty());
}

#[test]
fn failed_native_submission_releases_request_without_waiting_for_a_callback() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let state = Rc::new(WatchState {
        handle: File::open(file.path()).unwrap(),
        dir: file.path().to_path_buf(),
        file_filter: None,
        is_recursive: false,
        stopping: Cell::new(false),
        pending: Cell::new(false),
    });
    let handler: Arc<Mutex<dyn EventHandler>> = Arc::new(Mutex::new(|_| {}));
    assert!(start_read(state.clone(), handler.clone()).is_err());
    assert_eq!(Rc::strong_count(&state), 1);
    assert_eq!(Arc::strong_count(&handler), 1);
    assert!(!state.pending.get());
    stop_watch(&state).unwrap();
    assert!(state.stopping.get());
}

#[test]
fn completion_pointer_addresses_the_request_prefix_and_buffer_is_aligned() {
    assert_eq!(std::mem::offset_of!(ReadDirectoryRequest, overlapped), 0);
    assert_eq!(std::mem::offset_of!(ReadDirectoryRequest, buffer) % 4, 0);
    assert!(std::mem::align_of::<ReadDirectoryRequest>() >= 4);
}

#[test]
fn dropping_the_controller_joins_and_releases_the_handler() {
    let handler: Arc<Mutex<dyn EventHandler>> = Arc::new(Mutex::new(|_| {}));
    let weak = Arc::downgrade(&handler);
    let directory = tempfile::tempdir().unwrap();
    let mut watcher = ReadDirectoryChangesWatcher::create(handler).unwrap();
    watcher
        .watch(directory.path(), RecursiveMode::Recursive)
        .unwrap();
    drop(watcher);
    assert!(weak.upgrade().is_none());
}

#[test]
fn stopping_consumes_successful_or_aborted_completions_without_rearming() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    for error in [0, ERROR_OPERATION_ABORTED] {
        let file = tempfile::NamedTempFile::new().unwrap();
        let state = Rc::new(WatchState {
            handle: File::open(file.path()).unwrap(),
            dir: file.path().to_path_buf(),
            file_filter: None,
            is_recursive: false,
            stopping: Cell::new(true),
            pending: Cell::new(true),
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let request = Box::new(ReadDirectoryRequest {
            overlapped: OVERLAPPED {
                Internal: 0,
                InternalHigh: 0,
                Anonymous: OVERLAPPED_0 {
                    Anonymous: OVERLAPPED_0_0 {
                        Offset: 0,
                        OffsetHigh: 0,
                    },
                },
                hEvent: 0,
            },
            buffer: NotificationBuffer([0; BUF_SIZE as usize]),
            state: state.clone(),
            event_handler: Arc::new(Mutex::new(move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
            })),
        });
        unsafe {
            handle_event(error, 0, Box::into_raw(request).cast());
        }
        assert!(!state.pending.get());
        assert_eq!(Rc::strong_count(&state), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
