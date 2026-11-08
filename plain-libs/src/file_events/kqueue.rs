use super::event::*;
use super::{Error, EventHandler, RecursiveMode, Result, Watcher};
use crate::file_events::{Receiver, Sender, unbounded};
use std::collections::HashMap;
use std::env;
use std::fs::{File, metadata};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use walkdir::WalkDir;

const KQUEUE: mio::Token = mio::Token(0);
const MESSAGE: mio::Token = mio::Token(1);

const VNODE_FLAGS: &[u32] = &[
    libc::NOTE_WRITE,
    libc::NOTE_EXTEND,
    libc::NOTE_ATTRIB,
    libc::NOTE_LINK,
    libc::NOTE_RENAME,
    libc::NOTE_REVOKE,
    #[cfg(target_os = "freebsd")]
    libc::NOTE_CLOSE_WRITE,
    #[cfg(target_os = "freebsd")]
    libc::NOTE_CLOSE,
    #[cfg(target_os = "freebsd")]
    libc::NOTE_OPEN,
    #[cfg(target_os = "freebsd")]
    libc::NOTE_READ,
    #[cfg(target_os = "openbsd")]
    libc::NOTE_TRUNCATE,
    libc::NOTE_DELETE,
];

// The EventLoop will set up a mio::Poll and use it to wait for the following:
//
// -  messages telling it what to do
//
// -  events telling it that something has happened on one of the watched files.
struct EventLoop {
    running: bool,
    poll: mio::Poll,
    event_loop_waker: Arc<mio::Waker>,
    event_loop_tx: Sender<EventLoopMsg>,
    event_loop_rx: Receiver<EventLoopMsg>,
    kqueue: OwnedFd,
    event_handler: Box<dyn EventHandler>,
    watches: HashMap<PathBuf, (File, bool)>,
}

#[derive(Debug)]
pub struct KqueueWatcher {
    channel: Sender<EventLoopMsg>,
    waker: Arc<mio::Waker>,
}

enum EventLoopMsg {
    AddWatch(PathBuf, RecursiveMode, Sender<Result<()>>),
    RemoveWatch(PathBuf, Sender<Result<()>>),
    Shutdown,
}

impl EventLoop {
    pub fn new(kqueue: OwnedFd, event_handler: Box<dyn EventHandler>) -> Result<Self> {
        let (event_loop_tx, event_loop_rx) = unbounded::<EventLoopMsg>();
        let poll = mio::Poll::new()?;

        let event_loop_waker = Arc::new(mio::Waker::new(poll.registry(), MESSAGE)?);

        let kqueue_fd = kqueue.as_raw_fd();
        let mut evented_kqueue = mio::unix::SourceFd(&kqueue_fd);
        poll.registry()
            .register(&mut evented_kqueue, KQUEUE, mio::Interest::READABLE)?;

        let event_loop = EventLoop {
            running: true,
            poll,
            event_loop_waker,
            event_loop_tx,
            event_loop_rx,
            kqueue,
            event_handler,
            watches: HashMap::new(),
        };
        Ok(event_loop)
    }

    // Run the event loop.
    pub fn run(self) {
        let _ = thread::Builder::new()
            .name("notify-rs kqueue loop".to_string())
            .spawn(|| self.event_loop_thread());
    }

    fn event_loop_thread(mut self) {
        let mut events = mio::Events::with_capacity(16);
        loop {
            // Wait for something to happen.
            match self.poll.poll(&mut events, None) {
                Err(ref e) if matches!(e.kind(), std::io::ErrorKind::Interrupted) => {
                    // System call was interrupted, we will retry
                    // TODO: Not covered by tests (to reproduce likely need to setup signal handlers)
                }
                Err(e) => panic!("poll failed: {}", e),
                Ok(()) => {}
            }

            // Process whatever happened.
            for event in &events {
                self.handle_event(event);
            }

            // Stop, if we're done.
            if !self.running {
                break;
            }
        }
    }

