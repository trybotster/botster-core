//! The driver (plan 2.1, 2.5): framing, the hello, partial writes, the wake flag and the order of a pump, over mock edges.

use super::*;
use crate::driver::{HandoffError, HostDriver, HostEdges, HostWake, WorkerSpawn};
use botster_core_edges::edges::GroupSignal;
use botster_core_edges::scheduler::Production;
use botster_core_edges::{Scheduler, Wake as WakeEdge};
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod api;
mod deadlines;
mod observations;

#[derive(Default)]
struct TestWake {
    flag: AtomicBool,
}

impl WakeHandle for TestWake {
    fn wait(&self, _timeout: Duration) -> Wake {
        if self.flag.load(Ordering::SeqCst) {
            Wake::Woken
        } else {
            Wake::TimedOut
        }
    }

    fn fd(&self) -> std::os::fd::RawFd {
        -1
    }
}

impl WakeEdge for TestWake {
    fn signal(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    fn drain(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct MockLink {
    to_host: Vec<u8>,
    from_host: Vec<u8>,
    closed_by_host: bool,
    /// The most bytes that one `send` takes: a short write (plan 2.5: write interest while bytes wait).
    send_cap: Option<usize>,
    write_interest: bool,
    read_interest: bool,
    peer_closed: bool,
    /// Errors that the next `recv` calls return, last first (a test of the error kinds).
    fail_recv: Vec<io::ErrorKind>,
    /// Errors that the next `send` calls return, last first.
    fail_send: Vec<io::ErrorKind>,
}

struct Mock {
    rows: BTreeMap<String, Vec<u8>>,
    links: BTreeMap<LinkId, MockLink>,
    accept: Vec<LinkId>,
    next_link: u64,
    wake: Arc<TestWake>,
    spawns: Vec<WorkerSpawn>,
    send_cap: Option<usize>,
    /// The exits that the process edge reports, first first.
    exits: Vec<(ProcessIdentity, ExitStatus)>,
    /// Bytes that arrive on a link when the pump settles the wake: the readiness that the poll reports late.
    late: Vec<(LinkId, Vec<u8>)>,
    /// Exits that the process edge reports when the pump settles the wake: a reaper that queued an exit after the pump took
    /// the exits, and whose wake the settle consumed.
    late_exits: Vec<(ProcessIdentity, ExitStatus)>,
    /// The processes whose exit the process edge reported.
    ended: Vec<ProcessIdentity>,
    /// The scheduler's choices in the current pump: a pump that never ends fails the test at once (as `World::pump`).
    choices: u32,
}

type Shared = Arc<Mutex<Mock>>;

struct Edges(Shared, Box<dyn Scheduler + Send>);

fn frame(kind: FrameType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_frame(kind, payload, 1 << 22, &mut out).unwrap();
    out
}

impl HostEdges for Edges {
    fn fill_random(&mut self, buf: &mut [u8]) {
        buf.fill(9);
    }

    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        self.0
            .lock()
            .unwrap()
            .rows
            .insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        self.0.lock().unwrap().rows.remove(key);
        Ok(())
    }

    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .rows
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    fn spawn_worker(
        &mut self,
        spec: &WorkerSpawn,
    ) -> Result<ProcessIdentity, botster_core_edges::edges::SpawnError> {
        let mut mock = self.0.lock().unwrap();
        mock.spawns.push(spec.clone());
        let link = LinkId(mock.next_link);
        mock.next_link += 1;
        let hello = Hello {
            protocol: 1,
            instance: spec.instance.clone(),
            proof: token_proof(&spec.token, &spec.instance, spec.host_epoch),
            host_epoch: spec.host_epoch,
        };
        let mut payload = Vec::new();
        hello.encode(&mut payload).unwrap();
        let cap = mock.send_cap;
        mock.links.insert(
            link,
            MockLink {
                to_host: frame(FrameType::HELLO, &payload),
                send_cap: cap,
                ..MockLink::default()
            },
        );
        mock.accept.push(link);
        Ok(ProcessIdentity {
            pid: 500,
            start_time: 1,
        })
    }

    fn signal_group(&mut self, _identity: ProcessIdentity, _signal: GroupSignal) {}

    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        let mut mock = self.0.lock().unwrap();
        if mock.exits.is_empty() {
            None
        } else {
            let exit = mock.exits.remove(0);
            mock.ended.push(exit.0);
            Some(exit)
        }
    }

    /// A worker of the mock runs until the process edge reported its exit.
    fn identity_state(
        &self,
        identity: ProcessIdentity,
    ) -> botster_core_edges::edges::IdentityState {
        if self.0.lock().unwrap().ended.contains(&identity) {
            botster_core_edges::edges::IdentityState::Absent
        } else {
            botster_core_edges::edges::IdentityState::Matches
        }
    }

    fn accept_link(&mut self) -> Option<LinkId> {
        let mut mock = self.0.lock().unwrap();
        if mock.accept.is_empty() {
            None
        } else {
            Some(mock.accept.remove(0))
        }
    }

    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize> {
        let mut mock = self.0.lock().unwrap();
        let Some(l) = mock.links.get_mut(&link) else {
            return Ok(0);
        };
        if let Some(kind) = l.fail_recv.pop() {
            return Err(kind.into());
        }
        if l.to_host.is_empty() {
            return if l.peer_closed {
                Ok(0)
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            };
        }
        let n = l.to_host.len().min(buf.len());
        buf[..n].copy_from_slice(&l.to_host[..n]);
        l.to_host.drain(..n);
        Ok(n)
    }

    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize> {
        let mut mock = self.0.lock().unwrap();
        let Some(l) = mock.links.get_mut(&link) else {
            return Err(io::ErrorKind::BrokenPipe.into());
        };
        if let Some(kind) = l.fail_send.pop() {
            return Err(kind.into());
        }
        let n = l.send_cap.map_or(bytes.len(), |cap| cap.min(bytes.len()));
        if n == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        l.from_host.extend_from_slice(&bytes[..n]);
        Ok(n)
    }

    fn link_close(&mut self, link: LinkId) {
        if let Some(l) = self.0.lock().unwrap().links.get_mut(&link) {
            l.closed_by_host = true;
        }
    }

    fn set_write_interest(&mut self, link: LinkId, on: bool) {
        if let Some(l) = self.0.lock().unwrap().links.get_mut(&link) {
            l.write_interest = on;
        }
    }

    fn set_read_interest(&mut self, link: LinkId, on: bool) {
        if let Some(l) = self.0.lock().unwrap().links.get_mut(&link) {
            l.read_interest = on;
        }
    }

    fn handoff_route(
        &mut self,
        _l: LinkId,
        _r: RouteId,
        _t: StreamEndpoint,
        _o: &AttachOptions,
    ) -> Result<(), HandoffError> {
        Err(HandoffError)
    }

    fn wake(&self) -> Arc<dyn HostWake> {
        Arc::clone(&self.0.lock().unwrap().wake) as Arc<dyn HostWake>
    }

    fn settle_wake(&mut self) {
        let mut mock = self.0.lock().unwrap();
        let late = std::mem::take(&mut mock.late);
        for (link, bytes) in late {
            if let Some(l) = mock.links.get_mut(&link) {
                l.to_host.extend_from_slice(&bytes);
            }
        }
        let late_exits = std::mem::take(&mut mock.late_exits);
        mock.exits.extend(late_exits);
    }

    fn scheduler(&mut self) -> &mut dyn Scheduler {
        let mut mock = self.0.lock().unwrap();
        mock.choices += 1;
        assert!(mock.choices < 10_000, "the pump does not settle");
        drop(mock);
        &mut *self.1
    }
}

struct Rig {
    driver: HostDriver<Edges>,
    mock: Shared,
    now: Instant,
    unix: u64,
}

impl Rig {
    fn new(limits: CoreLimits) -> Rig {
        Rig::with_scheduler(limits, Box::new(Production::new()))
    }

