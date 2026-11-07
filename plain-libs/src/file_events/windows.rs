use crate::file_events::event::*;
use crate::file_events::{Error, EventHandler, RecursiveMode, Result, Watcher};
use crate::file_events::{Receiver, Sender, unbounded};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr;
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc::TryRecvError};
use std::thread::{self, JoinHandle};
use windows_sys::Win32::Foundation::{
    ERROR_NOT_FOUND, ERROR_NOTIFY_ENUM_DIR, ERROR_OPERATION_ABORTED, ERROR_TOO_MANY_POSTS, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_IO_COMPLETION, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ACTION_ADDED, FILE_ACTION_MODIFIED, FILE_ACTION_REMOVED, FILE_ACTION_RENAMED_NEW_NAME,
    FILE_ACTION_RENAMED_OLD_NAME, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED,
    FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_CREATION,
    FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
    FILE_NOTIFY_CHANGE_SECURITY, FILE_NOTIFY_CHANGE_SIZE, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, ReadDirectoryChangesW,
};
use windows_sys::Win32::System::IO::{CancelIoEx, OVERLAPPED, OVERLAPPED_0, OVERLAPPED_0_0};
use windows_sys::Win32::System::Threading::{
    CreateSemaphoreW, INFINITE, ReleaseSemaphore, SleepEx, WaitForSingleObjectEx,
};

const BUF_SIZE: u32 = 16384;

#[repr(align(4))]
struct NotificationBuffer([u8; BUF_SIZE as usize]);

// The native OVERLAPPED pointer is also the address of this stable request allocation.
#[repr(C)]
struct ReadDirectoryRequest {
    overlapped: OVERLAPPED,
    buffer: NotificationBuffer,
    state: Rc<WatchState>,
    event_handler: Arc<Mutex<dyn EventHandler>>,
}

struct WatchState {
    handle: File,
    dir: PathBuf,
    file_filter: Option<PathBuf>,
    is_recursive: bool,
    stopping: Cell<bool>,
    pending: Cell<bool>,
}

enum Action {
    Watch(PathBuf, RecursiveMode),
    Unwatch(PathBuf),
    Stop,
}

struct ReadDirectoryChangesServer {
    rx: Receiver<Action>,
    event_handler: Arc<Mutex<dyn EventHandler>>,
    cmd_tx: Sender<Result<()>>,
    watches: HashMap<PathBuf, Rc<WatchState>>,
    wakeup: Arc<OwnedHandle>,
}

impl ReadDirectoryChangesServer {
    fn run(mut self) {
        loop {
            loop {
                let result = match self.rx.try_recv() {
                    Ok(Action::Watch(path, mode)) => self.add_watch(path, mode.is_recursive()),
                    Ok(Action::Unwatch(path)) => self.remove_watch(path),
                    Ok(Action::Stop) | Err(TryRecvError::Disconnected) => return,
                    Err(TryRecvError::Empty) => break,
                };
                if self.cmd_tx.send(result).is_err() {
                    return;
                }
            }
            let status =
                unsafe { WaitForSingleObjectEx(self.wakeup.as_raw_handle() as HANDLE, 100, 1) };
            if !matches!(status, WAIT_OBJECT_0 | WAIT_IO_COMPLETION | WAIT_TIMEOUT) {
                emit_event(
                    &self.event_handler,
                    Err(Error::io(std::io::Error::last_os_error())),
                );
                return;
            }
        }
    }

    fn add_watch(&mut self, path: PathBuf, is_recursive: bool) -> Result<()> {
        let metadata = path
            .metadata()
            .map_err(|error| Error::io_watch(error).add_path(path.clone()))?;
        let (dir, file_filter) = if metadata.is_dir() {
            (path.clone(), None)
        } else if metadata.is_file() {
            (path.parent().unwrap().to_path_buf(), Some(path.clone()))
        } else {
            return Err(
                Error::generic("Input watch path is neither a file nor a directory").add_path(path),
            );
        };
        let handle = OpenOptions::new()
            .access_mode(FILE_LIST_DIRECTORY)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED)
            .open(&dir)
            .map_err(|error| Error::io(error).add_path(path.clone()))?;
        let state = Rc::new(WatchState {
            handle,
            dir,
            file_filter,
            is_recursive,
            stopping: Cell::new(false),
            pending: Cell::new(false),
        });
        if let Some(previous) = self.watches.remove(&path) {
            stop_watch(&previous)?;
        }
        start_read(state.clone(), self.event_handler.clone())?;
        self.watches.insert(path, state);
        Ok(())
    }

    fn remove_watch(&mut self, path: PathBuf) -> Result<()> {
        let state = self
            .watches
            .remove(&path)
            .ok_or_else(|| Error::watch_not_found().add_path(path))?;
        stop_watch(&state)
    }
}

