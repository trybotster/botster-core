//! The real worker binary with real payloads on real PTYs. Slow tier (BUILD.md testing rule 2): the PTY, the process group,
//! the exit watch and the worker-control signal are real operating-system conditions.
//!
//! The binary is the prebuilt one (`cargo xtask prebuild-worker` puts it in `target/candidate/`); a test never builds it.
//! Every worker and payload that a test starts is ended on every exit path, panics included: a Core-started session is
//! removed by [`Sessions`] on drop, and a worker that a test starts itself is killed by [`OwnedWorker`].
//!
//! Clause: Core EV-4, Core LC-5, Core LC-6, Core LC-7, Core AD-6, Core AD-7.
#![cfg(feature = "slow")]

use botster_core::prelude::*;
use botster_core::Core;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
use botster_core_link::hello::Hello;
use botster_core_link::launch::WorkerLaunch;
use botster_core_link::msg::{HostMsg, LaunchSpec, WorkerMsg};
use botster_core_link::proof::token_proof;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// `target/candidate/botster-worker`, found from this test binary (`target/<profile>/deps/<test>`), so it holds for any
/// target directory.
fn worker_binary() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    let target = exe
        .parent()
        .and_then(|deps| deps.parent())
        .and_then(|profile| profile.parent())
        .expect("target/<profile>/deps");
    let binary = target.join("candidate").join("botster-worker");
    assert!(
        binary.is_file(),
        "{} is missing: run `cargo xtask prebuild-worker` first",
        binary.display()
    );
    binary
}

/// A short and canonical temporary root: a Unix socket path is limited to about 104 bytes (plan section 5).
fn temp_root() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("p3")
        .tempdir_in("/tmp")
        .expect("a temporary root")
}

fn request(script: &str) -> SpawnRequest {
    SpawnRequest {
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        env: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
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

/// The sessions of one real Core. On drop it removes every session it created, so no worker or payload outlives the test.
struct Sessions {
    core: Core,
    ids: Vec<SessionId>,
    events: Vec<Event>,
}

impl Sessions {
    fn open(root: &std::path::Path, stop_grace: Duration) -> Sessions {
        let limits = CoreLimits {
            stop_grace,
            ..CoreLimits::default()
        };
        let core = Core::open(OpenConfig {
            data_dir: root.join("d"),
            worker_path: Some(worker_binary()),
            limits,
        })
        .expect("open");
        Sessions {
            core,
            ids: Vec::new(),
            events: Vec::new(),
        }
    }

    fn now() -> Now {
        Now {
            monotonic: Instant::now(),
            unix: 1_000_000,
        }
    }

    /// Pumps until `wanted` matches an event, waiting on the wake handle between pumps (no sleep).
    /// Pumps until an event matches `wanted`, waiting on the wake handle between pumps (no sleep). Polled events that no
    /// wait has taken stay queued for the next wait, so an event that comes with an earlier one is never lost.
    fn until(&mut self, what: &str, wanted: impl Fn(&Event) -> bool) -> Event {
        // timer: deadline — the limit of a wait for a real worker; not a contract value.
        let limit = Instant::now() + Duration::from_secs(20);
        let wake = self.core.wake_handle();
        loop {
            if let Some(at) = self.events.iter().position(&wanted) {
                return self.events.remove(at);
            }
            let report = self.core.pump(Sessions::now());
            let polled = self.core.poll_events(64);
            let new = !polled.is_empty();
            self.events.extend(polled);
            let now = Instant::now();
            assert!(
                now < limit,
                "no {what}; unclaimed events: {:#?}",
                self.events
            );
            if !report.more && !new {
                let until = self
                    .core
                    .next_deadline()
                    .map_or(limit, |d| d.min(limit))
                    .max(now);
                let _ = wake.wait(until - now);
            }
        }
    }

    fn completed(&mut self, op: OpId) -> OpResult {
        match self.until(
            "completion",
            |e| matches!(e, Event::Completed { op: o, .. } if *o == op),
        ) {
            Event::Completed { result, .. } => result,
            _ => unreachable!(),
        }
    }

    fn start(&mut self, id: &str, script: &str) -> SessionId {
        let session = SessionId(id.into());
        let create = self
            .core
            .begin(Op::Create {
                session: session.clone(),
                request: request(script),
            })
            .expect("create");
        self.ids.push(session.clone());
        assert!(matches!(self.completed(create), OpResult::Ok(_)));
        let start = self
            .core
            .begin(Op::Start {
                id: session.clone(),
            })
            .expect("start");
        let result = self.completed(start);
        assert!(matches!(result, OpResult::Ok(_)), "{result:?}");
        session
    }

    fn exit_of(&mut self, session: &SessionId) -> Exit {
        let event = self.until("exit", |e| {
            matches!(e, Event::SessionState { id, state: SessionState::Exited(_), .. } if id == session)
        });
        match event {
            Event::SessionState {
                state: SessionState::Exited(exit),
                ..
            } => exit,
            _ => unreachable!(),
        }
    }
}

impl Drop for Sessions {
    fn drop(&mut self) {
        for id in std::mem::take(&mut self.ids) {
            if let Ok(op) = self.core.begin(Op::Remove { id }) {
                let wanted =
                    move |e: &Event| matches!(e, Event::Completed { op: o, .. } if *o == op);
                if std::thread::panicking() {
                    // Best effort: a second panic would abort.
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        self.until("remove", wanted)
                    }));
                } else {
                    self.until("remove", wanted);
                }
            }
        }
    }
}

