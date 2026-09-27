//! Start rollback: a start that fails before the exit watch owns the child
//! kills the whole group, descendants included, and reaps the leader.
//!
//! Requires `script/prebuild-worker` for the scripted test worker.

use std::ffi::{CString, OsString};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::{FaultPoint, Order, OrderSeam, PluginProcess, StartFault, ORDER_SEAM, START_FAULT};
use crate::runtime::plugin_process::{
    LoadFrame, PluginLogCredits, PluginProcessConfig, PluginProcessError, PluginProcessRlimits,
    PluginReplyCredits,
};

/// Bound for the descendant's death; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(30);

fn worker() -> PathBuf {
    botster_core_test_support::real_worker::WorkerBinary::plugin_test_worker_from_env()
        .unwrap_or_else(|failure| panic!("{failure}"))
        .path
}

fn mkfifo(path: &Path) {
    let path = CString::new(path.as_os_str().as_bytes()).expect("fifo path");
    // SAFETY: mkfifo with a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0, "mkfifo");
}

/// A scratch directory with the three FIFOs of the descendant protocol.
struct Fifos {
    dir: PathBuf,
    descendant: PathBuf,
    alive: PathBuf,
    report: PathBuf,
}

impl Fifos {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "botster-plugin-rollback-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let fifos = Self {
            descendant: dir.join("descendant"),
            alive: dir.join("alive"),
            report: dir.join("report"),
            dir,
        };
        mkfifo(&fifos.descendant);
        mkfifo(&fifos.alive);
        mkfifo(&fifos.report);
        fifos
    }
}

impl Drop for Fifos {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn config(fifos: &Fifos) -> PluginProcessConfig {
    let env = |key: &str, path: &Path| (OsString::from(key), path.as_os_str().to_owned());
    PluginProcessConfig {
        worker_path: worker(),
        cwd: std::env::temp_dir(),
        env: vec![
            env("PLUGIN_TEST_DESCENDANT_FIFO", &fifos.descendant),
            env("PLUGIN_TEST_ALIVE_FIFO", &fifos.alive),
            env("PLUGIN_TEST_REPORT_FIFO", &fifos.report),
        ],
        rlimits: PluginProcessRlimits::default(),
        sandbox: opaque(json!({})),
        memory_cap_bytes: None,
        max_frame_bytes: 1024 * 1024,
        startup_deadline: Duration::from_secs(30),
        shutdown_deadline: Duration::from_secs(30),
        cancel_grace: Duration::from_secs(30),
        max_in_flight_invokes: 2,
        stderr_tail_bytes: 4096,
        ingress_bytes: 64 * 1024,
        reply_credits: PluginReplyCredits {
            count: 2,
            bytes: 64 * 1024,
        },
        log_credits: PluginLogCredits {
            count: 8,
            bytes: 16 * 1024,
        },
    }
}

/// Build an opaque payload newtype through its transparent serde form.
fn opaque<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("opaque payload")
}

fn load() -> LoadFrame {
    LoadFrame {
        sources: opaque(json!({})),
        config: opaque(json!({ "mode": "report" })),
    }
}

/// Fail the start at `point` once the child has started its descendant, and
/// return the leader's pid.
fn start_failing_at(point: FaultPoint, fifos: &Fifos) -> u32 {
    let leader: Arc<Mutex<Option<u32>>> = Arc::default();
    let hook_leader = leader.clone();
    let report = fifos.report.clone();
    START_FAULT.with(|slot| {
        *slot.borrow_mut() = Some(StartFault {
            point,
            before: Box::new(move |pid| {
                // The child reports after its descendant runs; this read is
                // the event that orders the fault after the descendant.
                let mut reported = String::new();
                File::open(&report)
                    .and_then(|mut report| report.read_to_string(&mut reported))
                    .expect("read the descendant report");
                assert!(!reported.trim().is_empty(), "the descendant started");
                *hook_leader.lock().expect("leader") = Some(pid);
            }),
        });
    });

    let result = PluginProcess::spawn(&config(fifos), &load());
    assert!(
        matches!(&result, Err(PluginProcessError::Launch(error)) if error.to_string().contains("injected")),
        "the injected fault must fail the start"
    );
    let leader = leader.lock().expect("leader").take();
    leader.expect("the fault hook ran")
}