impl Drop for ReadDirectoryChangesServer {
    fn drop(&mut self) {
        for state in self.watches.values() {
            if let Err(error) = stop_watch(state) {
                emit_event(&self.event_handler, Err(error));
            }
        }
    }
}

fn stop_watch(state: &WatchState) -> Result<()> {
    state.stopping.set(true);
    if !state.pending.get() {
        return Ok(());
    }
    let error = if unsafe { CancelIoEx(state.handle.as_raw_handle() as HANDLE, ptr::null()) } == 0 {
        let error = std::io::Error::last_os_error();
        (error.raw_os_error() != Some(ERROR_NOT_FOUND as i32)).then_some(error)
    } else {
        None
    };
    // Cancellation does not complete the request; its issuing thread must drain the completion APC.
    while state.pending.get() {
        unsafe {
            SleepEx(INFINITE, 1);
        }
    }
    match error {
        Some(error) => Err(Error::io(error).add_path(state.dir.clone())),
        None => Ok(()),
    }
}

fn start_read(state: Rc<WatchState>, event_handler: Arc<Mutex<dyn EventHandler>>) -> Result<()> {
    let request = Box::into_raw(Box::new(ReadDirectoryRequest {
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
        event_handler,
    }));
    let flags = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_ATTRIBUTES
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_CREATION
        | FILE_NOTIFY_CHANGE_SECURITY;
    let monitor_subdir = i32::from(state.file_filter.is_none() && state.is_recursive);
    state.pending.set(true);
    let result = unsafe {
        ReadDirectoryChangesW(
            state.handle.as_raw_handle() as HANDLE,
            ptr::addr_of_mut!((*request).buffer.0).cast(),
            BUF_SIZE,
            monitor_subdir,
            flags,
            ptr::null_mut(),
            ptr::addr_of_mut!((*request).overlapped),
            Some(handle_event),
        )
    };
    if result == 0 {
        let error = std::io::Error::last_os_error();
        state.pending.set(false);
        unsafe {
            drop(Box::from_raw(request));
        }
        return Err(Error::io(error).add_path(state.dir.clone()));
    }
    // The native request owns this allocation until its issuing thread runs the completion APC.
    Ok(())
}

fn emit_event(handler: &Arc<Mutex<dyn EventHandler>>, event: Result<Event>) {
    handler
        .lock()
        .expect("File watch event handler lock poisoned")
        .handle_event(event);
}

unsafe extern "system" fn handle_event(
    error_code: u32,
    bytes_written: u32,
    overlapped: *mut OVERLAPPED,
) {
    let request = unsafe { Box::from_raw(overlapped.cast::<ReadDirectoryRequest>()) };
    request.state.pending.set(false);
    if request.state.stopping.get() {
        return;
    }
    if error_code == ERROR_OPERATION_ABORTED {
        emit_event(
            &request.event_handler,
            Err(
                Error::io(std::io::Error::from_raw_os_error(error_code as i32))
                    .add_path(request.state.dir.clone()),
            ),
        );
        return;
    }
    if let Err(error) = start_read(request.state.clone(), request.event_handler.clone()) {
        emit_event(&request.event_handler, Err(error));
    }
    if error_code == ERROR_NOTIFY_ENUM_DIR || (error_code == 0 && bytes_written == 0) {
        emit_event(
            &request.event_handler,
            Ok(Event::new(EventKind::Other)
                .set_flag(Flag::Rescan)
                .add_path(request.state.dir.clone())),
        );
        return;
    }
    if error_code != 0 {
        emit_event(
            &request.event_handler,
            Err(
                Error::io(std::io::Error::from_raw_os_error(error_code as i32))
                    .add_path(request.state.dir.clone()),
            ),
        );
        return;
    }
    let Some(bytes) = request.buffer.0.get(..bytes_written as usize) else {
        emit_event(
            &request.event_handler,
            Err(Error::generic("Directory notification exceeds buffer")
                .add_path(request.state.dir.clone())),
        );
        return;
    };
    let records = match super::windows_records::records(bytes) {
        Ok(records) => records,
        Err(message) => {
            emit_event(
                &request.event_handler,
                Err(Error::generic(message).add_path(request.state.dir.clone())),
            );
            return;
        }
    };
    for record in records {
        let name: Vec<_> = record
            .name
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect();
        let path = request.state.dir.join(OsString::from_wide(&name));
        if request
            .state
            .file_filter
            .as_ref()
            .is_some_and(|file| *file != path)
        {
            continue;
        }
        let kind = match record.action {
            FILE_ACTION_RENAMED_OLD_NAME => EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            FILE_ACTION_RENAMED_NEW_NAME => EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            FILE_ACTION_ADDED => EventKind::Create(CreateKind::Any),
            FILE_ACTION_REMOVED => EventKind::Remove(RemoveKind::Any),
            FILE_ACTION_MODIFIED => EventKind::Modify(ModifyKind::Any),
            _ => continue,
        };
        emit_event(&request.event_handler, Ok(Event::new(kind).add_path(path)));
    }
}

