use super::*;
use botster_core_edges::edges::Wake as WakeEdge;
use botster_core_host::session::unknown_request;
use botster_core_link::frame::encode_frame;
use botster_core_link::proof::{token_proof, TOKEN_LEN};
use std::collections::BTreeSet;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// One scripted answer of the fake's `link_recv`. An empty script answers `WouldBlock`.
#[derive(Debug, Clone)]
enum Read {
    Data(Vec<u8>),
    Eof,
    Fail(io::ErrorKind),
    Interrupted,
}

#[derive(Default)]
struct FakeWake {
    signals: AtomicUsize,
}

impl WakeHandle for FakeWake {
    fn wait(&self, _timeout: Duration) -> Wake {
        Wake::TimedOut
    }

    fn fd(&self) -> RawFd {
        -1
    }
}

impl WakeEdge for FakeWake {
    fn signal(&self) {
        self.signals.fetch_add(1, Ordering::SeqCst);
    }

    fn drain(&self) {}
}

#[derive(Default)]
struct Picks {
    calls: Vec<(ChoicePoint, usize)>,
}

impl Scheduler for Picks {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        self.calls.push((point, candidates));
        candidates - 1
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        self.calls.push((point, max));
        max / 2
    }
}

/// Scripted inner edges. A closed link reads `Ok(0)` and refuses a send with `BrokenPipe`, as `RealEdges` does.
#[derive(Default)]
struct Fake {
    reads: BTreeMap<LinkId, VecDeque<Read>>,
    accepts: VecDeque<LinkId>,
    connects: BTreeMap<InstanceId, LinkId>,
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
    rows: BTreeMap<String, Vec<u8>>,
    fail_writes: bool,
    closed: BTreeSet<LinkId>,
    closes: Vec<LinkId>,
    sent: Vec<(LinkId, Vec<u8>)>,
    /// The most bytes that one `link_send` takes; all of them when `None`.
    send_limit: Option<usize>,
    /// Whether `link_send_descriptor` takes the bytes; it fails when false.
    take_descriptors: bool,
    calls: Vec<String>,
    settles: usize,
    wake: Arc<FakeWake>,
    picks: Picks,
    /// The identity check's answer for each pid; `Absent` for every other pid.
    states: BTreeMap<u32, IdentityState>,
}

impl HostEdges for Fake {
    fn fill_random(&mut self, buf: &mut [u8]) {
        buf.fill(7);
    }

    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        if self.fail_writes {
            return Err(StorageError::Failed { errno: 5 });
        }
        self.rows.insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        self.rows.remove(key);
        Ok(())
    }

    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        Ok(self
            .rows
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    fn spawn_worker(&mut self, _spec: &WorkerSpawn) -> Result<ProcessIdentity, SpawnError> {
        Ok(identity(41))
    }

    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        self.calls
            .push(format!("signal {} {signal:?}", identity.pid));
    }

    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState {
        self.states
            .get(&identity.pid)
            .copied()
            .unwrap_or(IdentityState::Absent)
    }

    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        self.exits.pop_front()
    }

    fn accept_link(&mut self) -> Option<LinkId> {
        self.accepts.pop_front()
    }

    fn connect_worker(&mut self, instance: &InstanceId) -> Option<LinkId> {
        self.connects.get(instance).copied()
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        self.calls.push(format!("remove {}", instance.0));
    }

    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize> {
        if self.closed.contains(&link) {
            return Ok(0);
        }
        match self.reads.get_mut(&link).and_then(VecDeque::pop_front) {
            None => Err(io::ErrorKind::WouldBlock.into()),
            Some(Read::Data(mut bytes)) => {
                let n = buf.len().min(bytes.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                if n < bytes.len() {
                    bytes.drain(..n);
                    self.reads
                        .entry(link)
                        .or_default()
                        .push_front(Read::Data(bytes));
                }
                Ok(n)
            }
            Some(Read::Eof) => Ok(0),
            Some(Read::Fail(kind)) => Err(kind.into()),
            Some(Read::Interrupted) => Err(io::ErrorKind::Interrupted.into()),
        }
    }

    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize> {
        if self.closed.contains(&link) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let n = self
            .send_limit
            .map_or(bytes.len(), |limit| limit.min(bytes.len()));
        self.sent.push((link, bytes[..n].to_vec()));
        Ok(n)
    }

    fn link_close(&mut self, link: LinkId) {
        self.closed.insert(link);
        self.closes.push(link);
    }

    fn set_write_interest(&mut self, link: LinkId, on: bool) {
        self.calls.push(format!("write {} {on}", link.0));
    }

    fn set_read_interest(&mut self, link: LinkId, on: bool) {
        self.calls.push(format!("read {} {on}", link.0));
    }

    fn link_send_descriptor(
        &mut self,
        link: LinkId,
        bytes: &[u8],
        endpoint: StreamEndpoint,
    ) -> Result<usize, (StreamEndpoint, DescriptorSendError)> {
        self.calls.push(format!("descriptor {} {bytes:?}", link.0));
        if self.take_descriptors {
            self.sent.push((link, bytes.to_vec()));
            return Ok(bytes.len());
        }
        Err((endpoint, DescriptorSendError::Failed))
    }

    fn close_route_stream(&mut self, endpoint: StreamEndpoint, bytes: &[u8]) {
        let endpoint = endpoint.downcast::<u8>().ok();
        self.calls
            .push(format!("close stream {endpoint:?} {bytes:?}"));
    }

    fn wake(&self) -> Arc<dyn HostWake> {
        self.wake.clone()
    }

    fn settle_wake(&mut self) {
        self.settles += 1;
    }

    fn scheduler(&mut self) -> &mut dyn Scheduler {
        &mut self.picks
    }

    fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({"fake": true})
    }
}