fn assert_rolled_back(point: FaultPoint, name: &str) {
    let fifos = Fifos::new(name);
    // Holding the input FIFO open keeps the descendant alive until killed.
    let _keep_descendant_alive = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fifos.descendant)
        .expect("hold the descendant FIFO");
    // Open the alive FIFO's read end first, without blocking, so the child's
    // write open succeeds; then read it blocking until EOF.
    let alive = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&fifos.alive)
        .expect("open the alive FIFO");

    let leader = start_failing_at(point, &fifos);

    // SAFETY: signal 0 only checks for existence.
    let leader_gone = unsafe { libc::kill(leader as libc::pid_t, 0) } != 0;
    assert!(leader_gone, "the rollback reaped the leader");

    // SAFETY: fcntl get/set on a descriptor this test owns.
    unsafe {
        let flags = libc::fcntl(alive.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(alive.as_raw_fd(), libc::F_SETFL, flags & !libc::O_NONBLOCK);
    }
    let (died, descendant_died) = mpsc::channel();
    std::thread::spawn(move || {
        let mut alive = alive;
        let mut sink = Vec::new();
        let _ = died.send(alive.read_to_end(&mut sink).is_ok());
    });
    // timer: deadline — EOF on the alive FIFO means the descendant died; expiry means only the leader was killed
    let died = descendant_died
        .recv_timeout(EVENT_DEADLINE)
        .expect("the rollback killed the whole group");
    assert!(died);
}

#[test]
fn a_failed_exit_watch_registration_kills_the_group_and_reaps() {
    assert_rolled_back(FaultPoint::RegisterWatch, "register");
}

#[test]
fn a_failed_exit_watch_thread_start_kills_the_group_and_reaps() {
    assert_rolled_back(FaultPoint::SpawnExitWatch, "spawn");
}

fn plain_config() -> PluginProcessConfig {
    PluginProcessConfig {
        worker_path: worker(),
        cwd: std::env::temp_dir(),
        env: Vec::new(),
        rlimits: PluginProcessRlimits::default(),
        sandbox: opaque(json!({})),
        memory_cap_bytes: None,
        max_frame_bytes: 1024 * 1024,
        startup_deadline: Duration::from_secs(30),
        shutdown_deadline: Duration::from_secs(30),
        cancel_grace: Duration::from_secs(30),
        max_in_flight_invokes: 2,
        stderr_tail_bytes: 4096,
        ingress_bytes: 64 * 1024,
        reply_credits: PluginReplyCredits {
            count: 2,
            bytes: 64 * 1024,
        },
        log_credits: PluginLogCredits {
            count: 8,
            bytes: 16 * 1024,
        },
    }
}

/// A handler abort closes the channel and exits by SIGABRT. Whichever of
/// the reader's EOF kill and the exit watch's reap comes first, the cause is
/// the crash, never the incidental `TransportClosed` kill (plan 7.5).
fn assert_abort_classified(order: Order) {
    use crate::actor::{
        PluginHandlerKind, PluginHandlerRef, PluginInvocationContext, PluginInvocationFailureKind,
        PluginInvocationRequest, PluginInvocationResult, PluginKey,
    };
    use crate::runtime::plugin_process::PluginExitCause;
    use crate::runtime::{PluginCancellationToken, PluginRuntime};
    use crate::session::RequestId;

    let seam = OrderSeam::new(order);
    ORDER_SEAM.with(|slot| *slot.borrow_mut() = Some(seam));
    let (process, _) = PluginProcess::spawn(&plain_config(), &load()).expect("loaded");

    let request = PluginInvocationRequest {
        request_id: RequestId("abort".to_string()),
        handler: PluginHandlerRef {
            plugin_key: PluginKey("order".to_string()),
            kind: PluginHandlerKind::Command,
            handler_id: "abort".to_string(),
        },
        timeout_ms: 1_000,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: None,
            metadata: None,
        },
        payload: opaque(json!({})),
    };
    let result = PluginRuntime::invoke(&process, request, PluginCancellationToken::new());
    assert!(matches!(
        result,
        PluginInvocationResult::Failed(ref failure)
            if failure.kind == PluginInvocationFailureKind::WorkerCrashed
    ));
    assert_eq!(
        process
            .exit()
            .expect("the exit settled the invocation")
            .cause,
        PluginExitCause::Crashed {
            signal: Some(libc::SIGABRT),
            code: None,
        }
    );
}

#[test]
fn an_abort_is_a_crash_when_the_eof_kill_comes_first() {
    assert_abort_classified(Order::EofFirst);
}

#[test]
fn an_abort_is_a_crash_when_the_exit_comes_first() {
    assert_abort_classified(Order::ExitFirst);
}

// ---------------------------------------------------------------------------
// Settlement versus the cancel grace (review I1) and slot retirement (I2).

