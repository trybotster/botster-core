//! The real edges of the host driver (plan 2.3, 2.5): the registry on disk, the OS random generator, worker processes, the
//! control socket and the wake object.

use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, SpawnSpec, Storage,
    StorageError,
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
use mio::{Events, Interest, Poll, Registry, Token, Waker};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The token of the self-pipe (TM-6). The tokens only register descriptors: no event is dispatched by its token (`wait`
/// takes any event as a wake, and `pump` services the listener and every link). They are still distinct: the listener is
/// 0, a link is its number (from 1), and the self-pipe is the last token.
const WAKER: Token = Token(usize::MAX);
/// The token of the control listener.
const LISTENER: Token = Token(0);

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

/// The state of a link after a registration call returned `result`, when it was `broken` before (audit A28): a refusal of
/// the poll breaks the link, and a broken link stays broken.
fn broken_after(result: &io::Result<()>, broken: Option<io::ErrorKind>) -> Option<io::ErrorKind> {
    match result {
        Err(error) => Some(error.kind()),
        Ok(()) => broken,
    }
}

/// The accepts that failed with something other than "no client waits" (for example `EMFILE`), and the last error.
#[derive(Debug, Default, PartialEq, Eq)]
struct AcceptFailures {
    count: u64,
    last: Option<String>,
}

impl AcceptFailures {
    /// What an accept returned: the client, or `None` when no client waits or the accept failed. A failure is counted.
    fn take<T>(&mut self, accepted: io::Result<T>) -> Option<T> {
        match accepted {
            Ok(client) => Some(client),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
            Err(error) => {
                self.count += 1;
                self.last = Some(error.to_string());
                None
            }
        }
    }
}

/// The removals of worker endpoints that failed (DESIGN.md "Adoption (P5)" part 1: recorded, never a failed `Remove`).
#[derive(Debug, Default, PartialEq, Eq)]
struct UnlinkFailures {
    count: u64,
    last: Option<String>,
}

impl UnlinkFailures {
    /// What a removal returned. An endpoint that is not there is no failure: the worker removed its own when it ended.
    fn record(&mut self, result: io::Result<()>) {
        match result {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                self.count += 1;
                self.last = Some(error.to_string());
            }
        }
    }
}

/// What the real edges report in the host's diagnostics (LC-10): the failed accepts, the foreign registry files and the
/// failed endpoint removals.
fn edge_diagnostics(
    accepts: &AcceptFailures,
    foreign_registry_files: usize,
    unlinks: &UnlinkFailures,
) -> serde_json::Value {
    serde_json::json!({
        "accept_failures": accepts.count,
        "last_accept_error": accepts.last,
        "foreign_registry_files": foreign_registry_files,
        "endpoint_unlink_failures": unlinks.count,
        "last_endpoint_unlink_error": unlinks.last,
    })
}

/// The directory of the worker endpoints inside the data directory (DESIGN.md "Adoption (P5)" part 1).
fn endpoint_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("w")
}

/// The endpoint of the worker of `instance`: `<data_dir>/w/<InstanceId>`.
pub(crate) fn endpoint_path(data_dir: &Path, instance: &InstanceId) -> PathBuf {
    endpoint_dir(data_dir).join(&instance.0)
}

/// What `lstat` tells about the endpoint directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirFacts {
    /// A directory itself, not a link to one (`lstat`).
    is_dir: bool,
    owner: u32,
    mode: u32,
}

/// AD-6 ("worker endpoints are connectable only by the host's uid"): the endpoint directory is a directory, not a link, owned
/// by `uid`, with no group or other bits. `uid` is the owner of the data directory, which `DataDir::open` proved is this
/// user (the effective uid).
fn endpoint_dir_safe(facts: DirFacts, uid: u32) -> bool {
    facts.is_dir && facts.owner == uid && facts.mode & 0o077 == 0
}

/// The result of the create of the endpoint directory. A directory that is there already is no failure: `open` then checks
/// that it is safe. Any other failure fails the open with its own error.
fn created(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        other => other,
    }
}

