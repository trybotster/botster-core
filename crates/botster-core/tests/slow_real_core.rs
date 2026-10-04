//! The real `Core` over a real data directory, a real lock, a real control socket and a real wake object. Slow tier (BUILD.md
//! testing rule 2): the file lock, the fsync of a row and the descriptor of the wake object are real operating-system
//! conditions that an in-memory edge cannot prove.
//!
//! Clause: Core LC-1, LC-2, LC-9, LC-12, DP-8, TH-2, TM-6, AD-6.
#![cfg(feature = "slow")]

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
    let gate = tmp.path().join("gate");
    common::mkfifo(&gate);
    // The worker blocks reading the FIFO (an external `/bin/cat`, not a shell builtin) and ends when the test closes it.
    let mut worker = common::ScriptWorker::new(
        tmp.path(),
        &format!("exec /bin/cat '{}' >/dev/null", gate.display()),
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
    let mut events = pump(&mut core);
    assert_eq!(
        core.get(&sid("s1")).unwrap().state,
        SessionState::Starting,
        "the host is idle and the worker runs: {events:?}"
    );
    // The worker ends now: opening the FIFO waits for its reader, and closing it ends `cat`.
    worker.disarm();
    drop(std::fs::OpenOptions::new().write(true).open(&gate).unwrap());
    // The host pumps only after a wake (TM-6): every wait must end by a wake, never by its timeout. The worker never
    // connects, so the wake that ends the start is the reaper's, through `RealEdges` and `PollWake`.
    while !events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, .. } if *op == start))
    {
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
    client.write_all(&frame).unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let began = Instant::now();
    let mut buf = [0u8; 16];
    loop {
        pump(&mut core);
        match client.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => panic!("the host sent bytes to an unknown worker"),
            Err(_) => {}
        }
        // timer: deadline — a failing run must not hang
        assert!(
            began.elapsed() < Duration::from_secs(8),
            "the link was not closed"
        );
        let _ = wake.wait(Duration::from_millis(50));
    }
}