use super::{Gate, TestSeams, HOLD_BEFORE_ARM, HOLD_BEFORE_CONSUME, TEST_SEAMS};
use crate::actor::{
    PluginHandlerKind, PluginHandlerRef, PluginInvocationContext, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginKey,
};
use crate::runtime::plugin_process::outbound::Lane;
use crate::runtime::plugin_process::supervisor::Expiry;
use crate::runtime::{PluginCancellationToken, PluginRuntime};
use crate::session::RequestId;
use std::time::Instant;

/// Bound for each event these tests wait for; expiry fails the test.
const WAIT: Duration = Duration::from_secs(30);
/// A grace that the tests hold the caller beyond.
const SHORT_GRACE: Duration = Duration::from_millis(200);

fn invocation(id: &str, handler_id: &str) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId(id.to_string()),
        handler: PluginHandlerRef {
            plugin_key: PluginKey("settle".to_string()),
            kind: PluginHandlerKind::Command,
            handler_id: handler_id.to_string(),
        },
        timeout_ms: 1_000,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: None,
            metadata: None,
        },
        payload: opaque(json!({})),
    }
}

/// A two-sided gate: the host side holds, the test side watches and releases.
fn gate() -> (Gate, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (reached_tx, reached_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    (
        Gate {
            reached: reached_tx,
            release: release_rx,
        },
        reached_rx,
        release_tx,
    )
}

fn event(rx: &mpsc::Receiver<()>, what: &str) {
    // timer: deadline — the named event arrives; expiry fails the test
    rx.recv_timeout(WAIT).unwrap_or_else(|_| panic!("{what}"));
}

/// A FIFO that the `block_after_signal` handler writes when it runs.
struct StartedFifo {
    dir: PathBuf,
    path: PathBuf,
    read: mpsc::Receiver<()>,
}

impl StartedFifo {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "botster-plugin-settle-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("started");
        mkfifo(&path);
        let (tx, read) = mpsc::channel();
        let reader = path.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            if File::open(&reader)
                .and_then(|mut fifo| fifo.read_to_string(&mut line))
                .is_ok()
            {
                let _ = tx.send(());
            }
        });
        Self { dir, path, read }
    }
}

impl Drop for StartedFifo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Harness {
    process: Arc<PluginProcess>,
    seams: Arc<TestSeams>,
    settled: mpsc::Receiver<()>,
    waiting: mpsc::Receiver<()>,
}

fn harness(
    max_in_flight: usize,
    started: Option<&StartedFifo>,
    writer_hold: Option<(Lane, Gate)>,
) -> Harness {
    let seams = Arc::new(TestSeams::default());
    let (settled_tx, settled) = mpsc::channel();
    let (waiting_tx, waiting) = mpsc::channel();
    *seams.settled.lock().expect("settled") = Some(settled_tx);
    *seams.waiting.lock().expect("waiting") = Some(waiting_tx);
    *seams.writer_hold.lock().expect("writer hold") = writer_hold;
    TEST_SEAMS.with(|slot| *slot.borrow_mut() = Some(seams.clone()));
    let mut config = plain_config();
    config.cancel_grace = SHORT_GRACE;
    config.max_in_flight_invokes = max_in_flight;
    if let Some(started) = started {
        config.env = vec![(
            OsString::from("PLUGIN_TEST_STARTED_FIFO"),
            started.path.clone().into_os_string(),
        )];
    }
    let (process, _) = PluginProcess::spawn(&config, &load()).expect("loaded");
    Harness {
        process: Arc::new(process),
        seams,
        settled,
        waiting,
    }
}

type HoldSlot = &'static std::thread::LocalKey<std::cell::RefCell<Option<Gate>>>;

/// Invoke on a new thread, with an optional caller-side hold armed there.
fn invoke_on_thread(
    process: &Arc<PluginProcess>,
    request: PluginInvocationRequest,
    token: &PluginCancellationToken,
    hold: Option<(HoldSlot, Gate)>,
) -> mpsc::Receiver<PluginInvocationResult> {
    let (tx, rx) = mpsc::channel();
    let process = process.clone();
    let token = token.clone();
    std::thread::spawn(move || {
        if let Some((slot, gate)) = hold {
            slot.with(|slot| *slot.borrow_mut() = Some(gate));
        }
        let _ = tx.send(PluginRuntime::invoke(&*process, request, token));
    });
    rx
}

fn result(rx: &mpsc::Receiver<PluginInvocationResult>) -> PluginInvocationResult {
    // timer: deadline — the invocation settles; expiry fails the test
    rx.recv_timeout(WAIT).expect("the invocation settles")
}