    fn with_scheduler(limits: CoreLimits, scheduler: Box<dyn Scheduler + Send>) -> Rig {
        Rig::with_config(config(limits), scheduler)
    }

    fn with_config(cfg: crate::EngineConfig, scheduler: Box<dyn Scheduler + Send>) -> Rig {
        let mock = Arc::new(Mutex::new(Mock {
            rows: BTreeMap::new(),
            links: BTreeMap::new(),
            accept: Vec::new(),
            next_link: 1,
            wake: Arc::new(TestWake::default()),
            spawns: Vec::new(),
            send_cap: None,
            exits: Vec::new(),
            late: Vec::new(),
            late_exits: Vec::new(),
            ended: Vec::new(),
            choices: 0,
        }));
        #[allow(clippy::disallowed_methods)] // a test starts the injected clock at a real instant
        let now = Instant::now();
        Rig {
            driver: HostDriver::open(cfg, Edges(Arc::clone(&mock), scheduler))
                .expect("the registry reads"),
            mock,
            now,
            unix: 10,
        }
    }

    fn pump(&mut self) -> PumpReport {
        self.mock.lock().unwrap().choices = 0;
        self.driver.pump(Now {
            monotonic: self.now,
            unix: self.unix,
        })
    }

    /// Decodes what the host wrote on `link`: the frames, in order.
    fn host_frames(&self, link: LinkId) -> Vec<(FrameType, Vec<u8>)> {
        let mock = self.mock.lock().unwrap();
        let mut decoder = FrameDecoder::new(1 << 22);
        let mut rest = mock.links[&link].from_host.as_slice();
        let mut out = Vec::new();
        while !rest.is_empty() {
            let took = decoder.push(rest);
            rest = &rest[took..];
            if let Ok(Some(f)) = decoder.next_frame() {
                out.push((f.kind, f.payload));
            }
        }
        out
    }

