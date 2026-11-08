use super::macos_ffi as fs;
use crate::file_events::event::*;
use crate::file_events::{Error, EventHandler, RecursiveMode, Result, Watcher};
use crate::platform::apple_device as cf;
use crate::platform::apple_device::OwnedCF;
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

// Event flags are from fsevent-sys 4.1.0 (fsevent-sys/SOURCE.json).
bitflags::bitflags! {
  #[repr(C)]
  #[derive(Debug)]
  struct StreamFlags: u32 {
    const MUST_SCAN_SUBDIRS = 0x00000001;
    const HISTORY_DONE = 0x00000010;
    const ROOT_CHANGED = 0x00000020;
    const MOUNT = 0x00000040;
    const UNMOUNT = 0x00000080;
    const ITEM_CREATED = 0x00000100;
    const ITEM_REMOVED = 0x00000200;
    const INODE_META_MOD = 0x00000400;
    const ITEM_RENAMED = 0x00000800;
    const ITEM_MODIFIED = 0x00001000;
    const FINDER_INFO_MOD = 0x00002000;
    const ITEM_CHANGE_OWNER = 0x00004000;
    const ITEM_XATTR_MOD = 0x00008000;
    const IS_FILE = 0x00010000;
    const IS_DIR = 0x00020000;
    const IS_SYMLINK = 0x00040000;
    const IS_HARDLINK = 0x00100000;
    const ITEM_CLONED = 0x00400000;
  }
}

pub struct FsEventWatcher {
    paths: OwnedCF,
    event_handler: Arc<Mutex<dyn EventHandler>>,
    stream: Option<EventStream>,
    recursive_info: HashMap<PathBuf, bool>,
}

// Controller mutations are exclusive; stop unschedules and drains its serial event queue before changing context.
unsafe impl Send for FsEventWatcher {}

fn translate_flags(flags: StreamFlags) -> Vec<Event> {
    let mut evs = Vec::new();

    if flags.contains(StreamFlags::HISTORY_DONE) {
        return evs;
    }

    if flags.contains(StreamFlags::MUST_SCAN_SUBDIRS) {
        evs.push(Event::rescan());
    }
    if flags.contains(StreamFlags::ROOT_CHANGED) {
        evs.push(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::From,
        ))));
    }
    if flags.contains(StreamFlags::MOUNT) {
        evs.push(Event::new(EventKind::Create(CreateKind::Other)));
    }
    if flags.contains(StreamFlags::UNMOUNT) {
        evs.push(Event::new(EventKind::Remove(RemoveKind::Other)));
    }

    let link_or_clone = flags
        .intersects(StreamFlags::IS_SYMLINK | StreamFlags::IS_HARDLINK | StreamFlags::ITEM_CLONED);
    if flags.contains(StreamFlags::ITEM_CREATED) {
        evs.push(if flags.contains(StreamFlags::IS_DIR) {
            Event::new(EventKind::Create(CreateKind::Folder))
        } else if flags.contains(StreamFlags::IS_FILE) {
            Event::new(EventKind::Create(CreateKind::File))
        } else if link_or_clone {
            Event::new(EventKind::Create(CreateKind::Other))
        } else {
            Event::new(EventKind::Create(CreateKind::Any))
        });
    }

    if flags.contains(StreamFlags::ITEM_REMOVED) {
        evs.push(if flags.contains(StreamFlags::IS_DIR) {
            Event::new(EventKind::Remove(RemoveKind::Folder))
        } else if flags.contains(StreamFlags::IS_FILE) {
            Event::new(EventKind::Remove(RemoveKind::File))
        } else if link_or_clone {
            Event::new(EventKind::Remove(RemoveKind::Other))
        } else {
            Event::new(EventKind::Remove(RemoveKind::Any))
        });
    }

    // FSEvents provides no mechanism to associate the old and new sides of a
    // rename event.
    if flags.contains(StreamFlags::ITEM_RENAMED) {
        evs.push(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::Any,
        ))));
    }

    // This is only described as "metadata changed", but it may be that it's
    // only emitted for some more precise subset of events... if so, will need
    // amending, but for now we have an Any-shaped bucket to put it in.
    if flags.contains(StreamFlags::INODE_META_MOD) {
        evs.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Any,
        ))));
    }

    if flags.contains(StreamFlags::FINDER_INFO_MOD) {
        evs.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Other,
        ))));
    }

    if flags.contains(StreamFlags::ITEM_CHANGE_OWNER) {
        evs.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Ownership,
        ))));
    }

    if flags.contains(StreamFlags::ITEM_XATTR_MOD) {
        evs.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Extended,
        ))));
    }

    // This is specifically described as a data change, which we take to mean
    // is a content change.
    if flags.contains(StreamFlags::ITEM_MODIFIED) {
        evs.push(Event::new(EventKind::Modify(ModifyKind::Data(
            DataChange::Content,
        ))));
    }

    evs
}

struct StreamContextInfo {
    active: AtomicBool,
    event_handler: Arc<Mutex<dyn EventHandler>>,
    recursive_info: HashMap<PathBuf, bool>,
}

