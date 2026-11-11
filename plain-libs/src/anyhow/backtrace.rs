pub(crate) use std::backtrace::Backtrace;
macro_rules! backtrace {
    () => {
        Some(std::backtrace::Backtrace::capture())
    };
}
macro_rules! backtrace_if_absent {
    ($ err : expr) => {
        backtrace!()
    };
}
fn _assert_send_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<Backtrace>();
}