    fn worker_says(&self, link: LinkId, msg: WorkerMsg) {
        let mut payload = Vec::new();
        msg.encode(&mut payload);
        self.mock
            .lock()
            .unwrap()
            .links
            .get_mut(&link)
            .unwrap()
            .to_host
            .extend(frame(FrameType::WORKER_MSG, &payload));
    }

    fn wake_set(&self) -> bool {
        self.mock.lock().unwrap().wake.flag.load(Ordering::SeqCst)
    }

    fn drain_events(&mut self) -> Vec<Event> {
        self.driver.poll_events(256)
    }
}

fn launched() -> WorkerMsg {
    WorkerMsg::Launched {
        features: BTreeSet::new(),
        terminal: terminal_state(),
        formats: vec![],
        payload: botster_core_link::msg::PayloadId {
            pid: 900,
            start_time: 3,
        },
    }
}

/// Core 9B, LC-1: `check_open` refuses a zero limit before anything else, and needs a worker path.
#[test]
fn check_open_refuses_a_bad_config_and_a_missing_worker_path() {
    let config = |worker: Option<&str>, limits: CoreLimits| OpenConfig {
        data_dir: "/d".into(),
        worker_path: worker.map(Into::into),
        limits,
    };
    assert_eq!(
        crate::driver::check_open(&config(None, CoreLimits::default()))
            .unwrap_err()
            .code,
        ErrorCode::MissingWorkerPath
    );
    assert!(matches!(
        crate::driver::check_open(&config(Some("/w"), limits(|l| l.max_sessions = 0)))
            .unwrap_err()
            .code,
        ErrorCode::InvalidConfig { .. }
    ));
    assert_eq!(
        crate::driver::check_open(&config(Some("/w"), CoreLimits::default())).unwrap(),
        std::path::PathBuf::from("/w")
    );
}

/// Plan 2.1: the driver runs a start through real frames: the hello, the launch, the report, and `Running`.
#[test]
fn a_start_runs_through_the_frames_of_the_link() {
    let mut rig = Rig::new(CoreLimits::default());
    rig.driver.begin(create("s1")).unwrap();
    let start = {
        rig.pump();
        rig.driver.begin(Op::Start { id: sid("s1") }).unwrap()
    };
    rig.pump();
    let link = LinkId(1);
    // The host answered the hello and sent the launch, in this order.
    let frames = rig.host_frames(link);
    assert_eq!(frames[0].0, FrameType::HELLO);
    assert_eq!(frames[1].0, FrameType::HOST_MSG);
    assert!(matches!(
        HostMsg::decode(&frames[1].1),
        Ok(HostMsg::Launch(_))
    ));
    rig.worker_says(link, launched());
    rig.pump();
    let events = rig.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == start)));
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
    let spawn = rig.mock.lock().unwrap().spawns[0].clone();
    assert_eq!(spawn.host_epoch, 7);
    assert_eq!(
        spawn.token, [9u8; TOKEN_LEN],
        "the token is the Entropy edge's draw (plan 2.3a)"
    );
}