    // Handle a single event.
    fn handle_event(&mut self, event: &mio::event::Event) {
        match event.token() {
            MESSAGE => {
                // The channel is readable - handle messages.
                self.handle_messages()
            }
            KQUEUE => {
                // inotify has something to tell us.
                self.handle_kqueue()
            }
            _ => unreachable!(),
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.event_loop_rx.try_recv() {
            match msg {
                EventLoopMsg::AddWatch(path, recursive_mode, tx) => {
                    let _ = tx.send(self.add_watch(path, recursive_mode.is_recursive()));
                }
                EventLoopMsg::RemoveWatch(path, tx) => {
                    let _ = tx.send(self.remove_watch(path, false));
                }
                EventLoopMsg::Shutdown => {
                    self.running = false;
                    break;
                }
            }
        }
    }

    fn handle_kqueue(&mut self) {
        let mut add_watches = Vec::new();
        let mut remove_watches = Vec::new();

        let timeout = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        loop {
            let mut event = std::mem::MaybeUninit::<libc::kevent>::uninit();
            let count = unsafe {
                libc::kevent(
                    self.kqueue.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    event.as_mut_ptr(),
                    1,
                    &timeout,
                )
            };
            if count == 0 {
                break;
            }
            if count < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                self.event_handler.handle_event(Err(error.into()));
                break;
            }
            let event = unsafe { event.assume_init() };
            if event.flags & libc::EV_ERROR != 0 {
                self.event_handler
                    .handle_event(Err(io::Error::from_raw_os_error(event.data as i32).into()));
                continue;
            }
            let Some(path) = self.watches.iter().find_map(|(path, (file, _))| {
                (file.as_raw_fd() as usize == event.ident as usize).then_some(path)
            }) else {
                continue;
            };
            for &flag in VNODE_FLAGS {
                if event.fflags & flag == 0 {
                    continue;
                }
                let kind = match flag {
                    libc::NOTE_WRITE if path.is_dir() => {
                        match std::fs::read_dir(path) {
                            Ok(entries) => {
                                let new_file = entries
                                    .filter_map(std::result::Result::ok)
                                    .map(|entry| entry.path())
                                    .find(|file| !self.watches.contains_key(file));
                                if let Some(file) = new_file {
                                    let kind = if file.is_dir() {
                                        CreateKind::Folder
                                    } else if file.is_file() {
                                        CreateKind::File
                                    } else {
                                        CreateKind::Other
                                    };
                                    add_watches.push(file.clone());
                                    self.event_handler
                                        .handle_event(Ok(
                                            Event::new(EventKind::Create(kind)).add_path(file)
                                        ));
                                    continue;
                                }
                            }
                            Err(error) => {
                                self.event_handler.handle_event(Err(error.into()));
                                continue;
                            }
                        }
                        EventKind::Modify(ModifyKind::Data(DataChange::Any))
                    }
                    libc::NOTE_WRITE => EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                    libc::NOTE_EXTEND => EventKind::Modify(ModifyKind::Data(DataChange::Size)),
                    #[cfg(target_os = "openbsd")]
                    libc::NOTE_TRUNCATE => EventKind::Modify(ModifyKind::Data(DataChange::Size)),
                    libc::NOTE_ATTRIB => EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
                    libc::NOTE_LINK => {
                        remove_watches.push(path.clone());
                        add_watches.push(path.clone());
                        EventKind::Modify(ModifyKind::Any)
                    }
                    libc::NOTE_RENAME => {
                        remove_watches.push(path.clone());
                        EventKind::Modify(ModifyKind::Name(RenameMode::Any))
                    }
                    libc::NOTE_DELETE | libc::NOTE_REVOKE => {
                        remove_watches.push(path.clone());
                        EventKind::Remove(RemoveKind::Any)
                    }
                    _ => {
                        self.event_handler
                            .handle_event(Ok(Event::new(EventKind::Other)));
                        continue;
                    }
                };
                self.event_handler
                    .handle_event(Ok(Event::new(kind).add_path(path.clone())));
            }
        }

        for path in remove_watches {
            self.remove_watch(path, true).ok();
        }

        for path in add_watches {
            self.add_watch(path, true).ok();
        }
    }

