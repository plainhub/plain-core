use crate::rusqlite::error::error_from_handle;
use crate::rusqlite::ffi;
use crate::rusqlite::{Connection, MAIN_DB, Name, Result};
use std::ffi::c_int;
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;
use std::thread;
use std::time::Duration;
impl Connection {
    pub fn backup<N: Name, P: AsRef<Path>, F: Fn(Progress)>(
        &self,
        name: N,
        dst_path: P,
        progress: Option<F>,
    ) -> Result<()> {
        use self::StepResult::{Busy, Done, Locked, More};
        let mut dst = Self::open(dst_path)?;
        let backup = Backup::new_with_names(self, name, &mut dst, MAIN_DB)?;
        let mut r = More;
        while r == More {
            r = backup.step(100)?;
            if let Some(ref f) = progress {
                f(backup.progress());
            }
        }
        match r {
            Done => Ok(()),
            Busy => Err(unsafe { error_from_handle(ptr::null_mut(), ffi::SQLITE_BUSY) }),
            Locked => Err(unsafe { error_from_handle(ptr::null_mut(), ffi::SQLITE_LOCKED) }),
            More => unreachable!(),
        }
    }
    pub fn restore<N: Name, P: AsRef<Path>, F: Fn(Progress)>(
        &mut self,
        name: N,
        src_path: P,
        progress: Option<F>,
    ) -> Result<()> {
        use self::StepResult::{Busy, Done, Locked, More};
        let src = Self::open(src_path)?;
        let restore = Backup::new_with_names(&src, MAIN_DB, self, name)?;
        let mut r = More;
        let mut busy_count = 0_i32;
        'restore_loop: while r == More || r == Busy {
            r = restore.step(100)?;
            if let Some(ref f) = progress {
                f(restore.progress());
            }
            if r == Busy {
                busy_count += 1;
                if busy_count >= 3 {
                    break 'restore_loop;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
        match r {
            Done => Ok(()),
            Busy => Err(unsafe { error_from_handle(ptr::null_mut(), ffi::SQLITE_BUSY) }),
            Locked => Err(unsafe { error_from_handle(ptr::null_mut(), ffi::SQLITE_LOCKED) }),
            More => unreachable!(),
        }
    }
}
pub const NO_PROGRESS: Option<fn(_: Progress)> = None;
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepResult {
    Done,
    More,
    Busy,
    Locked,
}
#[derive(Copy, Clone, Debug)]
pub struct Progress {
    pub remaining: c_int,
    pub pagecount: c_int,
}
pub struct Backup<'a, 'b> {
    phantom_from: PhantomData<&'a Connection>,
    to: &'b Connection,
    b: *mut ffi::sqlite3_backup,
}
impl Backup<'_, '_> {
    #[inline]
    pub fn new<'a, 'b>(from: &'a Connection, to: &'b mut Connection) -> Result<Backup<'a, 'b>> {
        Backup::new_with_names(from, MAIN_DB, to, MAIN_DB)
    }
    pub fn new_with_names<'a, 'b, F: Name, T: Name>(
        from: &'a Connection,
        from_name: F,
        to: &'b mut Connection,
        to_name: T,
    ) -> Result<Backup<'a, 'b>> {
        let to_name = to_name.as_cstr()?;
        let from_name = from_name.as_cstr()?;
        let to_db = to.db.borrow_mut().db;
        let b = unsafe {
            let b = ffi::sqlite3_backup_init(
                to_db,
                to_name.as_ptr(),
                from.db.borrow_mut().db,
                from_name.as_ptr(),
            );
            if b.is_null() {
                return Err(error_from_handle(to_db, ffi::sqlite3_errcode(to_db)));
            }
            b
        };
        Ok(Backup {
            phantom_from: PhantomData,
            to,
            b,
        })
    }
    #[inline]
    #[must_use]
    pub fn progress(&self) -> Progress {
        unsafe {
            Progress {
                remaining: ffi::sqlite3_backup_remaining(self.b),
                pagecount: ffi::sqlite3_backup_pagecount(self.b),
            }
        }
    }
    #[inline]
    pub fn step(&self, num_pages: c_int) -> Result<StepResult> {
        use self::StepResult::{Busy, Done, Locked, More};
        let rc = unsafe { ffi::sqlite3_backup_step(self.b, num_pages) };
        match rc {
            ffi::SQLITE_DONE => Ok(Done),
            ffi::SQLITE_OK => Ok(More),
            ffi::SQLITE_BUSY => Ok(Busy),
            ffi::SQLITE_LOCKED => Ok(Locked),
            _ => self.to.decode_result(rc).map(|()| More),
        }
    }
}
impl Drop for Backup<'_, '_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { ffi::sqlite3_backup_finish(self.b) };
    }
}