/// Plan 2.5, section 3: a link that takes a few bytes per call gets every frame whole and in order, and has no write interest
/// once nothing waits. (Write interest while bytes wait: `api::io_errors_are_told_apart_by_their_kind`.)
#[test]
fn a_short_write_delivers_whole_frames() {
    let mut rig = Rig::new(CoreLimits::default());
    rig.mock.lock().unwrap().send_cap = Some(3);
    rig.driver.begin(create("s1")).unwrap();
    rig.pump();
    rig.driver.begin(Op::Start { id: sid("s1") }).unwrap();
    rig.pump();
    let link = LinkId(1);
    for _ in 0..200 {
        rig.pump();
    }
    let frames = rig.host_frames(link);
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert!(
        !rig.mock.lock().unwrap().links[&link].write_interest,
        "write interest is off when nothing waits"
    );
}

/// Plan 2.5 rule 1, TM-6: a call that leaves work signals the wake before it returns; a pump that leaves none clears it.
#[test]
fn the_wake_follows_runnable_work_and_the_pump_settles_before_it_clears() {
    let mut rig = Rig::new(CoreLimits::default());
    assert!(!rig.wake_set());
    rig.driver.begin(create("s1")).unwrap();
    assert!(rig.wake_set(), "TM-6: begin signals");
    let report = rig.pump();
    assert!(!report.more);
    assert!(!rig.wake_set(), "the pump cleared it");
    // A poll that frees room for parked work signals again (EV-5d); here nothing is parked, so it stays clear.
    rig.driver.poll_events(8);
    assert!(!rig.wake_set());
}

/// Plan section 3, LC-10: a frame over the bound ends the link, and a first frame that is not a hello does too; the engine
/// learns of both as a closed link, and `diagnostics()` keeps why.
#[test]
fn a_bad_first_frame_or_an_oversize_frame_closes_the_link() {
    let mut rig = Rig::new(CoreLimits::default());
    rig.driver.begin(create("s1")).unwrap();
    rig.pump();
    rig.driver.begin(Op::Start { id: sid("s1") }).unwrap();
    rig.mock.lock().unwrap().links.clear();
    // A worker whose first frame is a report, not a hello.
    {
        let mut mock = rig.mock.lock().unwrap();
        let mut payload = Vec::new();
        WorkerMsg::Observed {
            observation: botster_core_link::msg::Observation::Bell,
        }
        .encode(&mut payload);
        let link = LinkId(40);
        mock.links.insert(
            link,
            MockLink {
                to_host: frame(FrameType::WORKER_MSG, &payload),
                ..MockLink::default()
            },
        );
        mock.accept.push(link);
    }
    rig.pump();
    assert!(rig.mock.lock().unwrap().links[&LinkId(40)].closed_by_host);
    // An oversize frame: the header names a length above the bound.
    {
        let mut mock = rig.mock.lock().unwrap();
        let link = LinkId(41);
        let mut bytes = u32::MAX.to_le_bytes().to_vec();
        bytes.push(0x01);
        mock.links.insert(
            link,
            MockLink {
                to_host: bytes,
                ..MockLink::default()
            },
        );
        mock.accept.push(link);
    }
    rig.pump();
    assert!(rig.mock.lock().unwrap().links[&LinkId(41)].closed_by_host);
    // LC-10, audit A28: each close is recorded with its reason, so an interoperability failure is visible where its cause
    // was known.
    let diagnostics = rig.driver.diagnostics();
    let closes: Vec<&str> = diagnostics["link_closes"]
        .as_array()
        .expect("a list of link closes")
        .iter()
        .filter_map(|c| c.as_str())
        .collect();
    assert!(
        closes.iter().any(|c| c.starts_with("link 40: ")),
        "{closes:?}"
    );
    // The reason is the decoder's own refusal of the same header, at the driver's bound.
    let mut decoder = FrameDecoder::new(rig.driver.engine().link_frame_bound());
    let mut header = u32::MAX.to_le_bytes().to_vec();
    header.push(0x01);
    decoder.push(&header);
    let oversize = decoder.next_frame().unwrap_err().to_string();
    assert!(
        closes.iter().any(|c| *c == format!("link 41: {oversize}")),
        "{closes:?}"
    );
}

