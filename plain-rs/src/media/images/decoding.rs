use std::{future::Future, path::Path, pin::Pin};

pub trait ThumbnailDecoder: Send + Sync {
    fn decode<'a>(
        &'a self,
        path: &'a str,
        width: u32,
        height: u32,
        center_crop: bool,
        cache: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, String>> + Send + 'a>>;
}