extern "C" fn retain_context(info: *const libc::c_void) -> *const libc::c_void {
    unsafe {
        Arc::increment_strong_count(info.cast::<Weak<StreamContextInfo>>());
    }
    info
}

extern "C" fn release_context(info: *const libc::c_void) {
    unsafe {
        Arc::decrement_strong_count(info.cast::<Weak<StreamContextInfo>>());
    }
}

type DispatchQueue = *mut libc::c_void;
static EVENT_QUEUE_KEY: u8 = 0;

unsafe extern "C" {
    fn dispatch_queue_create(
        label: *const libc::c_char,
        attributes: *const libc::c_void,
    ) -> DispatchQueue;
    fn dispatch_sync_f(
        queue: DispatchQueue,
        context: *mut libc::c_void,
        function: extern "C" fn(*mut libc::c_void),
    );
    fn dispatch_async_f(
        queue: DispatchQueue,
        context: *mut libc::c_void,
        function: extern "C" fn(*mut libc::c_void),
    );
    fn dispatch_queue_set_specific(
        queue: DispatchQueue,
        key: *const libc::c_void,
        context: *mut libc::c_void,
        destructor: Option<extern "C" fn(*mut libc::c_void)>,
    );
    fn dispatch_get_specific(key: *const libc::c_void) -> *mut libc::c_void;
    fn dispatch_release(object: DispatchQueue);
}

struct EventQueue(DispatchQueue);
impl EventQueue {
    fn new() -> Result<Self> {
        let queue = unsafe { dispatch_queue_create(c"filesystem events".as_ptr(), ptr::null()) };
        if queue.is_null() {
            return Err(Error::generic("Cannot allocate native event queue"));
        }
        unsafe {
            dispatch_queue_set_specific(queue, (&EVENT_QUEUE_KEY as *const u8).cast(), queue, None);
        }
        Ok(Self(queue))
    }
    fn is_current(&self) -> bool {
        unsafe { dispatch_get_specific((&EVENT_QUEUE_KEY as *const u8).cast()) == self.0 }
    }
}
impl Drop for EventQueue {
    fn drop(&mut self) {
        unsafe {
            dispatch_release(self.0);
        }
    }
}
struct EventStream {
    pointer: fs::FSEventStreamRef,
    queue: EventQueue,
    started: bool,
    context: Arc<StreamContextInfo>,
}
extern "C" fn release_stream(context: *mut libc::c_void) {
    unsafe {
        fs::FSEventStreamRelease(context.cast());
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        extern "C" fn drained(_: *mut libc::c_void) {}
        self.context.active.store(false, Ordering::Release);
        unsafe {
            if self.started {
                fs::FSEventStreamStop(self.pointer);
            }
            fs::FSEventStreamInvalidate(self.pointer);
            if self.queue.is_current() {
                // Release after this callback returns; synchronously waiting on its own queue would deadlock.
                dispatch_async_f(self.queue.0, self.pointer.cast(), release_stream);
            } else {
                dispatch_sync_f(self.queue.0, ptr::null_mut(), drained);
                fs::FSEventStreamRelease(self.pointer);
            }
        }
    }
}

fn native_watch_path(path: &Path) -> Result<OwnedCF> {
    let url = cf::cf_file_url(path).ok_or_else(|| {
        Error::generic("Cannot create native watch URL").add_path(path.to_owned())
    })?;
    unsafe {
        OwnedCF::from_owned(cf::CFURLCopyFileSystemPath(
            url.as_ptr(),
            cf::CF_URL_POSIX_PATH_STYLE,
        ))
    }
    .ok_or_else(|| Error::generic("Cannot create native watch path").add_path(path.to_owned()))
}

fn canonical_watch_path(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut existing = absolute.as_path();
    loop {
        match existing.canonicalize() {
            Ok(base) => return Ok(base.join(absolute.strip_prefix(existing).unwrap())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing.parent().ok_or(error)?;
            }
            Err(error) => return Err(error),
        }
    }
}

impl FsEventWatcher {
    fn from_event_handler(event_handler: Arc<Mutex<dyn EventHandler>>) -> Result<Self> {
        let paths = unsafe {
            OwnedCF::from_owned(fs::CFArrayCreateMutable(
                ptr::null(),
                0,
                &fs::kCFTypeArrayCallBacks,
            ))
        }
        .ok_or_else(|| Error::generic("Cannot allocate native watch paths"))?;
        Ok(Self {
            paths,
            event_handler,
            stream: None,
            recursive_info: HashMap::new(),
        })
    }

    fn watch_inner(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        self.stop();
        let result = self.append_path(path, recursive_mode);
        self.restart_after_update(result)
    }

    fn unwatch_inner(&mut self, path: &Path) -> Result<()> {
        self.stop();
        let result = self.remove_path(path);
        self.restart_after_update(result)
    }

    fn restart_after_update(&mut self, update: Result<()>) -> Result<()> {
        match (update, self.run()) {
            (Ok(()), restart) => restart,
            (Err(error), Ok(())) => Err(error),
            (Err(update), Err(restart)) => Err(Error::generic(&format!(
                "{update}; cannot restart native watcher: {restart}"
            ))),
        }
    }

