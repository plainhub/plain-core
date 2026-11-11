use crate::hyper::{
    rt::{Read, Write},
    upgrade::Upgraded,
};
use crate::hyper_util::common::rewind::Rewind;
use bytes::{Bytes, BytesMut};
pub fn downcast<T>(upgraded: Upgraded) -> Result<Parts<T>, Upgraded>
where
    T: Read + Write + Unpin + 'static,
{
    let crate::hyper::upgrade::Parts {
        io: rewind,
        mut read_buf,
        ..
    } = upgraded.downcast::<Rewind<T>>()?;
    if let Some(pre) = rewind.pre {
        read_buf = if read_buf.is_empty() {
            pre
        } else {
            let mut buf = BytesMut::from(read_buf);
            buf.extend_from_slice(&pre);
            buf.freeze()
        };
    }
    Ok(Parts {
        io: rewind.inner,
        read_buf,
    })
}
#[derive(Debug)]
#[non_exhaustive]
pub struct Parts<T> {
    pub io: T,
    pub read_buf: Bytes,
}
