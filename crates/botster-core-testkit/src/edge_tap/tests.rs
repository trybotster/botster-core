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
    calls: Vec<String>,
    settles: usize,
    wake: Arc<FakeWake>,
    picks: Picks,
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

    fn identity_state(&self, _identity: ProcessIdentity) -> IdentityState {
        IdentityState::Absent
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
        self.sent.push((link, bytes.to_vec()));
        Ok(bytes.len())
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

    fn handoff_route(
        &mut self,
        _link: LinkId,
        route: RouteId,
        _transport: StreamEndpoint,
        options: &AttachOptions,
    ) -> Result<(), HandoffError> {
        self.calls
            .push(format!("handoff {} {}", route.0, options.file_directory));
        Err(HandoffError)
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

fn hello_frame(instance: &str) -> Vec<u8> {
    let instance = InstanceId(instance.into());
    let hello = Hello {
        protocol: 1,
        proof: token_proof(&[3u8; TOKEN_LEN], &instance, 1),
        instance,
        host_epoch: 1,
    };
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
    assert!(rig
        .edges
        .handoff_route(
            A,
            RouteId(1),
            StreamEndpoint::new(5u8),
            &serde_json::from_value(serde_json::json!({"file_directory": "/tmp"})).unwrap()
        )
        .is_err());
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
                "handoff 1 /tmp"
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
        Read::Data(b"abc".to_vec()),
        Read::Interrupted,
        Read::Data(b"de".to_vec()),
    ]));
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert!(!rig.with(Tap::quiet));
    assert_eq!(
        rig.signals(),
        1,
        "the tap wakes the host to read what it holds"
    );
    // A second take-ahead finds nothing new, but the held bytes still keep the edges busy.
    assert!(!rig.with(Tap::quiet));
    assert_eq!(rig.recv(A, 2).unwrap(), b"ab");
    assert_eq!(rig.recv(A, 16).unwrap(), b"cde");
    assert_eq!(
        rig.recv(A, 16).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(rig.with(Tap::quiet));
}

#[test]
fn an_end_taken_ahead_reaches_the_driver_after_the_bytes() {
    let mut rig = rig(fake_with_link(vec![Read::Data(b"z".to_vec()), Read::Eof]));
    let mut fake_b = VecDeque::new();
    fake_b.push_back(Read::Fail(io::ErrorKind::ConnectionReset));
    rig.with(|t| {
        t.inner.accepts.push_back(B);
        t.inner.reads.insert(B, fake_b);
    });
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert_eq!(rig.edges.accept_link(), Some(B));
    assert!(!rig.with(Tap::quiet));
    assert_eq!(rig.recv(A, 16).unwrap(), b"z");
    assert_eq!(rig.recv(A, 16).unwrap(), b"", "the peer closed");
    assert_eq!(
        rig.recv(B, 16).unwrap_err().kind(),
        io::ErrorKind::ConnectionReset
    );
    // Each end is handed out once; after it the inner edge answers again.
    assert_eq!(
        rig.recv(B, 16).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn new_links_and_exits_taken_ahead_are_handed_out_first() {
    let mut fake = Fake::default();
    fake.accepts.push_back(A);
    fake.exits.push_back((identity(9), exit_status()));
    let mut rig = rig(fake);
    assert!(!rig.with(Tap::quiet));
    rig.with(|t| t.inner.accepts.push_back(B));
    assert_eq!(rig.edges.accept_link(), Some(A));
    assert_eq!(rig.edges.accept_link(), Some(B));
    assert_eq!(rig.edges.accept_link(), None);
    assert_eq!(
        rig.edges.poll_process_exit(),
        Some((identity(9), exit_status()))
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