    fn stop(&mut self) {
        drop(self.stream.take());
    }

    fn remove_path(&mut self, path: &Path) -> Result<()> {
        let canonical_path = canonical_watch_path(path)?;
        let cf_path = native_watch_path(&canonical_path)?;
        unsafe {
            let mut to_remove = Vec::new();
            for idx in 0..cf::CFArrayGetCount(self.paths.as_ptr()) {
                let item = cf::CFArrayGetValueAtIndex(self.paths.as_ptr(), idx);
                if fs::CFStringCompare(item, cf_path.as_ptr(), fs::CF_COMPARE_CASE_INSENSITIVE) == 0
                {
                    to_remove.push(idx);
                }
            }

            for idx in to_remove.iter().rev() {
                fs::CFArrayRemoveValueAtIndex(self.paths.as_ptr(), *idx);
            }
        }
        match self.recursive_info.remove(&canonical_path) {
            Some(_) => Ok(()),
            None => Err(Error::watch_not_found()),
        }
    }

    // https://github.com/thibaudgg/rb-fsevent/blob/master/ext/fsevent_watch/main.c
    fn append_path(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        let canonical_path = path.canonicalize()?;
        let cf_path = native_watch_path(&canonical_path)?;
        unsafe {
            fs::CFArrayAppendValue(self.paths.as_ptr(), cf_path.as_ptr());
        }
        self.recursive_info
            .insert(canonical_path, recursive_mode.is_recursive());
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        if unsafe { cf::CFArrayGetCount(self.paths.as_ptr()) } == 0 {
            return Ok(());
        }

        let queue = EventQueue::new()?;
        let context = Arc::new(StreamContextInfo {
            active: AtomicBool::new(true),
            event_handler: self.event_handler.clone(),
            recursive_info: self.recursive_info.clone(),
        });

        // The native stream can outlive invalidation, so its context must not own the Rust handler.
        let callback_context = Arc::new(Arc::downgrade(&context));
        let mut stream_context = fs::FSEventStreamContext {
            version: 0,
            info: Arc::as_ptr(&callback_context).cast_mut().cast(),
            retain: Some(retain_context),
            release: Some(release_context),
            copy_description: None,
        };

        let stream = unsafe {
            fs::FSEventStreamCreate(
                ptr::null(),
                callback,
                &mut stream_context,
                self.paths.as_ptr(),
                fs::EVENT_ID_SINCE_NOW,
                0.0,
                fs::CREATE_FLAG_FILE_EVENTS | fs::CREATE_FLAG_NO_DEFER,
            )
        };

        if stream.is_null() {
            return Err(Error::generic(
                "Cannot create native filesystem event stream",
            ));
        }
        unsafe {
            fs::FSEventStreamSetDispatchQueue(stream, queue.0);
        }
        let mut stream = EventStream {
            pointer: stream,
            queue,
            started: false,
            context,
        };
        if unsafe { fs::FSEventStreamStart(stream.pointer) } == 0 {
            return Err(Error::generic(
                "Cannot start native filesystem event stream",
            ));
        }
        stream.started = true;
        self.stream = Some(stream);

        Ok(())
    }
}

extern "C" fn callback(
    _stream_ref: *const libc::c_void,
    info: *mut libc::c_void,
    num_events: libc::size_t,
    event_paths: *mut libc::c_void,
    event_flags: *const fs::FSEventStreamEventFlags,
    _event_ids: *const fs::FSEventStreamEventId,
) {
    let context = unsafe { &*info.cast::<Weak<StreamContextInfo>>() };
    let Some(info) = context.upgrade() else {
        return;
    };
    if num_events == 0 {
        return;
    }
    let paths = unsafe {
        std::slice::from_raw_parts(event_paths.cast::<*const libc::c_char>(), num_events)
    };
    let flags = unsafe { std::slice::from_raw_parts(event_flags, num_events) };
    for (&path, &flags) in paths.iter().zip(flags) {
        if !info.active.load(Ordering::Acquire) {
            break;
        }
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(
            unsafe { CStr::from_ptr(path) }.to_bytes(),
        ));
        let flags = StreamFlags::from_bits_retain(flags);
        let watched = info.recursive_info.iter().any(|(root, recursive)| {
            path.starts_with(root)
                && (*recursive || &path == root || path.parent() == Some(root.as_path()))
        });
        if !watched {
            continue;
        }
        log::trace!("FSEvent: path = `{}`, flag = {:?}", path.display(), flags);
        for event in translate_flags(flags) {
            if !info.active.load(Ordering::Acquire) {
                break;
            }
            info.event_handler
                .lock()
                .expect("File watch event handler lock poisoned")
                .handle_event(Ok(event.add_path(path.clone())));
        }
    }
}

impl Watcher for FsEventWatcher {
    fn new<F: EventHandler>(event_handler: F) -> Result<Self> {
        Self::from_event_handler(Arc::new(Mutex::new(event_handler)))
    }

    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        self.watch_inner(path, recursive_mode)
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.unwatch_inner(path)
    }
}

impl Drop for FsEventWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/file_events/fsevent.rs"]
mod tests;