    fn add_watch(&mut self, path: PathBuf, is_recursive: bool) -> Result<()> {
        let recursive = is_recursive && metadata(&path).map_err(Error::io)?.is_dir();
        let mut files = Vec::new();
        let open = |path: PathBuf| {
            File::open(&path)
                .map(|file| (path.clone(), file))
                .map_err(|error| Error::io(error).add_path(path))
        };
        if recursive {
            for entry in WalkDir::new(path).follow_links(true) {
                files.push(open(entry.map_err(map_walkdir_error)?.into_path())?);
            }
        } else {
            files.push(open(path)?);
        }
        let changes: Vec<libc::kevent> = files
            .iter()
            .map(|(_, file)| {
                let mut event: libc::kevent = unsafe { std::mem::zeroed() };
                event.ident = file.as_raw_fd() as _;
                event.filter = libc::EVFILT_VNODE;
                event.flags = libc::EV_ADD | libc::EV_CLEAR;
                event.fflags = libc::NOTE_DELETE
                    | libc::NOTE_WRITE
                    | libc::NOTE_EXTEND
                    | libc::NOTE_ATTRIB
                    | libc::NOTE_LINK
                    | libc::NOTE_RENAME
                    | libc::NOTE_REVOKE;
                event
            })
            .collect();
        let count = changes
            .len()
            .try_into()
            .map_err(|_| Error::generic("Too many kqueue watches"))?;
        if unsafe {
            libc::kevent(
                self.kqueue.as_raw_fd(),
                changes.as_ptr(),
                count,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error().into());
        }
        self.watches.extend(
            files
                .into_iter()
                .map(|(path, file)| (path, (file, recursive))),
        );
        Ok(())
    }

    fn remove_watch(&mut self, path: PathBuf, remove_recursive: bool) -> Result<()> {
        let (_, recursive) = self
            .watches
            .remove(&path)
            .ok_or_else(Error::watch_not_found)?;
        if recursive || remove_recursive {
            self.watches.retain(|watch, _| !watch.starts_with(&path));
        }
        Ok(())
    }
}

fn map_walkdir_error(e: walkdir::Error) -> Error {
    if e.io_error().is_some() {
        // save to unwrap otherwise we whouldn't be in this branch
        Error::io(e.into_io_error().unwrap())
    } else {
        Error::generic(&e.to_string())
    }
}

impl KqueueWatcher {
    fn from_event_handler(event_handler: Box<dyn EventHandler>) -> Result<Self> {
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error().into());
        }
        let kqueue = unsafe { OwnedFd::from_raw_fd(fd) };
        if unsafe { libc::fcntl(kqueue.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        let event_loop = EventLoop::new(kqueue, event_handler)?;
        let channel = event_loop.event_loop_tx.clone();
        let waker = event_loop.event_loop_waker.clone();
        event_loop.run();
        Ok(KqueueWatcher { channel, waker })
    }

    fn watch_inner(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        let pb = if path.is_absolute() {
            path.to_owned()
        } else {
            let p = env::current_dir().map_err(Error::io)?;
            p.join(path)
        };
        let (tx, rx) = unbounded();
        let msg = EventLoopMsg::AddWatch(pb, recursive_mode, tx);

        self.channel
            .send(msg)
            .map_err(|e| Error::generic(&e.to_string()))?;
        self.waker
            .wake()
            .map_err(|e| Error::generic(&e.to_string()))?;
        rx.recv()
            .unwrap()
            .map_err(|e| Error::generic(&e.to_string()))
    }

    fn unwatch_inner(&mut self, path: &Path) -> Result<()> {
        let pb = if path.is_absolute() {
            path.to_owned()
        } else {
            let p = env::current_dir().map_err(Error::io)?;
            p.join(path)
        };
        let (tx, rx) = unbounded();
        let msg = EventLoopMsg::RemoveWatch(pb, tx);

        self.channel
            .send(msg)
            .map_err(|e| Error::generic(&e.to_string()))?;
        self.waker
            .wake()
            .map_err(|e| Error::generic(&e.to_string()))?;
        rx.recv()
            .unwrap()
            .map_err(|e| Error::generic(&e.to_string()))
    }
}

impl Watcher for KqueueWatcher {
    fn new<F: EventHandler>(event_handler: F) -> Result<Self> {
        Self::from_event_handler(Box::new(event_handler))
    }

    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        self.watch_inner(path, recursive_mode)
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.unwatch_inner(path)
    }
}

impl Drop for KqueueWatcher {
    fn drop(&mut self) {
        // we expect the event loop to live => unwrap must not panic
        self.channel.send(EventLoopMsg::Shutdown).unwrap();
        self.waker.wake().unwrap();
    }
}