fn identity(pid: u32) -> ProcessIdentity {
    ProcessIdentity {
        pid,
        start_time: u64::from(pid) * 10,
    }
}

fn exit_status() -> ExitStatus {
    ExitStatus::Code(0)
}

fn hello_of(instance: &str) -> Hello {
    let instance = InstanceId(instance.into());
    Hello {
        protocol: 1,
        proof: token_proof(&[3u8; TOKEN_LEN], &instance, 1),
        instance,
        host_epoch: 1,
    }
}

fn hello_frame(instance: &str) -> Vec<u8> {
    let hello = hello_of(instance);
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    let mut frame = Vec::new();
    encode_frame(FrameType::HELLO, &payload, DEFAULT_MAX_PAYLOAD, &mut frame).unwrap();
    frame
}

fn row_bytes(id: &str, instance: &str) -> Vec<u8> {
    let row = Row {
        version: botster_core_host::session::ROW_VERSION,
        id: SessionId(id.into()),
        instance: InstanceId(instance.into()),
        state: SessionState::Created,
        request: unknown_request(),
        labels: BTreeMap::new(),
        token: None,
        worker: None,
        payload: None,
        worker_protocol: None,
        worker_features: None,
    };
    serde_json::to_vec(&row).unwrap()
}

struct Rig {
    edges: EdgeTap<Fake>,
    tap: Weak<Mutex<Tap<Fake>>>,
    rows: Rows,
}

fn rig(fake: Fake) -> Rig {
    let rows = Rows::default();
    let (edges, tap) = EdgeTap::new(fake, Arc::clone(&rows));
    Rig { edges, tap, rows }
}

impl Rig {
    fn with<T>(&self, f: impl FnOnce(&mut Tap<Fake>) -> T) -> T {
        f(&mut lock(
            &self.tap.upgrade().expect("the driver holds the tap"),
        ))
    }

    fn signals(&self) -> usize {
        self.with(|t| t.inner.wake.signals.load(Ordering::SeqCst))
    }