/// The id of the next link of this host. Ids count up from 1 and never repeat, so a closed link's id never names a later
/// link (plan 23l).
fn take_link(next: &mut u64) -> LinkId {
    let link = LinkId(*next);
    *next += 1;
    link
}

/// Creates the endpoint directory with mode `0700` when it is missing, and refuses one that is not safe.
fn open_endpoint_dir(data_dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let dir = endpoint_dir(data_dir);
    created(std::fs::DirBuilder::new().mode(0o700).create(&dir))?;
    let owner = std::fs::metadata(data_dir)?.uid();
    let meta = std::fs::symlink_metadata(&dir)?;
    let facts = DirFacts {
        is_dir: meta.file_type().is_dir(),
        owner: meta.uid(),
        mode: meta.mode(),
    };
    if endpoint_dir_safe(facts, owner) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "the worker endpoint directory {} is not a 0700 directory of this user (AD-6)",
                dir.display()
            ),
        ))
    }
}

/// The path of the control socket inside the data directory. A Unix socket path is limited to about 100 bytes, so the name is
/// short.
fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join("c")
}

/// The control socket and the longest worker endpoint of `data_dir` must fit a Unix socket address. `open` checks them
/// before it touches the directory or the host epoch, and a directory whose sockets cannot be bound is a configuration error
/// of `data_dir`.
pub(crate) fn check_socket_path(data_dir: &Path) -> Result<(), CoreError> {
    // The longest worker endpoint has the longest `InstanceId`, `"<u64>-<u64>"` (DESIGN.md "Adoption (P5)" part 1).
    let longest = InstanceId(format!("{}-{}", u64::MAX, u64::MAX));
    for socket in [socket_path(data_dir), endpoint_path(data_dir, &longest)] {
        std::os::unix::net::SocketAddr::from_pathname(&socket).map_err(|error| {
            CoreError::new(
                ErrorCode::InvalidConfig {
                    field: "data_dir".into(),
                },
                format!("the socket {} cannot be bound: {error}", socket.display()),
            )
        })?;
    }
    Ok(())
}

/// One accepted control link and what is registered for it (plan 2.5: read interest follows what the engine can take, write
/// interest follows the outbound buffer).
struct LinkIo {
    stream: UnixStream,
    read: bool,
    write: bool,
    registered: bool,
    /// The poll registration failed: the link can no longer wake the host, so its next read or write fails with this error,
    /// and the driver closes it (audit A28).
    broken: Option<io::ErrorKind>,
}

impl LinkIo {
    /// Makes the registration follow the interest. Returns false when the poll refused it: the link is then broken.
    fn apply(&mut self, registry: &Registry, link: LinkId) -> bool {
        let token = Token(link.0 as usize);
        let result = match registration(interest_of(self.read, self.write), self.registered) {
            Registration::Reregister(i) => registry.reregister(&mut self.stream, token, i),
            Registration::Register(i) => registry.register(&mut self.stream, token, i).map(|()| {
                self.registered = true;
            }),
            Registration::Deregister => registry.deregister(&mut self.stream).map(|()| {
                self.registered = false;
            }),
            Registration::Keep => Ok(()),
        };
        self.record(&result)
    }

    /// Records what a registration call returned. Returns false when the link is broken.
    fn record(&mut self, result: &io::Result<()>) -> bool {
        self.broken = broken_after(result, self.broken);
        self.broken.is_none()
    }

