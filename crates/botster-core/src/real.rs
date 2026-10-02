//! The real edges of the host driver (plan 2.3, 2.5): the registry on disk, the OS random generator, worker processes, the
//! control socket and the wake object.

use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, ProcessIdentity, SpawnError, SpawnSpec, Storage, StorageError,
};
use botster_core_edges::scheduler::Production;
use botster_core_edges::{Entropy, Scheduler, Wake as WakeEdge};
use botster_core_host::driver::{HandoffError, HostEdges, HostWake, WorkerSpawn};
use botster_core_host::LinkId;
use botster_core_link::launch::WorkerLaunch;
use botster_core_sys::entropy::OsEntropy;
use botster_core_sys::process::Children;
use botster_core_sys::storage::{DataDir, FileStorage};
use mio::net::{UnixListener, UnixStream};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Registry, Token, Waker};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The token of the self-pipe (TM-6).
const WAKER: Token = Token(usize::MAX);
/// The token of the control listener.
const LISTENER: Token = Token(usize::MAX - 1);

/// The wake object: a `mio` poll over every descriptor that can bring host work, plus a self-pipe for runnable work
/// (plan 2.5). It starts no thread. `wait` may run on any thread while the owner thread registers descriptors (TH-2), because
/// the registry is separate from the poll.
///
/// Why `mio` and not `polling` (plan 7.2): `polling` registers a descriptor through an `unsafe` function, and this workspace
/// forbids unsafe code. `mio` registers a `SourceFd` safely, and `mio` is adopted for the worker drivers already.
pub struct PollWake {
    poll: Mutex<Poll>,
    registry: Registry,
    waker: Waker,
    flag: AtomicBool,
    /// The descriptor of the poll, for `WakeHandle::fd` (section 13 rule 5): a kqueue or epoll descriptor is pollable.
    fd: RawFd,
}

impl PollWake {
    fn new() -> io::Result<PollWake> {
        let poll = Poll::new()?;
        let registry = poll.registry().try_clone()?;
        let waker = Waker::new(&registry, WAKER)?;
        let fd = poll.as_raw_fd();
        Ok(PollWake {
            poll: Mutex::new(poll),
            registry,
            waker,
            flag: AtomicBool::new(false),
            fd,
        })
    }

    /// Consumes the readiness that is queued, without waiting. The edge-triggered readiness of `mio` is gone after this, so
    /// the driver reads every link once more (plan 2.5).
    fn consume(&self) {
        if let Ok(mut poll) = self.poll.try_lock() {
            let mut events = Events::with_capacity(64);
            let _ = poll.poll(&mut events, Some(Duration::ZERO));
        }
    }
}

impl WakeHandle for PollWake {
    fn wait(&self, timeout: Duration) -> Wake {
        if self.flag.load(Ordering::SeqCst) {
            return Wake::Woken;
        }
        let Ok(mut poll) = self.poll.lock() else {
            return Wake::Woken;
        };
        let mut events = Events::with_capacity(64);
        match poll.poll(&mut events, Some(timeout)) {
            Ok(()) if !events.is_empty() => Wake::Woken,
            Ok(()) if self.flag.load(Ordering::SeqCst) => Wake::Woken,
            Ok(()) => Wake::TimedOut,
            // An interrupted wait is a spurious wake, which TH-2 allows.
            Err(_) => Wake::Woken,
        }
    }

    fn fd(&self) -> RawFd {
        self.fd
    }
}

impl WakeEdge for PollWake {
    fn signal(&self) {
        self.flag.store(true, Ordering::SeqCst);
        let _ = self.waker.wake();
    }

