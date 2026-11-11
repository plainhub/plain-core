pub(crate) fn frame_threads() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("RS_H264_FRAME_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    })
}
pub(crate) fn row_progress_on() -> bool {
    {
        return false;
    }
}
pub(crate) fn row_publish_on() -> bool {
    {
        return false;
    }
}