/// Plan 2.5: a peer that closes its end (`Ok(0)`) is a closed link, and the pending read of its session fails.
#[test]
fn a_peer_close_fails_the_pending_op() {
    let mut rig = Rig::new(CoreLimits::default());
    rig.driver.begin(create("s1")).unwrap();
    rig.pump();
    rig.driver.begin(Op::Start { id: sid("s1") }).unwrap();
    rig.pump();
    let link = LinkId(1);
    rig.worker_says(link, launched());
    rig.pump();
    rig.drain_events();
    let read = rig
        .driver
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    rig.pump();
    rig.mock
        .lock()
        .unwrap()
        .links
        .get_mut(&link)
        .unwrap()
        .peer_closed = true;
    rig.pump();
    let events = rig.drain_events();
    assert!(
        events.iter().any(
            |e| matches!(e, Event::Completed { op, result: OpResult::Err(err) }
        if *op == read && err.code == ErrorCode::WorkerLinkFailed)
        ),
        "{events:?}"
    );
}

/// Core A5-2: the driver asks the scheduler for the bound of a `pump` and the batch of a poll; the production policy uses
/// both in full, and `pump_events` is the bound.
#[test]
fn a_pump_posts_at_most_pump_events() {
    let mut rig = Rig::new(limits(|l| l.pump_events = 1));
    rig.driver.begin(create("a")).unwrap();
    rig.driver.begin(create("b")).unwrap();
    let report = rig.pump();
    assert_eq!(report.events_posted, 1);
    assert!(report.more, "work remains (TM-6): the host pumps again");
    let mut guard = 0;
    while rig.pump().more {
        guard += 1;
        assert!(guard < 20);
    }
    assert_eq!(rig.drain_events().len(), 4);
}

/// Core TH-1: the driver, and so `Core`, is `Send`.
#[test]
fn the_driver_is_send() {
    fn is_send<T: Send>() {}
    is_send::<HostDriver<Edges>>();
}

fn start_session(rig: &mut Rig, name: &str, link: LinkId) {
    rig.driver.begin(create(name)).unwrap();
    rig.pump();
    rig.driver.begin(Op::Start { id: sid(name) }).unwrap();
    rig.pump();
    rig.worker_says(link, launched());
    rig.pump();
}

/// Core EV-5b, plan 2.5 rule 7 (F2): a route event that needs mandatory room stays unread on its link while the queue is full,
/// the read interest goes off, a poll that frees room restores it and signals the wake, and the next pump delivers it.
#[test]
fn a_blocked_frame_stays_unread_and_the_poll_restores_the_link() {
    let mut rig = Rig::new(limits(|l| {
        l.mandatory_events = 3;
        l.max_sessions = 4;
    }));
    start_session(&mut rig, "s1", LinkId(1));
    // `Created`, `Starting`, `Running` fill the queue of three.
    for _ in 0..5 {
        rig.worker_says(LinkId(1), WorkerMsg::RouteStalled { route: RouteId(1) });
    }
    rig.pump();
    {
        let mock = rig.mock.lock().unwrap();
        let link = &mock.links[&LinkId(1)];
        assert!(
            !link.read_interest,
            "read interest is off while a frame is held"
        );
    }
    rig.drain_events();
    assert!(
        rig.wake_set(),
        "the poll that freed room signalled the wake (EV-5d, TM-6)"
    );
    assert!(rig.mock.lock().unwrap().links[&LinkId(1)].read_interest);
    rig.pump();
    assert!(
        rig.mock.lock().unwrap().links[&LinkId(1)]
            .to_host
            .is_empty(),
        "the frames were consumed"
    );
}

