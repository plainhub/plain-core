use super::event::*;
use super::watch_tree::WatchTree;
use super::{Error, ErrorKind, EventHandler, RecursiveMode, Result, Watcher};
use crate::file_events::{Receiver, Sender, unbounded};
use crate::platform::inotify::{self, RawEvent};
use std::collections::HashMap;
use std::env;
use std::fs::metadata;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

const INOTIFY: mio::Token = mio::Token(0);
const MESSAGE: mio::Token = mio::Token(1);

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
    inotify: Option<OwnedFd>,
    event_handler: Box<dyn EventHandler>,

    watches: HashMap<PathBuf, (i32, u32, bool, bool)>,
    paths: HashMap<i32, PathBuf>,
    rename_source: Option<(u32, Option<PathBuf>)>,
}

#[derive(Debug)]
pub struct INotifyWatcher {
    channel: Sender<EventLoopMsg>,
    waker: Arc<mio::Waker>,
}

enum EventLoopMsg {
    AddWatch(PathBuf, RecursiveMode, Sender<Result<()>>),
    RemoveWatch(PathBuf, Sender<Result<()>>),
    Shutdown,
}

#[inline]
fn add_watch_by_event(
    path: &Option<PathBuf>,
    event: &RawEvent,
    watches: &HashMap<PathBuf, (i32, u32, bool, bool)>,
    add_watches: &mut Vec<PathBuf>,
) {
    if let Some(ref path) = *path {
        if event.has(libc::IN_ISDIR) {
            if let Some(parent_path) = path.parent() {
                if let Some(&(_, _, is_recursive, _)) = watches.get(parent_path) {
                    if is_recursive {
                        add_watches.push(path.to_owned());
                    }
                }
            }
        }
    }
}

#[inline]
fn remove_watch_by_event(
    path: &Option<PathBuf>,
    watches: &HashMap<PathBuf, (i32, u32, bool, bool)>,
    remove_watches: &mut Vec<PathBuf>,
) {
    if let Some(ref path) = *path {
        if watches.contains_key(path) {
            remove_watches.push(path.to_owned());
        }
    }
}

impl EventLoop {
    pub fn new(inotify: OwnedFd, event_handler: Box<dyn EventHandler>) -> Result<Self> {
        let (event_loop_tx, event_loop_rx) = unbounded::<EventLoopMsg>();
        let poll = mio::Poll::new()?;

        let event_loop_waker = Arc::new(mio::Waker::new(poll.registry(), MESSAGE)?);

        let inotify_fd = inotify.as_raw_fd();
        let mut evented_inotify = mio::unix::SourceFd(&inotify_fd);
        poll.registry()
            .register(&mut evented_inotify, INOTIFY, mio::Interest::READABLE)?;

        let event_loop = EventLoop {
            running: true,
            poll,
            event_loop_waker,
            event_loop_tx,
            event_loop_rx,
            inotify: Some(inotify),
            event_handler,
            watches: HashMap::new(),
            paths: HashMap::new(),
            rename_source: None,
        };
        Ok(event_loop)
    }

    // Run the event loop.
    pub fn run(self) {
        let _ = thread::Builder::new()
            .name("notify-rs inotify loop".to_string())
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
            INOTIFY => {
                // inotify has something to tell us.
                self.handle_inotify()
            }
            _ => unreachable!(),
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.event_loop_rx.try_recv() {
            match msg {
                EventLoopMsg::AddWatch(path, recursive_mode, tx) => {
                    let _ = tx.send(self.add_watch(path, recursive_mode.is_recursive(), true));
                }
                EventLoopMsg::RemoveWatch(path, tx) => {
                    let _ = tx.send(self.remove_watch(path, false));
                }
                EventLoopMsg::Shutdown => {
                    let _ = self.remove_all_watches();
                    drop(self.inotify.take());
                    self.running = false;
                    break;
                }
            }
        }
    }