    fn recv(&mut self, link: LinkId, len: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let n = self.edges.link_recv(link, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }
}

const A: LinkId = LinkId(1);
const B: LinkId = LinkId(2);

fn fake_with_link(reads: Vec<Read>) -> Fake {
    let mut fake = Fake::default();
    fake.accepts.push_back(A);
    fake.reads.insert(A, reads.into());
    fake
}

#[test]
fn every_call_passes_to_the_inner_edges_unchanged() {
    let mut fake = fake_with_link(vec![Read::Data(b"xyz".to_vec())]);
    fake.exits.push_back((identity(5), exit_status()));
    let mut rig = rig(fake);
    let mut random = [0u8; 4];
    rig.edges.fill_random(&mut random);
    assert_eq!(random, [7; 4]);
    assert_eq!(
        rig.edges.spawn_worker(&WorkerSpawn {
            program: "/w".into(),
            instance: InstanceId("1-1".into()),
            token: [0; TOKEN_LEN],
            host_epoch: 1,
            startup: Duration::from_secs(1),
        }),
        Ok(identity(41))
    );
    assert_eq!(rig.edges.identity_state(identity(5)), IdentityState::Absent);
    assert_eq!(
        rig.edges.poll_process_exit(),
        Some((identity(5), exit_status()))
    );
    assert_eq!(rig.edges.poll_process_exit(), None);
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert_eq!(rig.recv(A, 16).unwrap(), b"xyz");
    assert_eq!(
        rig.recv(A, 16).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(rig.edges.link_send(A, b"out").unwrap(), 3);
    rig.edges.settle_wake();
    rig.edges.signal_group(identity(5), GroupSignal::Term);
    rig.edges.remove_endpoint(&InstanceId("1-1".into()));
    rig.edges.set_write_interest(A, true);
    rig.edges.set_read_interest(A, false);
    assert_eq!(rig.edges.diagnostics(), serde_json::json!({"fake": true}));
    // The inner edge's answer comes back unchanged, with the endpoint it gave back.
    let (endpoint, why) = rig
        .edges
        .link_send_descriptor(A, b"rt", StreamEndpoint::new(5u8))
        .unwrap_err();
    assert_eq!(why, DescriptorSendError::Failed);
    assert_eq!(endpoint.downcast::<u8>().ok(), Some(5));
    rig.edges
        .close_route_stream(StreamEndpoint::new(6u8), b"rc");
    rig.edges.wake().signal();
    assert_eq!(rig.edges.scheduler().pick(ChoicePoint::ReadyWork, 3), 2);
    assert_eq!(rig.edges.scheduler().bound(ChoicePoint::ReadyWork, 8), 4);
    rig.edges.link_close(A);
    rig.with(|t| {
        assert_eq!(t.inner.sent, vec![(A, b"out".to_vec())]);
        assert_eq!(t.inner.settles, 1);
        assert_eq!(t.inner.closes, vec![A]);
        assert_eq!(
            t.inner.calls,
            [
                "signal 5 Term",
                "remove 1-1",
                "write 1 true",
                "read 1 false",
                "descriptor 1 [114, 116]",
                "close stream Some(6) [114, 99]"
            ]
        );
        assert_eq!(
            t.inner.picks.calls,
            vec![(ChoicePoint::ReadyWork, 3), (ChoicePoint::ReadyWork, 8)]
        );
    });
    assert_eq!(rig.signals(), 1);
}

#[test]
fn quiet_edges_are_quiet_and_signal_nothing() {
    let mut rig = rig(fake_with_link(vec![]));
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert!(rig.with(Tap::quiet));
    assert_eq!(rig.signals(), 0);
}

#[test]
fn bytes_taken_ahead_are_not_quiet_and_reach_the_driver_in_order() {
    let mut rig = rig(fake_with_link(vec![
        Read::Interrupted,
        Read::Data(b"abc".to_vec()),
        Read::Data(b"de".to_vec()),
    ]));
    assert_eq!(rig.edges.accept_link(), Some(A));
    // An interrupted take is taken again; the take stops at one chunk.
    assert!(!rig.with(Tap::quiet));
    assert_eq!(
        rig.signals(),
        1,
        "the tap wakes the host to read what it holds"
    );
    // While the tap holds bytes, it takes nothing more: the edges stay busy, and what it holds stays bounded.
    assert!(!rig.with(Tap::quiet));
    rig.with(|t| assert_eq!(t.inner.reads[&A].len(), 1));
    assert_eq!(rig.signals(), 2);
    assert_eq!(rig.recv(A, 2).unwrap(), b"ab");
    assert_eq!(rig.recv(A, 16).unwrap(), b"c");
    assert_eq!(rig.recv(A, 16).unwrap(), b"de");
    assert_eq!(
        rig.recv(A, 16).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(rig.with(Tap::quiet));
}

#[test]
fn an_end_taken_ahead_reaches_the_driver_once() {
    let mut rig = rig(fake_with_link(vec![Read::Eof]));
    let mut fake_b = VecDeque::new();
    fake_b.push_back(Read::Fail(io::ErrorKind::ConnectionReset));
    rig.with(|t| {
        t.inner.accepts.push_back(B);
        t.inner.reads.insert(B, fake_b);
    });
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert_eq!(rig.edges.accept_link(), Some(B));
    assert!(!rig.with(Tap::quiet));
    assert_eq!(rig.recv(A, 16).unwrap(), b"", "the peer closed");
    assert_eq!(
        rig.recv(B, 16).unwrap_err().kind(),
        io::ErrorKind::ConnectionReset
    );
    // Each end is handed out once; after it the inner edge answers again.
    assert_eq!(
        rig.recv(A, 16).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        rig.recv(B, 16).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn new_links_and_exits_taken_ahead_are_handed_out_first() {
    let mut fake = Fake::default();
    fake.accepts.extend([A, B]);
    fake.exits.push_back((identity(9), exit_status()));
    fake.exits.push_back((identity(8), exit_status()));
    let mut rig = rig(fake);
    // One take of each inbound edge: one link and one exit.
    assert!(!rig.with(Tap::quiet));
    rig.with(|t| {
        assert_eq!(t.inner.accepts, [B]);
        assert_eq!(t.inner.exits.len(), 1);
    });
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert_eq!(rig.edges.accept_link(), Some(B));
    assert_eq!(rig.edges.accept_link(), None);
    assert_eq!(
        rig.edges.poll_process_exit(),
        Some((identity(9), exit_status()))
    );
    assert!(!rig.with(Tap::quiet), "the second exit is new");
    assert_eq!(
        rig.edges.poll_process_exit(),
        Some((identity(8), exit_status()))
    );
    assert!(rig.with(Tap::quiet));
}

#[test]
fn a_hello_names_the_link_of_an_instance() {
    let mut hello = hello_frame("1-7");
    let rest = hello.split_off(3);
    let mut rig = rig(fake_with_link(vec![Read::Data(hello), Read::Data(rest)]));
    rig.with(|t| {
        t.inner.connects.insert(InstanceId("2-1".into()), B);
    });
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert_eq!(rig.edges.connect_worker(&InstanceId("2-1".into())), Some(B));
    assert_eq!(rig.edges.connect_worker(&InstanceId("9-9".into())), None);
    let instance = InstanceId("1-7".into());
    assert_eq!(rig.with(|t| t.link_of(&instance)), None);
    // The hello arrives in two reads; the tap reads it without changing a byte.
    let first = rig.recv(A, 4096).unwrap();
    assert_eq!(rig.with(|t| t.link_of(&instance)), None);
    let second = rig.recv(A, 4096).unwrap();
    assert_eq!([first, second].concat(), hello_frame("1-7"));
    assert_eq!(rig.with(|t| t.link_of(&instance)), Some(A));
    assert_eq!(rig.with(|t| t.link_of(&InstanceId("2-1".into()))), Some(B));
    rig.edges.link_close(A);
    assert_eq!(rig.with(|t| t.link_of(&instance)), None);
}

#[test]
fn a_first_frame_that_is_not_a_hello_names_nothing() {
    let mut frame = Vec::new();
    encode_frame(
        FrameType::WORKER_MSG,
        b"{}",
        DEFAULT_MAX_PAYLOAD,
        &mut frame,
    )
    .unwrap();
    frame.extend(hello_frame("1-7"));
    let mut rig = rig(fake_with_link(vec![Read::Data(frame)]));
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert!(!rig.with(Tap::quiet));
    assert_eq!(rig.with(|t| t.link_of(&InstanceId("1-7".into()))), None);
}

#[test]
fn a_link_or_an_exit_held_alone_keeps_the_edges_busy() {
    // Only an accepted link is held: the next answer is "not quiet", and the tap takes nothing more.
    let mut fake = Fake::default();
    fake.accepts.push_back(A);
    let mut links = rig(fake);
    assert!(!links.with(Tap::quiet));
    assert!(!links.with(Tap::quiet), "the tap still holds the link");
    assert_eq!(links.edges.accept_link(), Some(A));
    assert!(links.with(Tap::quiet));
    // Only an exit is held.
    let mut fake = Fake::default();
    fake.exits.push_back((identity(4), exit_status()));
    let mut exits = rig(fake);
    assert!(!exits.with(Tap::quiet));
    assert!(!exits.with(Tap::quiet), "the tap still holds the exit");
    assert_eq!(
        exits.edges.poll_process_exit(),
        Some((identity(4), exit_status()))
    );
    assert!(exits.with(Tap::quiet));
}

#[test]
fn a_connected_link_keeps_the_instance_that_core_asked_for() {
    // Core connects to the worker of 2-1 (adoption). A hello that names another instance on that link changes nothing:
    // only an accepted link is named by its hello.
    let mut fake = Fake::default();
    fake.connects.insert(InstanceId("2-1".into()), B);
    fake.reads
        .insert(B, vec![Read::Data(hello_frame("5-5"))].into());
    let mut rig = rig(fake);
    assert_eq!(rig.edges.connect_worker(&InstanceId("2-1".into())), Some(B));
    assert_eq!(rig.recv(B, 4096).unwrap(), hello_frame("5-5"));
    assert_eq!(rig.with(|t| t.link_of(&InstanceId("2-1".into()))), Some(B));
    assert_eq!(rig.with(|t| t.link_of(&InstanceId("5-5".into()))), None);
}

#[test]
fn a_hello_taken_ahead_also_names_the_link() {
    let rig = rig(fake_with_link(vec![Read::Data(hello_frame("3-3"))]));
    assert!(!rig.with(Tap::quiet));
    assert_eq!(rig.with(|t| t.link_of(&InstanceId("3-3".into()))), Some(A));
}

#[test]
fn a_broken_link_reaches_the_inner_closed_state_on_every_later_call() {
    let mut rig = rig(fake_with_link(vec![
        Read::Data(b"held".to_vec()),
        Read::Eof,
    ]));
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert!(!rig.with(Tap::quiet));
    rig.with(|t| t.break_link(A));
    rig.with(|t| assert_eq!(t.inner.closes, vec![A]));
    // What the tap held is dropped: the driver reads the inner edge's closed link, not old bytes or an old end.
    assert_eq!(rig.recv(A, 16).unwrap(), b"");
    assert_eq!(
        rig.edges.link_send(A, b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    // Until the driver closes the link, a take-ahead meets the inner edge's closed link again: that is not quiet.
    assert!(!rig.with(Tap::quiet));
    assert_eq!(rig.recv(A, 16).unwrap(), b"");
    rig.edges.link_close(A);
    rig.with(|t| assert_eq!(t.inner.closes, vec![A, A]));
    assert!(rig.with(Tap::quiet));
}

#[test]
fn rows_that_pass_through_are_decoded_by_session() {
    let mut fake = Fake::default();
    fake.rows
        .insert("session/old".into(), row_bytes("old", "0-1"));
    let mut rig = rig(fake);
    rig.edges
        .write_row("session/s1", &row_bytes("s1", "1-1"))
        .unwrap();
    rig.edges
        .write_row("other/s2", &row_bytes("s2", "1-2"))
        .unwrap();
    rig.edges
        .write_row("session/s3", &row_bytes("another", "1-3"))
        .unwrap();
    rig.edges.write_row("session/s4", b"not json").unwrap();
    rig.with(|t| t.inner.fail_writes = true);
    assert!(rig
        .edges
        .write_row("session/s5", &row_bytes("s5", "1-5"))
        .is_err());
    rig.with(|t| t.inner.fail_writes = false);
    let read = rig.edges.read_rows("session/").unwrap();
    assert_eq!(read.len(), 4);
    rig.edges.delete_row("session/s1").unwrap();
    rig.with(|t| assert!(!t.inner.rows.contains_key("session/s1")));
    let rows = lock(&rig.rows);
    let ids: Vec<_> = rows.keys().map(|id| id.0.as_str()).collect();
    // A row outlives its removal; a foreign key, a row of another id and bytes that Core rejects are not recorded.
    assert_eq!(ids, ["old", "s1"]);
    assert_eq!(
        rows[&SessionId("s1".into())].instance,
        InstanceId("1-1".into())
    );
}

#[test]
fn the_tap_ends_with_the_driver() {
    let rig = rig(Fake::default());
    let Rig { edges, tap, .. } = rig;
    assert!(tap.upgrade().is_some());
    drop(edges);
    assert!(tap.upgrade().is_none());
}

/// `corrupt_registry_row` (Core A10-2): the harness reads the bytes of exactly the session's row from the inner storage edge,
/// and its write reaches the inner storage but not the record of the session's processes.
#[test]
fn the_harness_reads_and_writes_exactly_one_stored_row_past_the_record() {
    let mut fake = Fake::default();
    fake.rows
        .insert("session/s1".into(), row_bytes("s1", "1-1"));
    fake.rows
        .insert("session/s10".into(), row_bytes("s10", "1-2"));
    let rig = rig(fake);
    rig.with(|t| {
        assert_eq!(t.stored_row("session/s1"), Some(row_bytes("s1", "1-1")));
        assert_eq!(t.stored_row("session/s10"), Some(row_bytes("s10", "1-2")));
        assert_eq!(t.stored_row("session/s"), None, "a prefix is no key");
        assert_eq!(t.stored_row("session/s2"), None);
        assert_eq!(t.store_row("session/s1", b"{"), Ok(()));
        assert_eq!(t.inner.rows["session/s1"], b"{");
        assert_eq!(t.stored_row("session/s1"), Some(b"{".to_vec()));
    });
    assert!(
        lock(&rig.rows).is_empty(),
        "the harness's write is no row of Core"
    );
    rig.with(|t| t.inner.fail_writes = true);
    assert_eq!(
        rig.with(|t| t.store_row("session/s1", b"x")),
        Err(StorageError::Failed { errno: 5 })
    );
}

/// `lose_worker` (Core AD-2, AD-6): only a recorded identity whose pid and start time still match gets the `KILL` of its
/// group, followed by a wake; a pid that another process reuses, or an ended process, gets no signal and no wake. The
/// identity check is the inner edge's own.
#[test]
fn only_a_matching_identity_is_killed_and_the_host_is_woken() {
    let mut fake = Fake::default();
    fake.states.insert(41, IdentityState::Matches);
    fake.states.insert(42, IdentityState::Reused);
    let rig = rig(fake);
    rig.with(|t| {
        assert_eq!(t.identity_state(identity(41)), IdentityState::Matches);
        assert_eq!(t.identity_state(identity(42)), IdentityState::Reused);
        assert_eq!(t.identity_state(identity(43)), IdentityState::Absent);
        assert_eq!(t.kill_group(identity(42)), IdentityState::Reused);
        assert_eq!(t.kill_group(identity(43)), IdentityState::Absent);
        assert!(t.inner.calls.is_empty(), "{:?}", t.inner.calls);
    });
    assert_eq!(rig.signals(), 0);
    assert_eq!(
        rig.with(|t| t.kill_group(identity(41))),
        IdentityState::Matches
    );
    rig.with(|t| assert_eq!(t.inner.calls, ["signal 41 Kill"]));
    assert_eq!(rig.signals(), 1);
}

fn worker_frame(msg: &WorkerMsg) -> Vec<u8> {
    let mut payload = Vec::new();
    msg.encode(&mut payload);
    let mut frame = Vec::new();
    encode_frame(
        FrameType::WORKER_MSG,
        &payload,
        DEFAULT_MAX_PAYLOAD,
        &mut frame,
    )
    .unwrap();
    frame
}

fn launched(payload: ProcessIdentity) -> WorkerMsg {
    WorkerMsg::Launched {
        features: BTreeSet::new(),
        terminal: TerminalState {
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            modes: ModeFlags::default(),
            title: None,
            cwd: None,
            last_output_at: None,
            focused: Some(false),
            model_rev: ModelRev(1),
            input_rev: InputRevs {
                client: InputRev(0),
                host: InputRev(0),
            },
        },
        formats: Vec::new(),
        payload: botster_core_link::msg::PayloadId {
            pid: payload.pid,
            start_time: payload.start_time,
        },
    }
}

/// `payload_alive` (Core AD-7, LC-5, EV-5(c)): the host keeps a started payload in memory. The worker's `Launched` report
/// on the link names the payload; before the tap reads it, no payload is named. The tap reads the report across reads,
/// after other reports and with later frames in the same read, without changing a byte; the launch outlives the link.
#[test]
fn a_launched_report_names_the_payload_of_the_link() {
    let exited = worker_frame(&WorkerMsg::Exited {
        code: Some(0),
        signal: None,
    });
    let mut report = worker_frame(&launched(identity(77)));
    let rest = report.split_off(5);
    let later = worker_frame(&launched(identity(78)));
    let reads = vec![
        Read::Data([hello_frame("1-7"), exited.clone()].concat()),
        Read::Data(report.clone()),
        Read::Data([rest.clone(), later.clone()].concat()),
    ];
    let mut rig = rig(fake_with_link(reads));
    let instance = InstanceId("1-7".into());
    assert_eq!(rig.edges.accept_link(), Some(A));
    let first = rig.recv(A, 4096).unwrap();
    let second = rig.recv(A, 4096).unwrap();
    assert_eq!(
        rig.with(|t| t.launch_of(&instance)),
        None,
        "half a report names nothing"
    );
    let third = rig.recv(A, 4096).unwrap();
    assert_eq!(
        [first, second, third].concat(),
        [hello_frame("1-7"), exited, report, rest, later].concat()
    );
    assert_eq!(rig.with(|t| t.launch_of(&instance)), Some(identity(77)));
    assert_eq!(rig.with(|t| t.launch_of(&InstanceId("1-8".into()))), None);
    rig.edges.link_close(A);
    assert_eq!(
        rig.with(|t| t.launch_of(&instance)),
        Some(identity(77)),
        "the launch outlives the link"
    );
}

/// The tap reads only the frames of a worker after its hello: after a frame of another kind, the launch of the spawned
/// worker is not known, and a later `Launched` report names nothing.
#[test]
fn a_report_after_a_frame_of_another_kind_names_nothing() {
    let mut other = Vec::new();
    encode_frame(FrameType::HOST_MSG, b"{}", DEFAULT_MAX_PAYLOAD, &mut other).unwrap();
    let reads = vec![Read::Data(
        [
            hello_frame("1-7"),
            other,
            worker_frame(&launched(identity(77))),
        ]
        .concat(),
    )];
    let mut rig = rig(fake_with_link(reads));
    assert_eq!(rig.edges.accept_link(), Some(A));
    rig.recv(A, 4096).unwrap();
    let instance = InstanceId("1-7".into());
    assert_eq!(rig.with(|t| t.link_of(&instance)), Some(A));
    assert_eq!(
        rig.with(|t| t.launch_of(&instance)),
        None,
        "the launch is not known"
    );
}

/// A first frame that is a hello by kind but not by its bytes names nothing, and the tap reads no later frame: a valid
/// hello after it names no instance.
#[test]
fn a_hello_that_does_not_decode_ends_the_reading_of_the_link() {
    let mut bad = Vec::new();
    encode_frame(
        FrameType::HELLO,
        b"not a hello",
        DEFAULT_MAX_PAYLOAD,
        &mut bad,
    )
    .unwrap();
    let reads = vec![Read::Data([bad, hello_frame("1-7")].concat())];
    let mut rig = rig(fake_with_link(reads));
    assert_eq!(rig.edges.accept_link(), Some(A));
    rig.recv(A, 4096).unwrap();
    assert_eq!(rig.with(|t| t.link_of(&InstanceId("1-7".into()))), None);
}

/// A link that Core connected to (an adoption) is not read: a `Launched` report on it names nothing.
#[test]
fn a_connected_link_is_not_read_for_a_launch() {
    let mut fake = Fake::default();
    fake.connects.insert(InstanceId("2-1".into()), B);
    fake.reads.insert(
        B,
        vec![Read::Data(worker_frame(&launched(identity(77))))].into(),
    );
    let mut rig = rig(fake);
    let instance = InstanceId("2-1".into());
    assert_eq!(rig.edges.connect_worker(&instance), Some(B));
    rig.recv(B, 4096).unwrap();
    assert_eq!(rig.with(|t| t.launch_of(&instance)), None);
}

#[test]
fn every_signal_of_core_passes_to_the_inner_edge_in_order() {
    let mut rig = rig(Fake::default());
    rig.edges.signal_group(identity(5), GroupSignal::Term);
    rig.edges.signal_group(identity(6), GroupSignal::EndPayload);
    rig.edges.signal_group(identity(5), GroupSignal::Kill);
    rig.with(|t| {
        assert_eq!(
            t.inner.calls,
            ["signal 5 Term", "signal 6 EndPayload", "signal 5 Kill"]
        );
    });
}

mod stand_ins {
    use super::*;
    use crate::stand_in::Role;
    use crate::worker::{impostor_answer, ImpostorField, ImpostorPlan};
    use std::io::{Read as _, Write as _};
    use std::os::unix::fs::MetadataExt as _;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    fn sid(name: &str) -> SessionId {
        SessionId(name.into())
    }

    fn plan() -> ImpostorPlan {
        ImpostorPlan {
            session: sid("s1"),
            field: ImpostorField::Instance,
            script: Vec::new(),
            token: [3; TOKEN_LEN],
            worker: identity(41),
        }
    }

    /// A rig whose inner edge connects the instance `i` as the link `A`, with a stand-in of `s1` at a real endpoint path:
    /// the path, the real worker's listener and the rig.
    fn rig_with_stand_in(dir: &Path) -> (PathBuf, UnixListener, Rig) {
        let path = dir.join("w");
        let worker = UnixListener::bind(&path).unwrap();
        let stand_in =
            StandIn::install(sid("s1"), Role::Answer(plan()), &path, &dir.join("aside")).unwrap();
        let mut fake = Fake::default();
        fake.connects.insert(InstanceId("i".into()), A);
        let rig = rig(fake);
        rig.with(|t| t.add_stand_in(stand_in));
        (path, worker, rig)
    }

    fn host_hello() -> Hello {
        hello_of("i")
    }

    /// What the host end can read now. The stand-in wrote its answer within the tap's call, in one write.
    fn answer(host: &mut UnixStream) -> Vec<u8> {
        host.set_nonblocking(true).unwrap();
        let mut got = vec![0u8; 4096];
        let n = host.read(&mut got).unwrap();
        got.truncate(n);
        got
    }

    fn inode(path: &Path) -> u64 {
        std::fs::symlink_metadata(path).unwrap().ino()
    }

    #[test]
    fn a_send_of_core_steps_the_stand_in_and_its_signals_start_at_its_answer() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _worker, mut rig) = rig_with_stand_in(dir.path());
        // The real endpoint waits aside while the tap holds the stand-in.
        let real = inode(&dir.path().join("aside"));
        assert_eq!(
            rig.with(|t| t.signals_since_answer(&sid("s1"))),
            Some(vec![])
        );
        assert_eq!(rig.with(|t| t.signals_since_answer(&sid("s2"))), None);
        rig.edges.signal_group(identity(1), GroupSignal::Kill);
        // Core's connect: the real edge's own connect reaches the stand-in at the path.
        let mut host = UnixStream::connect(&path).unwrap();
        assert_eq!(rig.edges.connect_worker(&InstanceId("i".into())), Some(A));
        host.write_all(&hello_frame("i")).unwrap();
        rig.edges.signal_group(identity(2), GroupSignal::Term);
        assert_eq!(
            rig.with(|t| t.signals_since_answer(&sid("s1"))),
            Some(vec![]),
            "no answer before Core's send"
        );
        assert_eq!(rig.edges.link_send(A, b"hello").unwrap(), 5);
        assert_eq!(answer(&mut host), impostor_answer(&plan(), &host_hello()));
        rig.edges
            .signal_group(identity(41), GroupSignal::EndPayload);
        rig.edges.signal_group(identity(3), GroupSignal::Kill);
        assert_eq!(
            rig.with(|t| t.signals_since_answer(&sid("s1"))),
            Some(vec![
                (identity(41), GroupSignal::EndPayload),
                (identity(3), GroupSignal::Kill)
            ])
        );
        assert_eq!(rig.with(|t| t.signals_since_answer(&sid("s2"))), None);
        drop(rig);
        assert_eq!(inode(&path), real, "the tap's end gives the path back");
    }

    /// The harness's own kill (`lose_worker`) goes to the inner edge and is no signal of Core: the record from the
    /// stand-in's answer on keeps only Core's signals.
    #[test]
    fn the_harnesss_kill_is_no_signal_of_core() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _worker, mut rig) = rig_with_stand_in(dir.path());
        rig.with(|t| t.inner.states.insert(41, IdentityState::Matches));
        let mut host = UnixStream::connect(&path).unwrap();
        assert_eq!(rig.edges.connect_worker(&InstanceId("i".into())), Some(A));
        host.write_all(&hello_frame("i")).unwrap();
        assert_eq!(rig.edges.link_send(A, b"hello").unwrap(), 5);
        assert_eq!(answer(&mut host), impostor_answer(&plan(), &host_hello()));
        assert_eq!(
            rig.with(|t| t.kill_group(identity(41))),
            IdentityState::Matches
        );
        assert_eq!(
            rig.with(|t| t.signals_since_answer(&sid("s1"))),
            Some(vec![]),
            "the harness's kill is no signal of Core"
        );
        rig.edges.signal_group(identity(41), GroupSignal::Kill);
        assert_eq!(
            rig.with(|t| t.signals_since_answer(&sid("s1"))),
            Some(vec![(identity(41), GroupSignal::Kill)]),
            "Core's own signal is"
        );
    }

    #[test]
    fn a_connect_of_core_steps_the_stand_in() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _worker, mut rig) = rig_with_stand_in(dir.path());
        let mut host = UnixStream::connect(&path).unwrap();
        host.write_all(&hello_frame("i")).unwrap();
        assert_eq!(rig.edges.connect_worker(&InstanceId("i".into())), Some(A));
        rig.edges.signal_group(identity(41), GroupSignal::Kill);
        assert_eq!(answer(&mut host), impostor_answer(&plan(), &host_hello()));
        assert_eq!(
            rig.with(|t| t.signals_since_answer(&sid("s1"))),
            Some(vec![(identity(41), GroupSignal::Kill)])
        );
    }
}

/// `hold_start_at` (Core AD-7, R-49 Q2): the tap holds a start at the host's `Launch`. The worker gets every byte before
/// the `Launch` and no byte from it on until the release; Core keeps the rest in its own buffer, unchanged and in order.
mod held_starts {
    use super::*;
    use botster_core_link::msg::LaunchSpec;