/// 9B (F3): link input counts against `pump_events`: a pump posts at most that many events, reports them, and leaves the rest
/// unread with `more` set.
#[test]
fn link_input_counts_against_the_pump_bound() {
    let mut rig = Rig::new(limits(|l| l.pump_events = 2));
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    for _ in 0..6 {
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Observed {
                observation: botster_core_link::msg::Observation::Bell,
            },
        );
    }
    let report = rig.pump();
    assert_eq!(report.events_posted, 2);
    assert!(report.more, "input remains unread");
    let mut total = 2;
    for _ in 0..10 {
        let r = rig.pump();
        assert!(r.events_posted <= 2);
        total += r.events_posted;
        if !r.more {
            break;
        }
    }
    assert_eq!(total, 6);
}

/// Plan 2.4 (F15): sessions are visited round-robin through the driver: with a small budget the first events are one state
/// per session, not all the work of the first session.
#[test]
fn sessions_are_visited_round_robin() {
    let mut rig = Rig::new(limits(|l| l.pump_events = 1));
    for name in ["a", "b", "c"] {
        rig.driver.begin(create(name)).unwrap();
    }
    let mut events = Vec::new();
    for _ in 0..30 {
        let r = rig.pump();
        events.extend(rig.drain_events());
        if !r.more {
            break;
        }
    }
    let first: Vec<&str> = events
        .iter()
        .take(3)
        .filter_map(|e| match e {
            Event::SessionState { id, .. } => Some(id.0.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(first, ["a", "b", "c"], "{events:?}");
}

/// 9B (F19): a link that holds many small frames is read in a loop, not by recursion: one pump takes them all, within one buffer.
#[test]
fn many_small_frames_are_read_without_recursion() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 100_000;
        l.pump_bytes = 8 << 20;
        l.mandatory_events = 1_000;
    }));
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    for _ in 0..50_000 {
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Observed {
                observation: botster_core_link::msg::Observation::Bell,
            },
        );
    }
    let _ = rig.pump();
    assert!(
        rig.mock.lock().unwrap().links[&LinkId(1)]
            .to_host
            .is_empty(),
        "every frame was read"
    );
}

/// 9B `pump_bytes` (F3): one pump takes at most `pump_bytes` from a link, even when that is below one read chunk.
#[test]
fn a_pump_reads_no_more_than_pump_bytes_from_a_link() {
    let mut rig = Rig::new(limits(|l| l.pump_bytes = 64));
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    for _ in 0..40 {
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Observed {
                observation: botster_core_link::msg::Observation::Bell,
            },
        );
    }
    let before = rig.mock.lock().unwrap().links[&LinkId(1)].to_host.len();
    let report = rig.pump();
    let after = rig.mock.lock().unwrap().links[&LinkId(1)].to_host.len();
    assert!(before - after <= 64, "took {} bytes", before - after);
    assert!(report.more, "input remains");
}

/// Core EV-5b, LC-4 (F2): the `Exited` frame of a worker waits on its link while the queue is full, and it ends the session
/// after a poll frees room.
#[test]
fn an_exited_frame_behind_a_full_queue_is_held_then_delivered() {
    let mut rig = Rig::new(limits(|l| {
        l.mandatory_events = 3;
        l.max_sessions = 4;
    }));
    start_session(&mut rig, "s1", LinkId(1));
    rig.worker_says(
        LinkId(1),
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    rig.pump();
    assert!(
        !matches!(
            rig.driver.get(&sid("s1")).unwrap().state,
            SessionState::Exited(_)
        ),
        "held while the queue is full"
    );
    for _ in 0..6 {
        rig.drain_events();
        rig.pump();
    }
    assert!(matches!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
}