    fn handle_inotify(&mut self) {
        let mut add_watches = Vec::new();
        let mut remove_watches = Vec::new();

        if let Some(ref mut inotify) = self.inotify {
            let mut buffer = [0; 1024];
            // Read all buffers available.
            loop {
                match inotify::read(inotify, &mut buffer) {
                    Ok(events) => {
                        let mut num_events = 0;
                        for event in events {
                            log::trace!("inotify event: {event:?}");

                            num_events += 1;
                            if event.has(libc::IN_Q_OVERFLOW) {
                                let ev = Ok(Event::rescan());
                                self.event_handler.handle_event(ev);
                            }

                            let path = self.paths.get(&event.descriptor).map(|root| {
                                if event.name.is_empty() {
                                    root.clone()
                                } else {
                                    root.join(&event.name)
                                }
                            });

                            let mut evs = Vec::new();

                            if event.has(libc::IN_MOVED_FROM) {
                                remove_watch_by_event(&path, &self.watches, &mut remove_watches);

                                self.rename_source = Some((event.cookie, path.clone()));
                                evs.push(
                                    Event::new(EventKind::Modify(ModifyKind::Name(
                                        RenameMode::From,
                                    )))
                                    .add_some_path(path.clone()),
                                );
                            } else if event.has(libc::IN_MOVED_TO) {
                                evs.push(
                                    Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To)))
                                        .add_some_path(path.clone()),
                                );

                                if let Some((_, source)) = self
                                    .rename_source
                                    .take_if(|(cookie, _)| *cookie == event.cookie)
                                {
                                    evs.push(
                                        Event::new(EventKind::Modify(ModifyKind::Name(
                                            RenameMode::Both,
                                        )))
                                        .add_some_path(source)
                                        .add_some_path(path.clone()),
                                    );
                                }
                                add_watch_by_event(&path, &event, &self.watches, &mut add_watches);
                            }
                            if event.has(libc::IN_MOVE_SELF) {
                                evs.push(
                                    Event::new(EventKind::Modify(ModifyKind::Name(
                                        RenameMode::From,
                                    )))
                                    .add_some_path(path.clone()),
                                );
                                // TODO stat the path and get to new path
                                // - emit To and Both events
                                // - change prefix for further events
                            }
                            if event.has(libc::IN_CREATE) {
                                evs.push(
                                    Event::new(EventKind::Create(if event.has(libc::IN_ISDIR) {
                                        CreateKind::Folder
                                    } else {
                                        CreateKind::File
                                    }))
                                    .add_some_path(path.clone()),
                                );
                                add_watch_by_event(&path, &event, &self.watches, &mut add_watches);
                            }
                            if event.has(libc::IN_DELETE) {
                                evs.push(
                                    Event::new(EventKind::Remove(if event.has(libc::IN_ISDIR) {
                                        RemoveKind::Folder
                                    } else {
                                        RemoveKind::File
                                    }))
                                    .add_some_path(path.clone()),
                                );
                                remove_watch_by_event(&path, &self.watches, &mut remove_watches);
                            }
                            if event.has(libc::IN_DELETE_SELF) {
                                let remove_kind = match &path {
                                    Some(watched_path) => {
                                        let current_watch = self.watches.get(watched_path);
                                        match current_watch {
                                            Some(&(_, _, _, true)) => RemoveKind::Folder,
                                            Some(&(_, _, _, false)) => RemoveKind::File,
                                            None => RemoveKind::Other,
                                        }
                                    }
                                    None => {
                                        log::trace!(
                                            "No patch for DELETE_SELF event, may be a bug?"
                                        );
                                        RemoveKind::Other
                                    }
                                };
                                evs.push(
                                    Event::new(EventKind::Remove(remove_kind))
                                        .add_some_path(path.clone()),
                                );
                                remove_watch_by_event(&path, &self.watches, &mut remove_watches);
                            }
                            if event.has(libc::IN_MODIFY) {
                                evs.push(
                                    Event::new(EventKind::Modify(ModifyKind::Data(
                                        DataChange::Any,
                                    )))
                                    .add_some_path(path.clone()),
                                );
                            }
                            if event.has(libc::IN_CLOSE_WRITE) {
                                evs.push(
                                    Event::new(EventKind::Access(AccessKind::Close(
                                        AccessMode::Write,
                                    )))
                                    .add_some_path(path.clone()),
                                );
                            }
                            if event.has(libc::IN_CLOSE_NOWRITE) {
                                evs.push(
                                    Event::new(EventKind::Access(AccessKind::Close(
                                        AccessMode::Read,
                                    )))
                                    .add_some_path(path.clone()),
                                );
                            }
                            if event.has(libc::IN_ATTRIB) {
                                evs.push(
                                    Event::new(EventKind::Modify(ModifyKind::Metadata(
                                        MetadataKind::Any,
                                    )))
                                    .add_some_path(path.clone()),
                                );
                            }
                            if event.has(libc::IN_OPEN) {
                                evs.push(
                                    Event::new(EventKind::Access(AccessKind::Open(
                                        AccessMode::Any,
                                    )))
                                    .add_some_path(path.clone()),
                                );
                            }

                            for ev in evs {
                                self.event_handler.handle_event(Ok(ev));
                            }
                        }

                        // All events read. Break out.
                        if num_events == 0 {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No events read. Break out.
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        self.event_handler.handle_event(Err(Error::io(e)));
                        break;
                    }
                }
            }
        }

        for path in remove_watches {
            self.remove_watch(path, true).ok();
        }

        for path in add_watches {
            self.add_watch(path, true, false).ok();
        }
    }

    fn add_watch(&mut self, path: PathBuf, is_recursive: bool, mut watch_self: bool) -> Result<()> {
        // If the watch is not recursive, or if we determine (by stat'ing the path to get its
        // metadata) that the watched path is not a directory, add a single path watch.
        if !is_recursive || !metadata(&path).map_err(Error::io_watch)?.is_dir() {
            return self.add_single_watch(path, false, true);
        }

        for entry in WatchTree::new(path).flatten() {
            if entry.metadata().is_ok_and(|metadata| metadata.is_dir()) {
                self.add_single_watch(entry.path, is_recursive, watch_self)?;
                watch_self = false;
            }
        }

        Ok(())
    }

    fn add_single_watch(
        &mut self,
        path: PathBuf,
        is_recursive: bool,
        watch_self: bool,
    ) -> Result<()> {
        let mut watchmask = libc::IN_ATTRIB
            | libc::IN_CREATE
            | libc::IN_OPEN
            | libc::IN_DELETE
            | libc::IN_CLOSE_WRITE
            | libc::IN_MODIFY
            | libc::IN_MOVED_FROM
            | libc::IN_MOVED_TO;

        if watch_self {
            watchmask |= libc::IN_DELETE_SELF;
            watchmask |= libc::IN_MOVE_SELF;
        }

        if let Some(&(_, old_watchmask, _, _)) = self.watches.get(&path) {
            watchmask |= old_watchmask;
            watchmask |= libc::IN_MASK_ADD;
        }

        if let Some(ref mut inotify) = self.inotify {
            log::trace!("adding inotify watch: {}", path.display());

            match inotify::add(inotify, &path, watchmask) {
                Err(e) => {
                    Err(if e.raw_os_error() == Some(libc::ENOSPC) {
                        // do not report inotify limits as "no more space" on linux #266
                        Error::new(ErrorKind::MaxFilesWatch)
                    } else {
                        Error::io(e)
                    }
                    .add_path(path))
                }
                Ok(w) => {
                    watchmask &= !libc::IN_MASK_ADD;
                    let is_dir = metadata(&path).map_err(Error::io)?.is_dir();
                    self.watches
                        .insert(path.clone(), (w, watchmask, is_recursive, is_dir));
                    self.paths.insert(w, path);
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    fn remove_watch(&mut self, path: PathBuf, remove_recursive: bool) -> Result<()> {
        match self.watches.remove(&path) {
            None => return Err(Error::watch_not_found().add_path(path)),
            Some((w, _, is_recursive, _)) => {
                if let Some(ref mut inotify) = self.inotify {
                    log::trace!("removing inotify watch: {}", path.display());

                    inotify::remove(inotify, w).map_err(|e| Error::io(e).add_path(path.clone()))?;
                    self.paths.remove(&w);

                    if is_recursive || remove_recursive {
                        let mut remove_list = Vec::new();
                        for (w, p) in &self.paths {
                            if p.starts_with(&path) {
                                inotify::remove(inotify, *w)
                                    .map_err(|e| Error::io(e).add_path(p.into()))?;
                                self.watches.remove(p);
                                remove_list.push(*w);
                            }
                        }
                        for w in remove_list {
                            self.paths.remove(&w);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn remove_all_watches(&mut self) -> Result<()> {
        if let Some(ref mut inotify) = self.inotify {
            for (w, p) in &self.paths {
                inotify::remove(inotify, *w).map_err(|e| Error::io(e).add_path(p.into()))?;
            }
            self.watches.clear();
            self.paths.clear();
        }
        Ok(())
    }
}

impl INotifyWatcher {
    fn from_event_handler(event_handler: Box<dyn EventHandler>) -> Result<Self> {
        let inotify = inotify::open()?;
        let event_loop = EventLoop::new(inotify, event_handler)?;
        let channel = event_loop.event_loop_tx.clone();
        let waker = event_loop.event_loop_waker.clone();
        event_loop.run();
        Ok(INotifyWatcher { channel, waker })
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

        // we expect the event loop to live and reply => unwraps must not panic
        self.channel.send(msg).unwrap();
        self.waker.wake().unwrap();
        rx.recv().unwrap()
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

        // we expect the event loop to live and reply => unwraps must not panic
        self.channel.send(msg).unwrap();
        self.waker.wake().unwrap();
        rx.recv().unwrap()
    }
}

impl Watcher for INotifyWatcher {
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

impl Drop for INotifyWatcher {
    fn drop(&mut self) {
        // we expect the event loop to live => unwrap must not panic
        self.channel.send(EventLoopMsg::Shutdown).unwrap();
        self.waker.wake().unwrap();
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/file_events/inotify.rs"]
mod tests;