    fn drain(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

/// The path of the control socket inside the data directory. A Unix socket path is limited to about 100 bytes, so the name is
/// short.
fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join("c")
}

/// One accepted control link and what is registered for it (plan 2.5: read interest follows what the engine can take, write
/// interest follows the outbound buffer).
struct LinkIo {
    stream: UnixStream,
    read: bool,
    write: bool,
    registered: bool,
}

impl LinkIo {
    fn apply(&mut self, registry: &Registry, link: LinkId) {
        let interest = match (self.read, self.write) {
            (true, true) => Some(Interest::READABLE | Interest::WRITABLE),
            (true, false) => Some(Interest::READABLE),
            (false, true) => Some(Interest::WRITABLE),
            (false, false) => None,
        };
        let token = Token(link.0 as usize);
        match (interest, self.registered) {
            (Some(i), true) => {
                let _ = registry.reregister(&mut self.stream, token, i);
            }
            (Some(i), false) => {
                if registry.register(&mut self.stream, token, i).is_ok() {
                    self.registered = true;
                }
            }
            (None, true) => {
                let _ = registry.deregister(&mut self.stream);
                self.registered = false;
            }
            (None, false) => {}
        }
    }
}

/// The real edges of one host.
pub struct RealEdges {
    // The data directory holds the lock for as long as the edges live (LC-2).
    storage: FileStorage,
    _lock: botster_core_sys::lock::LockFile,
    entropy: OsEntropy,
    children: Children,
    listener: UnixListener,
    socket: PathBuf,
    streams: BTreeMap<LinkId, LinkIo>,
    next_link: u64,
    wake: Arc<PollWake>,
    scheduler: Production,
}

impl RealEdges {
    /// Opens the edges of a host over an open data directory. The directory's lock and registry move into the edges.
    pub fn new(data: DataDir, data_dir: &Path) -> io::Result<(RealEdges, u64)> {
        let (lock, storage, epoch) = data.into_storage();
        let socket = socket_path(data_dir);
        // A socket file of an earlier host is stale: the lock proves that no host holds the directory now.
        let _ = std::fs::remove_file(&socket);
        let mut listener = UnixListener::bind(&socket)?;
        let wake = Arc::new(PollWake::new()?);
        wake.registry
            .register(&mut listener, LISTENER, Interest::READABLE)?;
        Ok((
            RealEdges {
                storage,
                _lock: lock,
                entropy: OsEntropy,
                // A reaper thread wakes the host when a worker ends (TM-6): the host sees the exit at once, not at a deadline.
                children: Children::with_notify({
                    let wake = Arc::clone(&wake);
                    Arc::new(move || WakeEdge::signal(&*wake))
                }),
                listener,
                socket,
                streams: BTreeMap::new(),
                next_link: 1,
                wake,
                scheduler: Production::new(),
            },
            epoch,
        ))
    }
}

impl Drop for RealEdges {
    fn drop(&mut self) {
        // LC-12: dropping never ends a worker. The socket file goes; the workers keep running.
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn map_storage<T>(result: Result<T, StorageError>) -> Result<T, StorageError> {
    result
}

impl HostEdges for RealEdges {
    fn fill_random(&mut self, buf: &mut [u8]) {
        self.entropy.fill(buf);
    }

    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        map_storage(self.storage.write_row(key, bytes))
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        self.storage.delete_row(key)
    }

    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        let mut rows = Vec::new();
        for key in self.storage.list_rows()? {
            if key.starts_with(prefix) {
                if let Some(bytes) = self.storage.read_row(&key)? {
                    rows.push((key, bytes));
                }
            }
        }
        Ok(rows)
    }

    fn spawn_worker(&mut self, spec: &WorkerSpawn) -> Result<ProcessIdentity, SpawnError> {
        let launch = WorkerLaunch {
            control: self.socket.clone(),
            instance: spec.instance.clone(),
            host_epoch: spec.host_epoch,
            token: spec.token,
        };
        self.children.spawn(&SpawnSpec {
            program: spec.program.clone(),
            args: launch.args(),
            env: launch.env(),
            cwd: None,
        })
    }

    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        self.children.signal_group(identity, signal);
    }

    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        self.children.poll_exit()
    }

    fn accept_link(&mut self) -> Option<LinkId> {
        loop {
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    let link = LinkId(self.next_link);
                    self.next_link += 1;
                    let registered = self.wake.registry.register(
                        &mut stream,
                        Token(link.0 as usize),
                        Interest::READABLE,
                    );
                    if registered.is_ok() {
                        self.streams.insert(
                            link,
                            LinkIo {
                                stream,
                                read: true,
                                write: false,
                                registered: true,
                            },
                        );
                        return Some(link);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            }
        }
    }

    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize> {
        match self.streams.get_mut(&link) {
            Some(io) => io.stream.read(buf),
            None => Ok(0),
        }
    }

    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize> {
        match self.streams.get_mut(&link) {
            Some(io) => io.stream.write(bytes),
            None => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }

    fn link_close(&mut self, link: LinkId) {
        if let Some(mut io) = self.streams.remove(&link) {
            if io.registered {
                let _ = self.wake.registry.deregister(&mut io.stream);
            }
        }
    }

    fn set_write_interest(&mut self, link: LinkId, on: bool) {
        if let Some(io) = self.streams.get_mut(&link) {
            io.write = on;
            io.apply(&self.wake.registry, link);
        }
    }

    fn set_read_interest(&mut self, link: LinkId, on: bool) {
        if let Some(io) = self.streams.get_mut(&link) {
            io.read = on;
            io.apply(&self.wake.registry, link);
        }
    }

    fn handoff_route(
        &mut self,
        _link: LinkId,
        _route: RouteId,
        _transport: StreamEndpoint,
        _options: &AttachOptions,
    ) -> Result<(), HandoffError> {
        // The descriptor handoff over the control link (`SCM_RIGHTS`, DP-2) belongs to the route data plane (P4a). Until it
        // exists, a route closes with the typed `HandoffFailed` (OU-2), and Core closes the endpoint.
        Err(HandoffError)
    }

    fn wake(&self) -> Arc<dyn HostWake> {
        Arc::clone(&self.wake) as Arc<dyn HostWake>
    }

    fn settle_wake(&mut self) {
        self.wake.consume();
    }

    fn scheduler(&mut self) -> &mut dyn Scheduler {
        &mut self.scheduler
    }
}

// `SourceFd` is part of the registration of a descriptor that `mio` does not own; it is kept in the imports for the
// registrations that P3 and P7 add (exit watches, lanes).
#[allow(dead_code)]
fn _source_fd(fd: &RawFd) -> SourceFd<'_> {
    SourceFd(fd)
}
