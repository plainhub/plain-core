use crate::rusqlite::ffi;
use crate::rusqlite::{Connection, InnerConnection, Result};
use std::ffi::{c_int, c_void};
use std::panic::catch_unwind;
use std::time::Duration;
impl Connection {
    pub fn busy_timeout(&self, timeout: Duration) -> Result<()> {
        let ms: i32 = timeout
            .as_secs()
            .checked_mul(1000)
            .and_then(|t| t.checked_add(timeout.subsec_millis().into()))
            .and_then(|t| t.try_into().ok())
            .expect("too big");
        self.db.borrow_mut().busy_timeout(ms)
    }
}
impl InnerConnection {
    #[inline]
    fn busy_timeout(&mut self, timeout: c_int) -> Result<()> {
        let r = unsafe { ffi::sqlite3_busy_timeout(self.db, timeout) };
        let res = self.decode_result(r);
        if res.is_ok() {
            self.set_clientdata(c"sqlite3_busy_handler", None::<c_void>, |_, _| {
                ffi::SQLITE_OK
            })?;
        }
        res
    }
}