/// Wait until the supervisor has processed every deadline up to `at`: it
/// fires a probe armed for `at` only after all earlier expiries.
fn supervisor_passed(process: &PluginProcess, at: Instant) {
    let (tx, rx) = mpsc::channel();
    process.shared.supervisor.arm_expiry(at, Expiry::Probe(tx));
    event(&rx, "the supervisor passes the grace instant");
}

fn no_kill(process: &PluginProcess) {
    let state = process.shared.killer.state_for_test();
    assert!(state.is_none(), "a healthy process was killed: {state:?}");
    assert!(process.exit().is_none(), "a healthy process exited");
}

/// A queued invocation is cancelled: `Cancel` goes out, the grace is armed,
/// the child answers `Cancelled` at once, and the caller is held before it
/// consumes that outcome until the grace instant has passed. The result path
/// owns the disarm, so nothing kills the healthy process.
#[test]
fn a_timely_result_disarms_the_grace_while_its_caller_is_held() {
    let started = StartedFifo::new("held");
    let h = harness(2, Some(&started), None);
    let _running = invoke_on_thread(
        &h.process,
        invocation("running", "block_after_signal"),
        &PluginCancellationToken::new(),
        None,
    );
    event(&started.read, "the running handler started");
    let (hold, held, release) = gate();
    let token = PluginCancellationToken::new();
    let queued = invoke_on_thread(
        &h.process,
        invocation("queued", "echo"),
        &token,
        Some((&HOLD_BEFORE_CONSUME, hold)),
    );
    let cancelled_at = Instant::now();
    token.cancel();
    event(&h.settled, "the queued invocation's result settles");
    event(&held, "the caller is held before it consumes the result");

    supervisor_passed(&h.process, cancelled_at + SHORT_GRACE + SHORT_GRACE);
    no_kill(&h.process);
    let _ = release.send(());
    assert!(matches!(
        result(&queued),
        PluginInvocationResult::Failed(ref failure)
            if failure.kind == PluginInvocationFailureKind::Cancelled
    ));
    h.process.kill();
}

/// The result settles between queuing `Cancel` and arming the grace: the
/// arm sees the settled outcome, and no deadline remains to kill.
#[test]
fn a_result_before_the_grace_is_armed_leaves_no_deadline() {
    let started = StartedFifo::new("arm");
    let h = harness(2, Some(&started), None);
    let _running = invoke_on_thread(
        &h.process,
        invocation("running", "block_after_signal"),
        &PluginCancellationToken::new(),
        None,
    );
    event(&started.read, "the running handler started");
    let (hold, held, release) = gate();
    let token = PluginCancellationToken::new();
    let queued = invoke_on_thread(
        &h.process,
        invocation("queued", "echo"),
        &token,
        Some((&HOLD_BEFORE_ARM, hold)),
    );
    token.cancel();
    event(&held, "the canceller is held before it arms the grace");
    event(&h.settled, "the result settles first");
    let _ = release.send(());
    assert!(matches!(
        result(&queued),
        PluginInvocationResult::Failed(ref failure)
            if failure.kind == PluginInvocationFailureKind::Cancelled
    ));

    supervisor_passed(&h.process, Instant::now() + SHORT_GRACE + SHORT_GRACE);
    no_kill(&h.process);
    h.process.kill();
}

/// With one slot, the writer is held after sending the first `Invoke` and
/// before retiring it. The first invocation completes; the second must wait
/// for the slot as an event, not be refused or killed, and completes once
/// the writer retires the first frame.
#[test]
fn a_next_invocation_waits_for_the_previous_frame_to_retire() {
    let (hold, held, release) = gate();
    let h = harness(1, None, Some((Lane::Invoke, hold)));
    let first = invoke_on_thread(
        &h.process,
        invocation("first", "echo"),
        &PluginCancellationToken::new(),
        None,
    );
    event(&held, "the writer is held after sending the first invoke");
    assert!(matches!(
        result(&first),
        PluginInvocationResult::Completed(_)
    ));

    let second = invoke_on_thread(
        &h.process,
        invocation("second", "echo"),
        &PluginCancellationToken::new(),
        None,
    );
    event(&h.waiting, "the second invocation waits for the slot");
    no_kill(&h.process);
    let _ = release.send(());
    assert!(matches!(
        result(&second),
        PluginInvocationResult::Completed(_)
    ));
    no_kill(&h.process);
    drop(h.seams);
    h.process.kill();
}
