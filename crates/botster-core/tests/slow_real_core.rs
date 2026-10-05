//! The real `Core` over a real data directory, a real lock, a real control socket and a real wake object. Slow tier (BUILD.md
//! testing rule 2): the file lock, the fsync of a row and the descriptor of the wake object are real operating-system
//! conditions that an in-memory edge cannot prove.
//!
//! Clause: Core LC-1, LC-2, LC-9, LC-12, DP-8, TH-2, TM-6, AD-6.
#![cfg(feature = "slow")]
// The test is the host: it reads the real clock and passes the time to `pump` (Core TM-1).
#![allow(clippy::disallowed_methods)]

mod common;

use botster_core::prelude::*;
use botster_core::Core;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn config(dir: &std::path::Path) -> OpenConfig {
    OpenConfig {
        data_dir: dir.join("d"),
        worker_path: Some(PathBuf::from("/bin/true")),
        limits: CoreLimits::default(),
    }
}

fn request() -> SpawnRequest {
    SpawnRequest {
        argv: vec!["/bin/true".into()],
        env: BTreeMap::new(),
        cwd: "/".into(),
        size: Size {
            rows: 24,
            cols: 80,
            cell_px: None,
        },
        labels: BTreeMap::new(),
        color_profile: None,
        notification_policy: None,
        size_policy: None,
    }
}

fn pump(core: &mut Core) -> Vec<Event> {
    let mut out = Vec::new();
    loop {
        let report = core.pump(Now {
            monotonic: Instant::now(),
            unix: 1_000_000,
        });
        out.extend(core.poll_events(64));
        if !report.more {
            return out;
        }
    }
}

/// Core LC-2: a second `open` of one directory is refused while the first is open, with the real `flock`; the lock ends with
/// the handle (LC-12: nothing else ends).
#[test]
fn a_second_open_is_refused_until_the_first_is_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let first = Core::open(config(tmp.path())).expect("open");
    assert_eq!(
        Core::open(config(tmp.path())).err().expect("held").code,
        ErrorCode::DataDirInUse
    );
    drop(first);
    assert!(Core::open(config(tmp.path())).is_ok());
}

/// Core LC-1, 9B: no worker path and a zero limit are refused before the directory is touched.
#[test]
fn open_checks_the_config_first() {
    let tmp = tempfile::tempdir().unwrap();
    let mut no_worker = config(tmp.path());
    no_worker.worker_path = None;
    assert_eq!(
        Core::open(no_worker).err().expect("refused").code,
        ErrorCode::MissingWorkerPath
    );
    let mut zero = config(tmp.path());
    zero.limits.max_sessions = 0;
    assert!(matches!(
        Core::open(zero).err().expect("refused").code,
        ErrorCode::InvalidConfig { .. }
    ));
    assert!(
        !tmp.path().join("d").exists(),
        "a refused config creates nothing"
    );
}

/// Core LC-3, LC-9, DP-8, AD-1: a `Created` row is durable: a new handle on the directory adopts it with its labels, under a
/// higher epoch, and the instance survives (ID-2).
#[test]
fn a_created_row_survives_a_drop_and_a_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let mut core = Core::open(config(tmp.path())).expect("open");
    let create = core
        .begin(Op::Create {
            session: SessionId("s1".into()),
            request: request(),
        })
        .unwrap();
    let events = pump(&mut core);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == create)));
    let labels = BTreeMap::from([("k".to_string(), "v".to_string())]);
    core.begin(Op::UpdateMetadata {
        id: SessionId("s1".into()),
        labels,
    })
    .unwrap();
    pump(&mut core);
    let instance = events.iter().find_map(|e| match e {
        Event::SessionState { instance, .. } => Some(instance.clone()),
        _ => None,
    });
    drop(core);
    let mut again = Core::open(config(tmp.path())).expect("reopen");
    let adopt = again.begin(Op::AdoptAll).unwrap();
    let events = pump(&mut again);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, .. } if *op == adopt)));
    let record = again
        .get(&SessionId("s1".into()))
        .expect("the row was adopted");
    assert_eq!(record.state, SessionState::Created);
    assert_eq!(record.labels.get("k").map(String::as_str), Some("v"));
    let adopted = events.iter().find_map(|e| match e {
        Event::SessionState { instance, .. } => Some(instance.clone()),
        _ => None,
    });
    assert_eq!(adopted, instance, "ID-2: the instance survives adoption");
}

