use super::*;

#[test]
fn inotify_watcher_is_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<INotifyWatcher>();
}
