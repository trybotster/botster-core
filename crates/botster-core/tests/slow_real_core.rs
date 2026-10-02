//! The real `Core` over a real data directory, a real lock, a real control socket and a real wake object. Slow tier (BUILD.md
//! testing rule 2): the file lock, the fsync of a row and the descriptor of the wake object are real operating-system
//! conditions that an in-memory edge cannot prove.
//!
//! Clause: Core LC-1, LC-2, LC-9, LC-12, DP-8, TH-2, TM-6, AD-6.
#![cfg(feature = "slow")]

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
