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
        let polled = poll.poll(&mut events, Some(timeout));
        wake_after(
            polled.is_ok(),
            !events.is_empty(),
            self.flag.load(Ordering::SeqCst),
        )
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

/// Calls `call` again while the kernel interrupts it (`EINTR`), and returns its first other result.
fn retry_interrupted<T>(mut call: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match call() {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

/// What a wait that ended means (TH-2): an event or the flag is a wake; a timeout with neither is `TimedOut`; a failed wait
/// (an interrupted one) is a spurious wake, which TH-2 allows.
fn wake_after(polled_ok: bool, any_event: bool, flagged: bool) -> Wake {
    if !polled_ok || any_event || flagged {
        Wake::Woken
    } else {
        Wake::TimedOut
    }
}

/// The readiness that a link needs: what the engine can read and what waits to be written (plan 2.5).
fn interest_of(read: bool, write: bool) -> Option<Interest> {
    match (read, write) {
        (true, true) => Some(Interest::READABLE | Interest::WRITABLE),
        (true, false) => Some(Interest::READABLE),
        (false, true) => Some(Interest::WRITABLE),
        (false, false) => None,
    }
}

/// What to do with the registration of a link whose interest is `interest` and that is `registered` or not.
#[derive(Debug, PartialEq, Eq)]
enum Registration {
    Reregister(Interest),
    Register(Interest),
    Deregister,
    Keep,
}

fn registration(interest: Option<Interest>, registered: bool) -> Registration {
    match (interest, registered) {
        (Some(i), true) => Registration::Reregister(i),
        (Some(i), false) => Registration::Register(i),
        (None, true) => Registration::Deregister,
        (None, false) => Registration::Keep,
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
        let token = Token(link.0 as usize);
        match registration(interest_of(self.read, self.write), self.registered) {
            Registration::Reregister(i) => {
                let _ = registry.reregister(&mut self.stream, token, i);
            }
            Registration::Register(i) => {
                if registry.register(&mut self.stream, token, i).is_ok() {
                    self.registered = true;
                }
            }
            Registration::Deregister => {
                let _ = registry.deregister(&mut self.stream);
                self.registered = false;
            }
            Registration::Keep => {}
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
            match retry_interrupted(|| self.listener.accept()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A call that the kernel interrupts is made again; any other result, an error included, is returned at once.
    #[test]
    fn an_interrupted_call_is_made_again_and_other_results_return() {
        let mut calls = 0;
        let result: io::Result<()> = retry_interrupted(|| {
            calls += 1;
            match calls {
                1 => Err(io::Error::from(io::ErrorKind::Interrupted)),
                2 => Err(io::Error::from(io::ErrorKind::WouldBlock)),
                _ => panic!("called again after an error that is not an interruption"),
            }
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert_eq!(calls, 2);
        assert_eq!(retry_interrupted(|| Ok::<u8, io::Error>(7)).unwrap(), 7);
    }

    /// TH-2: a wait is a wake when it was interrupted, when an event came, or when the flag is set; only a quiet timeout is
    /// `TimedOut`.
    #[test]
    fn a_wait_ends_as_a_wake_or_a_timeout() {
        assert_eq!(wake_after(true, false, false), Wake::TimedOut);
        assert_eq!(wake_after(true, true, false), Wake::Woken);
        assert_eq!(wake_after(true, false, true), Wake::Woken);
        assert_eq!(wake_after(false, false, false), Wake::Woken);
    }

    /// Plan 2.5: the interest of a link follows what the engine can read and what waits to be written, and the registration
    /// follows the interest.
    #[test]
    fn a_link_registers_for_exactly_what_it_needs() {
        assert_eq!(
            interest_of(true, true),
            Some(Interest::READABLE | Interest::WRITABLE)
        );
        assert_eq!(interest_of(true, false), Some(Interest::READABLE));
        assert_eq!(interest_of(false, true), Some(Interest::WRITABLE));
        assert_eq!(interest_of(false, false), None);
        let r = Interest::READABLE;
        assert_eq!(registration(Some(r), true), Registration::Reregister(r));
        assert_eq!(registration(Some(r), false), Registration::Register(r));
        assert_eq!(registration(None, true), Registration::Deregister);
        assert_eq!(registration(None, false), Registration::Keep);
    }
}

/// The tests that open real edges (a data directory, a socket, a poll) start a process that links the whole crate, which is
/// slow: they run in the slow tier (BUILD.md testing rule 2).
#[cfg(test)]
#[cfg(feature = "slow")]
mod slow_tests {
    use super::*;

    fn edges() -> (RealEdges, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("d");
        let data = DataDir::open(&dir).unwrap();
        let (edges, epoch) = RealEdges::new(data, &dir).unwrap();
        assert_eq!(epoch, 1);
        (edges, tmp)
    }

    fn connect(tmp: &tempfile::TempDir) -> std::os::unix::net::UnixStream {
        std::os::unix::net::UnixStream::connect(socket_path(&tmp.path().join("d"))).unwrap()
    }

    fn accept(edges: &mut RealEdges) -> LinkId {
        // A connect on a Unix socket is queued at once; the accept sees it.
        edges.accept_link().expect("a client is waiting")
    }

    /// Plan 2.3: the rows go through the registry of the data directory: write, read by prefix, delete.
    #[test]
    fn the_edges_keep_rows_in_the_registry() {
        let (mut edges, _tmp) = edges();
        edges.write_row("session/a", b"1").unwrap();
        edges.write_row("session/b", b"2").unwrap();
        edges.write_row("other/c", b"3").unwrap();
        assert_eq!(
            edges.read_rows("session/").unwrap(),
            vec![
                ("session/a".to_string(), b"1".to_vec()),
                ("session/b".to_string(), b"2".to_vec())
            ]
        );
        edges.delete_row("session/a").unwrap();
        assert_eq!(edges.read_rows("session/").unwrap().len(), 1);
    }

    /// Plan 2.3: random bytes come from the system; the descriptor handoff is not built yet and says so.
    #[test]
    fn the_edges_draw_random_bytes_and_refuse_the_handoff() {
        let (mut edges, _tmp) = edges();
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        edges.fill_random(&mut a);
        edges.fill_random(&mut b);
        assert_ne!(a, [0; 32]);
        assert_ne!(a, b);
        assert!(edges.poll_process_exit().is_none());
        assert!(edges.accept_link().is_none(), "no client waits");
        let endpoint = StreamEndpoint::new(());
        let options = AttachOptions {
            file_directory: "/tmp".into(),
            file_permissions: None,
            route_features: vec![],
            terminal_formats: vec![],
            connect_deadline: None,
            owner: None,
            query_deadline: None,
            route_tag: None,
            route_limits: None,
            history: None,
            stall_deadline: None,
            answers_queries: false,
            input: true,
        };
        assert!(edges
            .handoff_route(LinkId(1), RouteId(1), endpoint, &options)
            .is_err());
    }

    /// Plan 2.5: a client that connects is a link with its own number; the host reads what it writes, sends what it is
    /// given, follows the interest that the engine sets, and closing the link ends the stream for the client.
    #[test]
    fn a_link_reads_writes_and_closes() {
        use std::io::{Read, Write};
        let (mut edges, tmp) = edges();
        let mut client1 = connect(&tmp);
        let mut client2 = connect(&tmp);
        let first = accept(&mut edges);
        let second = accept(&mut edges);
        assert_eq!((first, second), (LinkId(1), LinkId(2)));
        client1.write_all(b"abc").unwrap();
        let mut buf = [0u8; 8];
        let n = edges.link_recv(first, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"abc");
        assert_eq!(
            edges.link_recv(LinkId(99), &mut buf).unwrap(),
            0,
            "no such link: closed"
        );
        assert_eq!(edges.link_send(second, b"xyz").unwrap(), 3);
        let mut got = [0u8; 3];
        client2.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"xyz");
        assert!(edges.link_send(LinkId(99), b"x").is_err());
        // The interest follows the engine: write on, read off, then both off, then read on again.
        edges.set_write_interest(first, true);
        assert!(edges.streams[&first].write && edges.streams[&first].read);
        edges.set_read_interest(first, false);
        assert!(edges.streams[&first].write && !edges.streams[&first].read);
        edges.set_write_interest(first, false);
        assert!(
            !edges.streams[&first].registered,
            "no interest: not registered"
        );
        edges.set_read_interest(first, true);
        assert!(edges.streams[&first].registered);
        edges.link_close(first);
        assert!(!edges.streams.contains_key(&first));
        let mut rest = Vec::new();
        client1.read_to_end(&mut rest).unwrap();
        assert!(rest.is_empty(), "the client sees the end of the stream");
    }

    /// LC-2, LC-12: the control socket is at its name while the edges live and gone when they drop.
    #[test]
    fn the_socket_lives_with_the_edges() {
        let (edges, tmp) = edges();
        let socket = socket_path(&tmp.path().join("d"));
        assert_eq!(socket, tmp.path().join("d").join("c"));
        assert!(socket.exists());
        drop(edges);
        assert!(!socket.exists());
    }

    /// TM-6, TH-2: the wake flag is set by `signal` and cleared by `drain`; a set flag makes a wait return at once; the handle
    /// has a descriptor of its own.
    #[test]
    fn the_wake_follows_its_flag() {
        let (edges, _tmp) = edges();
        let wake = &edges.wake;
        assert_eq!(wake.wait(Duration::ZERO), Wake::TimedOut);
        wake.signal();
        assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
        wake.drain();
        // The readiness of the self-pipe is still queued: `consume` takes it without waiting, and a quiet wait times out.
        wake.consume();
        assert_eq!(wake.wait(Duration::ZERO), Wake::TimedOut);
        assert!(wake.fd() > 2, "the descriptor of the poll");
    }
}
