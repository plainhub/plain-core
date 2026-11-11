use crate::rusqlite::error::decode_result_raw;
use crate::rusqlite::ffi;
use crate::rusqlite::inner_connection::InnerConnection;
use crate::rusqlite::{Connection, Name, Result};
use std::ffi::c_int;
use std::ptr;
#[derive(Clone, Copy, Default)]
#[repr(i32)]
#[non_exhaustive]
pub enum CheckpointMode {
    #[default]
    PASSIVE = ffi::SQLITE_CHECKPOINT_PASSIVE,
    FULL = ffi::SQLITE_CHECKPOINT_FULL,
    RESTART = ffi::SQLITE_CHECKPOINT_RESTART,
    TRUNCATE = ffi::SQLITE_CHECKPOINT_TRUNCATE,
    NOOP = -1,
}
impl Connection {
    pub fn wal_checkpoint_v2<N: Name>(
        &self,
        db_name: Option<N>,
        mode: CheckpointMode,
    ) -> Result<(c_int, c_int)> {
        self.db.borrow_mut().wal_checkpoint_v2(db_name, mode)
    }
}
impl InnerConnection {
    pub(crate) fn wal_checkpoint_v2<N: Name>(
        &mut self,
        db_name: Option<N>,
        mode: CheckpointMode,
    ) -> Result<(c_int, c_int)> {
        let cs = db_name.as_ref().map(N::as_cstr).transpose()?;
        let db_name = cs.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        let mut n_log = 0;
        let mut n_ckpt = 0;
        unsafe {
            decode_result_raw(
                self.db,
                ffi::sqlite3_wal_checkpoint_v2(
                    self.db,
                    db_name,
                    mode as c_int,
                    &raw mut n_log,
                    &raw mut n_ckpt,
                ),
            )?;
        };
        Ok((n_log, n_ckpt))
    }
}