    fn check(&self) -> io::Result<()> {
        match self.broken {
            Some(kind) => Err(io::Error::new(
                kind,
                "the poll registration of the link failed",
            )),
            None => Ok(()),
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
    /// The data directory: the worker endpoints are in it.
    data_dir: PathBuf,
    unlinks: UnlinkFailures,
    streams: BTreeMap<LinkId, LinkIo>,
    next_link: u64,
    accepts: AcceptFailures,
    /// Files of the registry directory that are not rows, at the last read of the rows.
    foreign_registry_files: usize,
    wake: Arc<PollWake>,
    scheduler: Production,
}

impl RealEdges {
    /// Opens the edges of a host over an open data directory. The directory's lock and registry move into the edges.
    pub(crate) fn new(data: DataDir, data_dir: &Path) -> io::Result<(RealEdges, u64)> {
        let (lock, storage, epoch) = data.into_storage();
        open_endpoint_dir(data_dir)?;
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
                data_dir: data_dir.to_path_buf(),
                unlinks: UnlinkFailures::default(),
                streams: BTreeMap::new(),
                next_link: 1,
                accepts: AcceptFailures::default(),
                foreign_registry_files: 0,
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

impl HostEdges for RealEdges {
    fn fill_random(&mut self, buf: &mut [u8]) {
        self.entropy.fill(buf);
    }

    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        self.storage.write_row(key, bytes)
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        self.storage.delete_row(key)
    }

    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        let scan = self.storage.scan()?;
        // A file that Core did not write is not a row: it is counted and left alone (lead ruling on audit A1).
        self.foreign_registry_files = scan.foreign;
        let mut rows = Vec::new();
        for key in scan.keys {
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
            endpoint: endpoint_path(&self.data_dir, &spec.instance),
            startup_ms: WorkerLaunch::millis(spec.startup),
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

    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState {
        botster_core_sys::process::identity_state(identity)
    }

    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        self.children.poll_exit()
    }

    /// DESIGN.md "Adoption (P5)" 3.1: a non-blocking connect to the worker endpoint, registered as an accepted link is. A
    /// connect that fails (no endpoint, no worker listening) is `None`: the adoption is `Lost(WorkerUnreachable)`.
    fn connect_worker(&mut self, instance: &InstanceId) -> Option<LinkId> {
        let stream =
            retry_interrupted(|| UnixStream::connect(endpoint_path(&self.data_dir, instance)))
                .ok()?;
        Some(self.add_stream(stream))
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        self.unlinks.record(std::fs::remove_file(endpoint_path(
            &self.data_dir,
            instance,
        )));
    }

    fn accept_link(&mut self) -> Option<LinkId> {
        let (stream, _) = self
            .accepts
            .take(retry_interrupted(|| self.listener.accept()))?;
        Some(self.add_stream(stream))
    }

    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize> {
        match self.streams.get_mut(&link) {
            Some(io) => {
                io.check()?;
                io.stream.read(buf)
            }
            None => Ok(0),
        }
    }

    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize> {
        match self.streams.get_mut(&link) {
            Some(io) => {
                io.check()?;
                io.stream.write(bytes)
            }
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
            if !io.apply(&self.wake.registry, link) {
                // The link cannot wake the host any more: the host pumps now and finds it broken.
                WakeEdge::signal(&*self.wake);
            }
        }
    }

    fn set_read_interest(&mut self, link: LinkId, on: bool) {
        if let Some(io) = self.streams.get_mut(&link) {
            io.read = on;
            if !io.apply(&self.wake.registry, link) {
                WakeEdge::signal(&*self.wake);
            }
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

    fn diagnostics(&self) -> serde_json::Value {
        edge_diagnostics(&self.accepts, self.foreign_registry_files, &self.unlinks)
    }
}

impl RealEdges {
    /// A link of an accepted or a connected stream, registered for reading. A link that the poll does not take is broken:
    /// its first read fails, and the driver closes it and records why.
    fn add_stream(&mut self, mut stream: UnixStream) -> LinkId {
        let link = take_link(&mut self.next_link);
        let registered =
            self.wake
                .registry
                .register(&mut stream, Token(link.0 as usize), Interest::READABLE);
        self.streams.insert(
            link,
            LinkIo {
                stream,
                read: true,
                write: false,
                registered: registered.is_ok(),
                broken: broken_after(&registered, None),
            },
        );
        link
    }
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

    /// Audit A47: a data directory whose control socket path fits a Unix socket address passes the check, and one whose path
    /// does not is `InvalidConfig{data_dir}`, which `open` reports before it touches the directory.
    #[test]
    fn a_control_socket_path_that_cannot_be_bound_is_a_config_error() {
        assert!(check_socket_path(Path::new("/tmp/d")).is_ok());
        let long = Path::new("/tmp").join("x".repeat(200));
        assert_eq!(
            check_socket_path(&long).unwrap_err().code,
            ErrorCode::InvalidConfig {
                field: "data_dir".into()
            }
        );
    }

    /// Audit A28: a refusal of the poll breaks a link, and a broken link stays broken whatever a later call returns: every
    /// read or write of it then fails with the refusal's error.
    #[test]
    fn a_refused_registration_breaks_the_link_for_good() {
        let refused = || Err(io::Error::from(io::ErrorKind::NotFound));
        assert_eq!(broken_after(&Ok(()), None), None);
        assert_eq!(
            broken_after(&refused(), None),
            Some(io::ErrorKind::NotFound)
        );
        assert_eq!(
            broken_after(&Ok(()), Some(io::ErrorKind::NotFound)),
            Some(io::ErrorKind::NotFound)
        );
        let (stream, _peer) = UnixStream::pair().unwrap();
        let mut io = LinkIo {
            stream,
            read: true,
            write: false,
            registered: false,
            broken: None,
        };
        assert!(io.record(&Ok(())), "a registration that the poll takes");
        assert!(io.check().is_ok());
        assert!(!io.record(&refused()), "refused");
        assert_eq!(io.check().unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(!io.record(&Ok(())), "a broken link stays broken");
        assert_eq!(io.check().unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    /// Audit A28, LC-10: an accept with no client waiting is not a failure; any other failed accept is counted with its error,
    /// and the diagnostics of the edges report the count, the last error and the foreign registry files.
    #[test]
    fn a_failed_accept_is_counted_and_reported() {
        let mut accepts = AcceptFailures::default();
        assert_eq!(accepts.take(Ok(5u8)), Some(5));
        assert_eq!(
            accepts.take::<u8>(Err(io::ErrorKind::WouldBlock.into())),
            None
        );
        assert_eq!(accepts, AcceptFailures::default(), "no client waits");
        assert_eq!(accepts.take::<u8>(Err(io::Error::other("first"))), None);
        assert_eq!(accepts.take::<u8>(Err(io::Error::other("second"))), None);
        assert_eq!(
            accepts,
            AcceptFailures {
                count: 2,
                last: Some("second".into())
            }
        );
        let mut unlinks = UnlinkFailures::default();
        unlinks.record(Ok(()));
        unlinks.record(Err(io::ErrorKind::NotFound.into()));
        assert_eq!(
            unlinks,
            UnlinkFailures::default(),
            "a missing endpoint is no failure"
        );
        unlinks.record(Err(io::Error::other("busy")));
        assert_eq!(
            edge_diagnostics(&accepts, 3, &unlinks),
            serde_json::json!({
                "accept_failures": 2,
                "last_accept_error": "second",
                "foreign_registry_files": 3,
                "endpoint_unlink_failures": 1,
                "last_endpoint_unlink_error": "busy",
            })
        );
    }

    /// Link ids count up from 1, one per link, and never repeat (plan 23l).
    #[test]
    fn link_ids_count_up_and_never_repeat() {
        let mut next = 1;
        let links: Vec<LinkId> = (0..3).map(|_| take_link(&mut next)).collect();
        assert_eq!(links, [LinkId(1), LinkId(2), LinkId(3)]);
        assert_eq!(next, 4);
    }

    /// DESIGN.md part 1: an endpoint directory that is there already is no failure of its create; any other failure is.
    #[test]
    fn only_an_existing_endpoint_directory_is_no_failure_of_the_create() {
        assert!(created(Ok(())).is_ok());
        assert!(created(Err(io::ErrorKind::AlreadyExists.into())).is_ok());
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::NotFound,
            io::ErrorKind::Other,
        ] {
            assert_eq!(created(Err(kind.into())).unwrap_err().kind(), kind);
        }
    }

    /// DESIGN.md "Adoption (P5)" part 1: the endpoint of an instance is `<data_dir>/w/<InstanceId>`.
    #[test]
    fn a_worker_endpoint_is_in_the_endpoint_directory() {
        assert_eq!(
            endpoint_path(Path::new("/d"), &InstanceId("7-1".into())),
            PathBuf::from("/d/w/7-1")
        );
    }

    /// AD-6, DESIGN.md part 1: only a directory itself, of the host's uid, with no group or other bits, is safe.
    #[test]
    fn only_a_private_directory_of_the_user_is_a_safe_endpoint_directory() {
        let safe = DirFacts {
            is_dir: true,
            owner: 501,
            mode: 0o40700,
        };
        assert!(endpoint_dir_safe(safe, 501));
        assert!(!endpoint_dir_safe(safe, 502), "another user's");
        assert!(
            !endpoint_dir_safe(
                DirFacts {
                    is_dir: false,
                    ..safe
                },
                501
            ),
            "a link or a file"
        );
        for bits in [0o040, 0o020, 0o010, 0o004, 0o002, 0o001] {
            assert!(
                !endpoint_dir_safe(
                    DirFacts {
                        mode: 0o40700 | bits,
                        ..safe
                    },
                    501
                ),
                "{bits:o}"
            );
        }
    }

    /// AD-6, DESIGN.md part 1: `open` creates the endpoint directory with mode 0700 and opens an existing private one. It
    /// refuses one that other users can reach and a link, and a failed create fails the open with its own error.
    #[test]
    fn the_endpoint_directory_is_created_private_and_an_unsafe_one_is_refused() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let mode = |path: &Path| {
            std::fs::symlink_metadata(path)
                .unwrap()
                .permissions()
                .mode()
        };
        let data = tempfile::tempdir().unwrap();
        open_endpoint_dir(data.path()).unwrap();
        assert_eq!(mode(&endpoint_dir(data.path())) & 0o777, 0o700);
        open_endpoint_dir(data.path()).expect("an existing private directory is opened");

        std::fs::set_permissions(
            endpoint_dir(data.path()),
            std::fs::Permissions::from_mode(0o750),
        )
        .unwrap();
        assert_eq!(
            open_endpoint_dir(data.path()).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );

        let linked = tempfile::tempdir().unwrap();
        let target = linked.path().join("target");
        std::fs::DirBuilder::new().create(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).unwrap();
        symlink(&target, endpoint_dir(linked.path())).unwrap();
        assert_eq!(
            open_endpoint_dir(linked.path()).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied,
            "a link is not the directory itself"
        );

        // The superuser creates in a read-only directory, so the create cannot fail there.
        if !rustix::process::geteuid().is_root() {
            let closed = tempfile::tempdir().unwrap();
            std::fs::set_permissions(closed.path(), std::fs::Permissions::from_mode(0o500))
                .unwrap();
            let refused = open_endpoint_dir(closed.path()).unwrap_err().kind();
            std::fs::set_permissions(closed.path(), std::fs::Permissions::from_mode(0o700))
                .unwrap();
            assert_eq!(
                refused,
                io::ErrorKind::PermissionDenied,
                "the create's own error"
            );
        }
    }

    /// DESIGN.md part 1: a data directory whose longest worker endpoint cannot be bound is refused, as one whose control
    /// socket cannot be bound is.
    #[test]
    fn a_data_directory_whose_longest_endpoint_cannot_be_bound_is_a_config_error() {
        // The control socket fits; the longest endpoint (41 more bytes) does not.
        let base = "/tmp/".to_string() + &"d".repeat(60);
        assert!(
            std::os::unix::net::SocketAddr::from_pathname(socket_path(Path::new(&base))).is_ok()
        );
        assert_eq!(
            check_socket_path(Path::new(&base)).unwrap_err().code,
            ErrorCode::InvalidConfig {
                field: "data_dir".into()
            }
        );
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

    /// Audit A28: the poll refuses a change of the interest of a link, here because the link's descriptor was taken out of the
    /// poll behind the edges' back, which epoll refuses (kqueue has no such refusal, so the test is Linux only). The edges
    /// wake the host, and every later read or write of the link fails although the client's bytes wait to be read; closing
    /// the link ends the stream for the client.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_link_that_the_poll_refuses_wakes_the_host_and_fails() {
        use std::io::{Read, Write};
        let (mut edges, tmp) = edges();
        let mut client = connect(&tmp);
        let link = accept(&mut edges);
        edges.settle_wake();
        assert_eq!(edges.wake.wait(Duration::ZERO), Wake::TimedOut);
        // A change that the poll takes does not wake the host (the link has nothing to read yet).
        edges.set_read_interest(link, true);
        assert_eq!(edges.wake.wait(Duration::ZERO), Wake::TimedOut);
        client.write_all(b"abc").unwrap();
        edges.settle_wake();
        let io = edges.streams.get_mut(&link).expect("accepted");
        edges.wake.registry.deregister(&mut io.stream).unwrap();
        edges.set_write_interest(link, true);
        assert_eq!(
            edges.wake.wait(Duration::ZERO),
            Wake::Woken,
            "the host pumps and finds the link broken"
        );
        // A broken link wakes the host again at the next change of its read interest.
        edges.wake.drain();
        edges.settle_wake();
        assert_eq!(edges.wake.wait(Duration::ZERO), Wake::TimedOut);
        edges.set_read_interest(link, false);
        assert_eq!(edges.wake.wait(Duration::ZERO), Wake::Woken);
        let mut buf = [0u8; 8];
        let read = edges.link_recv(link, &mut buf).unwrap_err();
        let write = edges.link_send(link, b"x").unwrap_err();
        assert_ne!(read.kind(), io::ErrorKind::WouldBlock, "{read}");
        assert_eq!(read.kind(), write.kind());
        edges.link_close(link);
        // The edges closed the link with the client's bytes unread: the client sees a reset.
        client
            // timer: deadline — a link that the edges do not close fails the test instead of hanging it
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        let mut rest = Vec::new();
        assert_eq!(
            client.read_to_end(&mut rest).unwrap_err().kind(),
            io::ErrorKind::ConnectionReset
        );
        assert!(rest.is_empty(), "the edges sent nothing");
    }

    /// Audit A28, LC-10: the host closes a link whose registration the poll refuses, and its diagnostics say why. The refusal
    /// is made as in `a_link_that_the_poll_refuses_wakes_the_host_and_fails`, after the host accepted the client.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_host_closes_a_link_that_the_poll_refuses_and_records_why() {
        use crate::Core;
        use std::io::Read;
        let pump = |core: &mut Core| {
            let monotonic = crate::real_now();
            core.pump(Now {
                monotonic,
                unix: 1_000_000,
            })
        };
        let tmp = tempfile::tempdir().unwrap();
        let mut core = Core::open(OpenConfig {
            data_dir: tmp.path().join("d"),
            worker_path: Some(PathBuf::from("/bin/true")),
            limits: CoreLimits::default(),
        })
        .unwrap();
        let mut client = connect(&tmp);
        // A connect on a Unix socket is queued at once: this pump accepts it.
        pump(&mut core);
        let edges = core.driver.edges();
        let io = edges.streams.get_mut(&LinkId(1)).expect("accepted");
        edges.wake.registry.deregister(&mut io.stream).unwrap();
        pump(&mut core);
        let closes = &core.diagnostics()["link_closes"];
        assert_eq!(
            closes,
            &serde_json::json!(["link 1: the poll registration of the link failed"]),
        );
        client
            // timer: deadline — a link that the host does not close fails the test instead of hanging it
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        let mut rest = Vec::new();
        client.read_to_end(&mut rest).unwrap();
        assert!(rest.is_empty(), "the client sees the end of the stream");
    }
}