fn create_wakeup() -> Result<OwnedHandle> {
    let raw = unsafe { CreateSemaphoreW(ptr::null_mut(), 0, 1, ptr::null_mut()) };
    if raw == 0 || raw == INVALID_HANDLE_VALUE {
        return Err(Error::io(std::io::Error::last_os_error()));
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(raw as _) })
}

#[derive(Debug)]
pub struct ReadDirectoryChangesWatcher {
    tx: Sender<Action>,
    cmd_rx: Receiver<Result<()>>,
    wakeup: Arc<OwnedHandle>,
    worker: Option<JoinHandle<()>>,
}

impl ReadDirectoryChangesWatcher {
    fn create(event_handler: Arc<Mutex<dyn EventHandler>>) -> Result<Self> {
        let wakeup = Arc::new(create_wakeup()?);
        let (tx, rx) = unbounded();
        let (cmd_tx, cmd_rx) = unbounded();
        let worker_wakeup = wakeup.clone();
        let worker = thread::Builder::new()
            .name("filesystem events".into())
            .spawn(move || {
                ReadDirectoryChangesServer {
                    rx,
                    event_handler,
                    cmd_tx,
                    watches: HashMap::new(),
                    wakeup: worker_wakeup,
                }
                .run();
            })
            .map_err(Error::io)?;
        Ok(Self {
            tx,
            cmd_rx,
            wakeup,
            worker: Some(worker),
        })
    }

    fn wakeup_server(&self) -> Result<()> {
        if unsafe { ReleaseSemaphore(self.wakeup.as_raw_handle() as HANDLE, 1, ptr::null_mut()) }
            == 0
        {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_TOO_MANY_POSTS as i32) {
                return Err(Error::io(error));
            }
        }
        Ok(())
    }

    fn send_action_require_ack(&mut self, action: Action) -> Result<()> {
        self.tx
            .send(action)
            .map_err(|_| Error::generic("File watcher worker stopped"))?;
        self.wakeup_server()?;
        self.cmd_rx
            .recv()
            .map_err(|_| Error::generic("File watcher acknowledgement channel closed"))?
    }
}

impl Watcher for ReadDirectoryChangesWatcher {
    fn new<F: EventHandler>(event_handler: F) -> Result<Self> {
        Self::create(Arc::new(Mutex::new(event_handler)))
    }
    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> Result<()> {
        let path = std::path::absolute(path).map_err(Error::io)?;
        self.send_action_require_ack(Action::Watch(path, mode))
    }
    fn unwatch(&mut self, path: &Path) -> Result<()> {
        let path = std::path::absolute(path).map_err(Error::io)?;
        self.send_action_require_ack(Action::Unwatch(path))
    }
}

impl Drop for ReadDirectoryChangesWatcher {
    fn drop(&mut self) {
        if self.tx.send(Action::Stop).is_ok() {
            if let Err(error) = self.wakeup_server() {
                log::error!("Cannot wake file watcher during shutdown: {error}");
            }
        }
        if let Some(worker) = self.worker.take() {
            // A handler may drop its controller on the worker; the queued stop then owns shutdown.
            if worker.thread().id() != thread::current().id() {
                if worker.join().is_err() {
                    log::error!("File watcher worker panicked during shutdown");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/file_events/windows.rs"]
mod tests;