/// Core TM-6, TH-2: a call that leaves work signals the real wake object before it returns, a waiter on another thread sees
/// it, and a pump that leaves no work clears it.
#[test]
fn begin_wakes_a_waiter_on_another_thread_and_the_pump_clears_it() {
    let tmp = tempfile::tempdir().unwrap();
    let mut core = Core::open(config(tmp.path())).expect("open");
    let wake = core.wake_handle();
    assert_eq!(wake.wait(Duration::ZERO), Wake::TimedOut);
    core.begin(Op::Create {
        session: SessionId("s1".into()),
        request: request(),
    })
    .unwrap();
    std::thread::scope(|scope| {
        let waiter = scope.spawn(|| wake.wait(Duration::from_secs(5))); // timer: deadline — a failing run must not hang
        assert_eq!(waiter.join().unwrap(), Wake::Woken);
    });
    pump(&mut core);
    assert_eq!(
        wake.wait(Duration::ZERO),
        Wake::TimedOut,
        "TM-6: the pump left no work"
    );
}

/// Core 13 (rule 5): the wake handle has a descriptor that the host can poll.
#[test]
fn the_wake_handle_has_a_real_descriptor() {
    let tmp = tempfile::tempdir().unwrap();
    let core = Core::open(config(tmp.path())).expect("open");
    assert!(core.wake_handle().fd() >= 0);
}

/// Core AD-6: the data directory and its registry are private to the host's user.
#[test]
fn the_data_directory_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let _core = Core::open(config(tmp.path())).expect("open");
    for path in [tmp.path().join("d"), tmp.path().join("d").join("rows")] {
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", path.display());
    }
}

fn sid(name: &str) -> SessionId {
    SessionId(name.into())
}

/// Core AD-2, LC-4, TM-6: a worker that ends before it connects is seen at once, not at a deadline: the reaper thread wakes
/// the host, the exit is polled from the real process edge, and the start ends. The worker ends only after the host is idle
/// (it reads a FIFO that the test closes), so only the reaper's wake can end the wait.
#[test]
fn a_worker_that_exits_before_it_connects_ends_the_start_at_once() {
    let tmp = tempfile::tempdir().unwrap();
    let ready = tmp.path().join("ready");
    common::mkfifo(&ready);
    // The worker says that it runs (an external `/bin/echo` into a FIFO), then waits, without the CPU, until the test ends it
    // with a signal or the test process is gone.
    let worker = common::ScriptWorker::new(
        tmp.path(),
        &format!(
            "/bin/echo ready > '{}'\n{}",
            ready.display(),
            common::WAIT_WHILE_THE_PARENT_LIVES
        ),
    );
    let mut open = config(tmp.path());
    open.worker_path = Some(worker.path.clone());
    // The start deadline is far away: a start that ends before it ended because the exit was seen, not because time ran out.
    open.limits.startup = Duration::from_secs(120);
    let mut core = Core::open(open).expect("open");
    let wake = core.wake_handle();
    core.begin(Op::Create {
        session: sid("s1"),
        request: request(),
    })
    .unwrap();
    pump(&mut core);
    let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    let began = Instant::now();
    let mut events = pump(&mut core);
    assert_eq!(
        core.get(&sid("s1")).unwrap().state,
        SessionState::Starting,
        "the host is idle and the worker runs: {events:?}"
    );
    // The worker ends now, while the host waits: the test reads that it runs, then ends it with `SIGTERM`.
    let (told, heard) = std::sync::mpsc::channel();
    let fifo = ready.clone();
    std::thread::spawn(move || {
        let _ = told.send(std::fs::read_to_string(fifo));
    });
    let said = heard
        // timer: deadline — bounds the wait for the worker's start
        .recv_timeout(Duration::from_secs(10))
        .expect("the worker runs");
    assert_eq!(said.unwrap().trim(), "ready");
    let pid = worker.pid().expect("the worker recorded its pid");
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).unwrap();
    // The host pumps only after a wake (TM-6): every wait must end by a wake, never by its timeout. The worker never
    // connects, so the wake that ends the start is the reaper's, through `RealEdges` and `PollWake`.
    while !events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, .. } if *op == start))
    {
        // timer: deadline — a host that never settles must fail the test, not spin
        assert!(began.elapsed() < Duration::from_secs(100), "{events:?}");
        let woke = wake
            // timer: deadline — a failing run must not hang; the start deadline (120 s) is longer than this wait
            .wait(Duration::from_secs(100));
        assert_eq!(woke, Wake::Woken, "the exit woke the host: {events:?}");
        events.extend(pump(&mut core));
    }
    assert!(
        events.iter().any(
            |e| matches!(e, Event::Completed { op, result: OpResult::Err(_) } if *op == start)
        ),
        "{events:?}"
    );
    assert!(matches!(
        core.get(&sid("s1")).unwrap().state,
        SessionState::Lost(_) | SessionState::Exited(_)
    ));
}