/// Core EV-4: a real payload that exits with a code is `Exited{code}` with no signal; one that dies by a signal is
/// `Exited{signal}` with no code.
#[test]
fn ev_4_a_real_exit_carries_the_code_or_the_signal() {
    let root = temp_root();
    let mut s = Sessions::open(root.path(), Duration::from_secs(5));
    let coded = s.start("s1", "echo out; exit 3");
    let exit = s.exit_of(&coded);
    assert_eq!((exit.code, exit.signal), (Some(3), None));
    let signalled = s.start("s2", "kill -TERM $$");
    let exit = s.exit_of(&signalled);
    assert_eq!((exit.code, exit.signal), (None, Some(15)));
}

/// Core LC-5: `Stop` ends the payload with the graceful request; a payload that ignores it is killed at `stop_grace`; the
/// worker lives on until `Remove` (LC-7), which then ends it.
#[test]
fn lc_5_stop_asks_then_kills_and_the_worker_survives() {
    let root = temp_root();
    let mut s = Sessions::open(root.path(), Duration::from_millis(300));
    let polite = s.start("s1", "exec sleep 30");
    let stop = s.core.begin(Op::Stop { id: polite.clone() }).unwrap();
    s.completed(stop);
    let exit = s.exit_of(&polite);
    assert_eq!((exit.signal, exit.cause), (Some(15), ExitCause::HostStop));
    let stubborn = s.start("s2", "trap '' TERM; exec sleep 30");
    let stop = s
        .core
        .begin(Op::Stop {
            id: stubborn.clone(),
        })
        .unwrap();
    s.completed(stop);
    let exit = s.exit_of(&stubborn);
    assert_eq!((exit.signal, exit.cause), (Some(9), ExitCause::Killed));
    let record = s
        .core
        .get(&stubborn)
        .expect("the session stays until Remove");
    assert!(matches!(record.state, SessionState::Exited(_)));
}

/// Core LC-6: `Signal` reaches the payload's group: a child that the payload started in its group dies with it, so the PTY
/// output ends and the exit is reported.
#[test]
fn lc_6_signal_reaches_the_whole_group() {
    let root = temp_root();
    let mut s = Sessions::open(root.path(), Duration::from_secs(5));
    let session = s.start("s1", "sleep 30 & wait");
    let signal = s
        .core
        .begin(Op::Signal {
            id: session.clone(),
            sig: Signal::Int,
        })
        .unwrap();
    // `sh -c` ignores nothing here: SIGINT ends the shell and its background `sleep`, both in the payload's group.
    let result = s.completed(signal);
    assert!(matches!(result, OpResult::Ok(_)), "{result:?}");
    let exit = s.exit_of(&session);
    assert_eq!(exit.signal, Some(2));
}

/// A worker that a test starts itself, killed with its group on drop.
struct OwnedWorker(Child);

impl Drop for OwnedWorker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The host side of one control link, written with the link's own codec. It checks only what the worker sends.
struct Link {
    stream: UnixStream,
    decoder: FrameDecoder,
}

impl Link {
    fn send(&mut self, kind: FrameType, payload: &[u8]) {
        let mut out = Vec::new();
        encode_frame(kind, payload, u32::MAX, &mut out).unwrap();
        self.stream.write_all(&out).unwrap();
    }

