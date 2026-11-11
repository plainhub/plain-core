pub mod bounds;
mod io;
mod timer;
pub use self::io::{Read, ReadBuf, ReadBufCursor, Write};
pub use self::timer::{Sleep, Timer};
pub trait Executor<Fut> {
    fn execute(&self, fut: Fut);
}
