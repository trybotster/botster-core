//! Shared by the real-harness tests and their fixture binary (`tests/bin/fixture.rs`).
#![allow(dead_code)]

use botster_core_conformance::{CoreHarness, OpenSpec, WorkerBuild};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::ProcessIdentity;
use botster_core_testkit::candidate::Candidate;
use botster_core_testkit::real::guard::SETTLE;
use botster_core_testkit::real::RealCoreHarness;
use botster_probe_script::{Script, Step};
use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The verified prebuilt binaries (`cargo xtask prebuild-worker`).
pub fn candidate() -> Candidate {
    let dir = Candidate::beside_test_binary().expect("the candidate directory");
    Candidate::locate(&dir).unwrap_or_else(|error| panic!("{error}"))
}

pub fn harness() -> RealCoreHarness {
    RealCoreHarness::new(candidate()).expect("the harness's root and guard")
}

/// Opens the handle `h` over the directory `d` with the current worker and the default limits.
pub fn open(harness: &mut RealCoreHarness) -> Box<dyn CoreApi> {
    let spec = OpenSpec {
        handle: "h".into(),
        data_dir: harness.data_dir("d"),
        worker: harness.worker(WorkerBuild::Current),
        limits: serde_json::json!({}),
    };
    harness.open(&spec).expect("a real Core")
}

/// The probe script of `steps`, as `argv[1]` of the probe (design 6.2).
pub fn script(steps: Vec<Step>) -> String {
    serde_json::to_string(&Script::<serde_json::Value> {
        program: steps,
        fake: serde_json::Value::Null,
    })
    .expect("a script serializes")
}

/// A payload that only `KILL` ends: it and a child of its group survive `TERM`.
pub fn stubborn(marker: Option<&str>) -> Vec<Step> {
    let mut steps = vec![
        Step::IgnoreSigterm {},
        Step::ForkChild {
            script: vec![Step::IgnoreSigterm {}, Step::Hold {}],
        },
    ];
    if let Some(marker) = marker {
        steps.push(Step::Print {
            bytes_hex: marker.bytes().map(|b| format!("{b:02x}")).collect(),
            fake: Default::default(),
        });
    }
    steps.push(Step::Hold {});
    steps
}

pub fn sid(name: &str) -> SessionId {
    SessionId(name.into())
}

/// Creates and starts the session `name` running the harness's probe with `steps`, and returns once `Start` completed.
pub fn start(
    harness: &RealCoreHarness,
    core: &mut dyn CoreApi,
    name: &str,
    steps: Vec<Step>,
) -> Vec<Event> {
    let request = SpawnRequest {
        argv: vec![harness.probe_binary(), script(steps)],
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
    };
    let create = core
        .begin(Op::Create {
            session: sid(name),
            request,
        })
        .expect("Create is admitted");
    let mut events = pump_until(core, |e| completed(e, create).is_some());
    assert!(
        matches!(completed(&events, create), Some(OpResult::Ok(_))),
        "{events:?}"
    );
    let start = core
        .begin(Op::Start { id: sid(name) })
        .expect("Start is admitted");
    events.extend(pump_until(core, |e| completed(e, start).is_some()));
    assert!(
        matches!(completed(&events, start), Some(OpResult::Ok(_))),
        "{events:?}"
    );
    events
}

/// The limit of a wait for Core: a host that never settles fails the test instead of hanging the run.
// timer: deadline — the limit of a wait for real processes and a real host; not a contract value.
pub const HOST_DEADLINE: Duration = Duration::from_secs(30);

/// Pumps until `done` holds for the events so far. The host pumps after each wake (TM-6).
pub fn pump_until(core: &mut dyn CoreApi, done: impl Fn(&[Event]) -> bool) -> Vec<Event> {
    let wake = core.wake_handle();
    let began = Instant::now();
    let mut events = Vec::new();
    loop {
        loop {
            let report = core.pump(Now {
                monotonic: Instant::now(),
                unix: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("after the epoch")
                    .as_secs(),
            });
            events.extend(core.poll_events(64));
            if !report.more {
                break;
            }
        }
        if done(&events) {
            return events;
        }
        let left = HOST_DEADLINE.saturating_sub(began.elapsed());
        assert!(!left.is_zero(), "the host never got there: {events:?}");
        // timer: deadline — the rest of HOST_DEADLINE; a wake that never comes fails the test.
        assert_eq!(wake.wait(left), Wake::Woken, "no wake came: {events:?}");
    }
}