    fn host_frame(msg: &HostMsg) -> Vec<u8> {
        let mut payload = Vec::new();
        msg.encode(&mut payload);
        let mut frame = Vec::new();
        encode_frame(
            FrameType::HOST_MSG,
            &payload,
            DEFAULT_MAX_PAYLOAD,
            &mut frame,
        )
        .unwrap();
        frame
    }

    fn launch() -> Vec<u8> {
        host_frame(&HostMsg::Launch(Box::new(LaunchSpec {
            argv: vec!["/bin/sh".into()],
            env: BTreeMap::new(),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            color_profile: None,
            notification_policy: NotificationPolicy::default(),
            size_policy: SizePolicy::default(),
            link_frame_bound: DEFAULT_MAX_PAYLOAD,
            stop_grace_ms: 250,
            limits: CoreLimits::default(),
        })))
    }

    fn instance() -> InstanceId {
        InstanceId("1-7".into())
    }

    /// A rig whose link A has read the hello of the instance "1-7".
    fn linked(fake: Fake) -> Rig {
        let mut fake = fake;
        fake.accepts.push_back(A);
        fake.reads
            .insert(A, vec![Read::Data(hello_frame("1-7"))].into());
        let mut rig = rig(fake);
        assert_eq!(rig.edges.accept_link(), Some(A));
        rig.recv(A, 4096).unwrap();
        rig
    }