/// Core AD-6: a client that connects to the control socket is accepted and read; a hello for an instance that no start waits
/// for is answered by closing the link, which the client sees as the end of the stream.
#[test]
fn a_hello_for_an_unknown_instance_is_closed() {
    use botster_core_link::frame::{encode_frame, FrameType};
    use botster_core_link::hello::Hello;
    use std::io::{Read, Write};
    let tmp = tempfile::tempdir().unwrap();
    let mut core = Core::open(config(tmp.path())).expect("open");
    let wake = core.wake_handle();
    let socket = tmp.path().join("d").join("c");
    let mut client = std::os::unix::net::UnixStream::connect(&socket).expect("the host listens");
    let hello = Hello {
        protocol: 1,
        instance: InstanceId("9-9".into()),
        proof: botster_core_link::proof::token_proof(
            &[7; botster_core_link::proof::TOKEN_LEN],
            &InstanceId("9-9".into()),
            1,
        ),
        host_epoch: 1,
    };
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    let mut frame = Vec::new();
    encode_frame(FrameType::HELLO, &payload, 1 << 20, &mut frame).unwrap();
    // The whole hello is in the socket before the host looks: one wake and one pump read it and close the link.
    client.write_all(&frame).unwrap();
    let (said, heard) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut rest = Vec::new();
        let _ = said.send(client.read_to_end(&mut rest).map(|_| rest));
    });
    let woke = wake
        // timer: deadline — a host that is never woken fails the test instead of hanging it
        .wait(Duration::from_secs(8));
    assert_eq!(woke, Wake::Woken, "TM-6: the connection wakes the host");
    pump(&mut core);
    let sent = heard
        // timer: deadline — a link that the host does not close fails the test instead of hanging it
        .recv_timeout(Duration::from_secs(8))
        .expect("the host closed the link")
        .unwrap();
    assert!(
        sent.is_empty(),
        "the host sent bytes to an unknown worker: {sent:?}"
    );
}

/// Plan R12, testing rule 10 (review finding F28): a test whose cleanup never runs leaves no worker. The session is started
/// and the host is dropped with no `Stop` (LC-12: the worker survives the host); the guard in test code then kills the
/// worker's group and reaps it.
#[test]
fn a_worker_is_not_left_when_the_cleanup_of_a_test_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let ready = tmp.path().join("ready");
    common::mkfifo(&ready);
    let worker = common::ScriptWorker::new(
        tmp.path(),
        &format!(
            "/bin/echo ready > '{}'\n{}",
            ready.display(),
            common::WAIT_WHILE_THE_PARENT_LIVES
        ),
    );
    let mut open = config(tmp.path());
    open.worker_path = Some(worker.path.clone());
    let mut core = Core::open(open).expect("open");
    core.begin(Op::Create {
        session: sid("s1"),
        request: request(),
    })
    .unwrap();
    pump(&mut core);
    core.begin(Op::Start { id: sid("s1") }).unwrap();
    pump(&mut core);
    let (told, heard) = std::sync::mpsc::channel();
    let fifo = ready.clone();
    std::thread::spawn(move || {
        let _ = told.send(std::fs::read_to_string(fifo));
    });
    heard
        // timer: deadline — bounds the wait for the worker's start
        .recv_timeout(Duration::from_secs(10))
        .expect("the worker runs")
        .unwrap();
    let pid = worker.pid().expect("the worker recorded its pid");
    drop(core);
    assert!(
        rustix::process::test_kill_process(pid).is_ok(),
        "the worker outlives the host (LC-12)"
    );
    drop(worker);
    assert!(
        rustix::process::test_kill_process(pid).is_err(),
        "the guard killed and reaped the worker"
    );
}

/// Lead ruling on audit A1, Core AD-1, AD-2, A10-2: through the real registry, a damaged row is `Lost(RegistryCorrupt)` under
/// the id that its path names, and keeps that id in use; a file that Core did not write is counted, left untouched, and does
/// not block `AdoptAll`.
#[test]
fn a_damaged_row_is_registry_corrupt_and_a_foreign_file_is_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let mut core = Core::open(config(tmp.path())).expect("open");
    core.begin(Op::Create {
        session: sid("s1"),
        request: request(),
    })
    .unwrap();
    pump(&mut core);
    drop(core);
    let kind = tmp.path().join("d").join("rows").join("session");
    let rows: Vec<PathBuf> = std::fs::read_dir(&kind)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(rows.len(), 1, "one session, one row file: {rows:?}");
    std::fs::write(&rows[0], b"\xff damaged").unwrap();
    let foreign = tmp.path().join("d").join("rows").join("notes.txt");
    std::fs::write(&foreign, b"someone else's").unwrap();
    let mut again = Core::open(config(tmp.path())).expect("reopen");
    let adopt = again.begin(Op::AdoptAll).unwrap();
    let events = pump(&mut again);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == adopt)),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::SessionState { id, state: SessionState::Lost(LostReason::RegistryCorrupt), .. } if *id == sid("s1")
        )),
        "{events:?}"
    );
    assert_eq!(
        again
            .begin(Op::Create {
                session: sid("s1"),
                request: request(),
            })
            .unwrap_err()
            .code,
        ErrorCode::IdInUse
    );
    assert_eq!(again.diagnostics()["edges"]["foreign_registry_files"], 1);
    assert_eq!(std::fs::read(&foreign).unwrap(), b"someone else's");
}