pub fn completed(events: &[Event], op: OpId) -> Option<OpResult> {
    events.iter().find_map(|e| match e {
        Event::Completed { op: o, result } if *o == op => Some(result.clone()),
        _ => None,
    })
}

/// The last state that the events report for the session.
pub fn state(events: &[Event], id: &SessionId) -> Option<SessionState> {
    events.iter().rev().find_map(|e| match e {
        Event::SessionState { id: i, state, .. } if i == id => Some(*state),
        _ => None,
    })
}

/// How long an anchor of the default limits may take to end its group: its grace, then the guard's settle limit.
pub fn cleanup_deadline() -> Duration {
    CoreLimits::default().stop_grace + SETTLE
}

/// Waits for the end of the process that `identity` names, on the kernel's exit event: `pidfd` on Linux, `EVFILT_PROC` on
/// macOS. Neither needs the process to be a child, and neither reaps it, so no reaper of Core or the worker is raced (audit
/// A10). An identity whose pid is gone or names another process has ended already.
pub fn wait_exit(identity: ProcessIdentity, deadline: Duration) -> Result<(), String> {
    let ended = |watching: bool| {
        botster_core_sys::process::start_time(identity.pid) != Some(identity.start_time)
            || !watching
    };
    #[cfg(target_os = "linux")]
    {
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        use rustix::process::{pidfd_open, Pid, PidfdFlags};
        let pid = i32::try_from(identity.pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or("not a pid")?;
        let fd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(fd) => Some(fd),
            Err(rustix::io::Errno::SRCH) => None,
            Err(error) => return Err(format!("pidfd_open {}: {error}", identity.pid)),
        };
        if ended(fd.is_some()) {
            return Ok(());
        }
        let fd = fd.expect("watching");
        let timeout = Timespec::try_from(deadline).map_err(|e| e.to_string())?;
        loop {
            let mut fds = [PollFd::new(&fd, PollFlags::IN)];
            match poll(&mut fds, Some(&timeout)) {
                Ok(0) => return Err(format!("{identity:?} still runs after {deadline:?}")),
                Ok(_) => return Ok(()),
                Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(format!("poll: {error}")),
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use kqueue::{EventFilter, FilterFlag, Watcher};
        let pid = i32::try_from(identity.pid).map_err(|e| e.to_string())?;
        let mut watcher = Watcher::new().map_err(|e| e.to_string())?;
        watcher
            .add_pid(pid, EventFilter::EVFILT_PROC, FilterFlag::NOTE_EXIT)
            .map_err(|e| e.to_string())?;
        // A process that has already exited cannot be watched: the registration fails.
        let watching = watcher.watch().is_ok();
        if ended(watching) {
            return Ok(());
        }
        match watcher.poll(Some(deadline)) {
            Some(_) => Ok(()),
            None => Err(format!("{identity:?} still runs after {deadline:?}")),
        }
    }
}

/// Reads lines of `reader` on a helper thread until `stop` holds for one, within `deadline`. The lines, in order.
pub fn lines_until(
    reader: impl std::io::Read + Send + 'static,
    stop: fn(&str) -> bool,
    deadline: Duration,
) -> Vec<String> {
    use std::io::BufRead;
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(reader).lines() {
            let Ok(line) = line else { return };
            let last = stop(&line);
            if sent.send(line).is_err() || last {
                return;
            }
        }
    });
    let began = Instant::now();
    let mut lines = Vec::new();
    loop {
        let left = deadline.saturating_sub(began.elapsed());
        // timer: deadline — the rest of `deadline`; a process that never writes fails the test.
        match received.recv_timeout(left) {
            Ok(line) => {
                let last = stop(&line);
                lines.push(line);
                if last {
                    return lines;
                }
            }
            Err(error) => panic!("no line ended the read ({error}); lines so far: {lines:?}"),
        }
    }
}