    fn sent(rig: &Rig) -> Vec<u8> {
        rig.with(|t| t.inner.sent.iter().flat_map(|(_, b)| b.clone()).collect())
    }

    fn calls(rig: &Rig) -> Vec<String> {
        rig.with(|t| t.inner.calls.clone())
    }

    #[test]
    fn a_held_start_keeps_its_launch_and_every_later_byte_with_core_until_the_release() {
        let mut rig = linked(Fake::default());
        assert!(rig.with(|t| t.hold_launch(instance())));
        assert!(!rig.with(|t| t.hold_launch(instance())), "one hold");
        let before = host_frame(&HostMsg::Stop);
        let bytes = [before.clone(), launch(), host_frame(&HostMsg::Kill)].concat();
        assert_eq!(rig.edges.link_send(A, &bytes).unwrap(), before.len());
        let rest = &bytes[before.len()..];
        let held = rig.edges.link_send(A, rest).unwrap_err();
        assert_eq!(held.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(sent(&rig), before);
        // Core's write interest on the held link does not reach the inner edge; other links are not held.
        rig.edges.set_write_interest(A, true);
        rig.edges.set_write_interest(B, true);
        assert_eq!(calls(&rig), ["write 1 false", "write 2 true"]);
        rig.with(|t| t.inner.take_descriptors = true);
        let blocked = rig
            .edges
            .link_send_descriptor(A, b"rt", StreamEndpoint::new(5u8));
        assert!(matches!(blocked, Err((_, DescriptorSendError::Blocked))));
        assert_eq!(sent(&rig), before);
        assert!(!rig.with(|t| t.release_launch(&InstanceId("2-1".into()))));
        assert!(rig.with(|t| t.release_launch(&instance())));
        assert_eq!(calls(&rig).last().unwrap(), "write 1 true");
        assert!(
            !rig.with(|t| t.release_launch(&instance())),
            "released once"
        );
        assert_eq!(rig.edges.link_send(A, rest).unwrap(), rest.len());
        assert_eq!(sent(&rig), bytes, "no byte reordered, dropped or changed");
    }

    /// Core's last write interest during the hold is the one that the release gives the inner edge.
    #[test]
    fn the_release_gives_the_inner_edge_cores_last_write_interest() {
        let mut rig = linked(Fake::default());
        rig.with(|t| t.hold_launch(instance()));
        rig.edges.link_send(A, &launch()).unwrap_err();
        rig.edges.set_write_interest(A, true);
        rig.edges.set_write_interest(A, false);
        rig.with(|t| t.release_launch(&instance()));
        assert_eq!(
            calls(&rig),
            ["write 1 false", "write 1 false", "write 1 false"]
        );
    }

    /// A hold set before the link exists holds nothing until the `Launch`; the bytes that the inner edge takes in part keep
    /// the tap's reading in step, so the hold begins exactly at the `Launch`.
    #[test]
    fn the_hold_begins_at_the_launch_when_the_inner_edge_takes_bytes_in_part() {
        let mut rig = linked(Fake {
            send_limit: Some(3),
            ..Fake::default()
        });
        rig.with(|t| t.hold_launch(instance()));
        let before = [host_frame(&HostMsg::Stop), host_frame(&HostMsg::Kill)].concat();
        let bytes = [before.clone(), launch()].concat();
        let mut at = 0;
        let held = loop {
            match rig.edges.link_send(A, &bytes[at..]) {
                Ok(n) => {
                    assert!(n > 0 && n <= 3);
                    at += n;
                }
                Err(e) => break e,
            }
        };
        assert_eq!(held.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(at, before.len());
        assert_eq!(sent(&rig), before);
    }

    /// A descriptor send passes through a link that is not held, and the tap's reading of the link keeps step with it.
    #[test]
    fn a_descriptor_send_keeps_the_reading_of_the_link_in_step() {
        let mut rig = linked(Fake {
            take_descriptors: true,
            ..Fake::default()
        });
        rig.with(|t| t.hold_launch(instance()));
        let route = host_frame(&HostMsg::Stop);
        let (first, rest) = route.split_at(4);
        assert_eq!(
            rig.edges
                .link_send_descriptor(A, first, StreamEndpoint::new(5u8))
                .unwrap(),
            first.len()
        );
        let bytes = [rest.to_vec(), launch()].concat();
        assert_eq!(rig.edges.link_send(A, &bytes).unwrap(), rest.len());
        assert_eq!(sent(&rig), route);
    }

    /// Only a held start's `Launch` is held: the `Launch` of another instance, and every byte of a link whose hello the tap
    /// has not read, pass unchanged.
    #[test]
    fn a_launch_that_is_not_held_passes() {
        let mut rig = linked(Fake::default());
        rig.with(|t| t.hold_launch(InstanceId("2-1".into())));
        assert_eq!(rig.edges.link_send(A, &launch()).unwrap(), launch().len());
        assert_eq!(rig.edges.link_send(B, &launch()).unwrap(), launch().len());
        assert_eq!(sent(&rig), [launch(), launch()].concat());
    }

    /// The tap stops reading a link whose start is not held: a hold set later does not hold it. A link's bytes that are not
    /// frames also end the reading, and every later byte passes.
    #[test]
    fn a_link_that_the_tap_stopped_reading_is_never_held() {
        let mut rig = linked(Fake::default());
        rig.edges.link_send(A, &host_frame(&HostMsg::Stop)).unwrap();
        rig.with(|t| t.hold_launch(instance()));
        assert_eq!(rig.edges.link_send(A, &launch()).unwrap(), launch().len());

        let mut rig = linked(Fake::default());
        rig.with(|t| t.hold_launch(instance()));
        let junk = [0xFFu8; 8];
        assert_eq!(rig.edges.link_send(A, &junk).unwrap(), junk.len());
        assert_eq!(rig.edges.link_send(A, &launch()).unwrap(), launch().len());
    }
}