    fn msg(&mut self, msg: &HostMsg) {
        let mut payload = Vec::new();
        msg.encode(&mut payload);
        self.send(FrameType::HOST_MSG, &payload);
    }

    /// The next frame; the read times out at the socket (a marked deadline set by the caller).
    fn frame(&mut self) -> (FrameType, Vec<u8>) {
        let mut buf = [0u8; 4096];
        loop {
            if let Some(frame) = self.decoder.next_frame().unwrap() {
                return (frame.kind, frame.payload);
            }
            let n = self
                .stream
                .read(&mut buf)
                .expect("a frame before the deadline");
            assert!(n > 0, "the worker closed the link");
            let mut rest = &buf[..n];
            while !rest.is_empty() {
                let took = self.decoder.push(rest);
                rest = &rest[took..];
                if took == 0 {
                    break;
                }
            }
        }
    }

    fn report(&mut self) -> WorkerMsg {
        let (kind, payload) = self.frame();
        assert_eq!(kind, FrameType::WORKER_MSG);
        WorkerMsg::decode(&payload).unwrap()
    }
}

/// Core LC-5 and the P1 interface (F7): `SIGUSR1` on the worker ends its payload without a host request: the graceful
/// request, then the group kill at `stop_grace`. The worker keeps running, reports the exit, and ends on `Remove` (LC-7).
#[test]
fn lc_5_the_worker_control_signal_ends_the_payload_and_the_worker_stays() {
    let root = temp_root();
    let socket = root.path().join("c");
    let listener = UnixListener::bind(&socket).unwrap();
    let fifo = root.path().join("f");
    let made = Command::new("/usr/bin/mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success());
    let launch = WorkerLaunch {
        control: socket,
        instance: InstanceId("1-1".into()),
        host_epoch: 1,
        token: [5; 32],
    };
    let mut command = Command::new(worker_binary());
    command.args(launch.args()).env_clear().envs(launch.env());
    let worker = OwnedWorker(command.spawn().unwrap());
    let (stream, _) = listener.accept().unwrap();
    // timer: deadline — the limit of a wait for a real worker's frame; not a contract value.
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut link = Link {
        stream,
        decoder: FrameDecoder::new(1 << 20),
    };
    let (kind, payload) = link.frame();
    assert_eq!(kind, FrameType::HELLO);
    let hello = Hello::decode(&payload).unwrap();
    let proof = token_proof(&launch.token, &launch.instance, launch.host_epoch);
    assert_eq!(hello.proof, proof, "AD-6");
    let mut reply = Vec::new();
    Hello {
        protocol: 1,
        instance: launch.instance.clone(),
        proof,
        host_epoch: 1,
    }
    .encode(&mut reply)
    .unwrap();
    link.send(FrameType::HELLO, &reply);
    link.msg(&HostMsg::Launch(Box::new(LaunchSpec {
        argv: vec![
            "/bin/sh".into(),
            "-c".into(),
            format!(
                "trap '' TERM; /bin/echo up > {}; exec sleep 30",
                fifo.display()
            ),
        ],
        env: BTreeMap::new(),
        cwd: "/".into(),
        size: request("").size,
        color_profile: None,
        notification_policy: NotificationPolicy::All,
        size_policy: SizePolicy::Latest,
        link_frame_bound: 1 << 20,
        stop_grace_ms: 200,
    })));
    let WorkerMsg::Launched { payload, .. } = link.report() else {
        panic!("not launched");
    };
    assert!(payload.pid > 0);
    // The payload ignores TERM once it has written to the FIFO. External `/bin/echo` and a blocking open: a signal cannot
    // interrupt a shell builtin's FIFO open here.
    let mut up = String::new();
    std::fs::File::open(&fifo)
        .unwrap()
        .read_to_string(&mut up)
        .unwrap();
    assert_eq!(up, "up\n");
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(i32::try_from(worker.0.id()).unwrap()).unwrap(),
        rustix::process::Signal::USR1,
    )
    .unwrap();
    assert_eq!(
        link.report(),
        WorkerMsg::Exited {
            code: None,
            signal: Some(9)
        },
        "TERM is ignored, so the kill of the grace ends it"
    );
    link.msg(&HostMsg::Remove);
    assert!(matches!(link.report(), WorkerMsg::RemoveResult { .. }));
    let mut worker = worker;
    let status = worker.0.wait().unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "LC-7: the worker ends after the result"
    );
}
