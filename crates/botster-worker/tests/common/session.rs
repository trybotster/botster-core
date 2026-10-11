//! The real worker binary with real payloads on real PTYs. Slow tier (BUILD.md testing rule 2): the PTY, the process group,
//! the exit watch and the worker's signals are real operating-system conditions.
//!
//! The integration fixture uses the prebuilt binary. The unit fixture starts a test observer that runs the same Driver.
//! Mutation tests therefore exercise the changed Driver, not an unchanged candidate binary. Tests never build a worker.
//! Each test starts its worker itself and speaks the control link with the link's own codec, checking only what the worker
//! sends. Each worker is an unreaped child of the test. A separate guard ends the payload group on Drop and panic.
//! The guard keeps a member in the payload session. That member kills its current group on socket EOF.
//! Production alone reaps the payload. [`OwnedWorker`] ends and reaps the worker. (Real Core with real workers is the real-process harness's suite, plan 4.2.)
//!
//! Clause: Core EV-4, Core LC-5, Core LC-6, Core LC-7, Core AD-6, Core AD-7.
#![cfg(feature = "slow")]

#[path = "../../../botster-core-sys/tests/common/payload_guard.rs"]
pub(crate) mod payload_guard;

#[path = "../../../botster-core-sys/tests/common/process_guard.rs"]
pub(crate) mod process_guard;

use botster_core_contract::prelude::*;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
use botster_core_link::hello::Hello;
use botster_core_link::launch::WorkerLaunch;
use botster_core_link::msg::{
    AdoptedPayload, HostMsg, LaunchSpec, Observation, PayloadId, WorkerMsg,
};
use botster_core_link::proof::token_proof;
use botster_test_process::{quoted, Blocker, Bounded, Deadline, Guard, OwnedChild};
use payload_guard::PayloadGuard;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

#[path = "candidate.rs"]
mod candidate;
use candidate::worker_binary;

/// A short and canonical temporary root: a Unix socket path is limited to about 104 bytes (plan section 5).
fn temp_root() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("p3")
        .tempdir_in("/tmp")
        .expect("a temporary root")
}

/// A FIFO in `root` (external `mkfifo`; the vault's FIFO rule: external commands, never shell builtins, at a FIFO).
fn fifo(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    let made = Command::new("/usr/bin/mkfifo").arg(&path).status().unwrap();
    assert!(made.success());
    path
}

/// A worker that a test starts itself.
struct OwnedWorker {
    worker: Child,
    payload_guard: Option<PayloadGuard>,
    observer_guard: Option<process_guard::GroupGuard>,
}

impl Drop for OwnedWorker {
    /// Through the worker, never a payload id (lead ruling on P1 F7): while the worker is our unreaped child, `SIGTERM`
    /// makes it end the payload group that it still holds, then itself.
    fn drop(&mut self) {
        // The payload guard's member starts ending the payload group; its report is read once the worker, which holds the
        // PTY master, has ended (see `PayloadGuard::release`).
        if let Some(guard) = self.payload_guard.as_mut() {
            guard.release();
        }
        drop(self.observer_guard.take());
        if let Ok(None) = self.worker.try_wait() {
            if let Some(pid) =
                rustix::process::Pid::from_raw(i32::try_from(self.worker.id()).unwrap_or(0))
            {
                end_child_worker(pid);
            }
        }
        drop(self.payload_guard.take());
    }
}

/// Ends a worker that is an unreaped child of this process and of nothing else: `SIGTERM`; `SIGKILL` if it has not ended by
/// the deadline. The observer only observes the exit (`waitid` with `WNOWAIT`), and nothing else reaps the child, so the
/// pid stays the worker's through the last signal; the reap comes last.
fn end_child_worker(pid: rustix::process::Pid) {
    use botster_core_sys::signal::signal_process;
    use rustix::process::{waitid, waitpid, Signal, WaitId, WaitIdOptions, WaitOptions};
    let raw = pid.as_raw_nonzero().get().unsigned_abs();
    let _ = signal_process(raw, Signal::TERM);
    let (tx, rx) = std::sync::mpsc::channel();
    let observer = std::thread::spawn(move || loop {
        match waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        ) {
            Err(rustix::io::Errno::INTR) | Ok(None) => continue,
            _ => {
                let _ = tx.send(());
                return;
            }
        }
    });
    // timer: deadline — the limit of a worker's own cleanup after SIGTERM; not a contract value.
    if rx.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = signal_process(raw, Signal::KILL);
    }
    let _ = observer.join();
    let _ = waitpid(Some(pid), WaitOptions::empty());
}

/// The exit status of `child`, an unreaped worker of this test, once it is available by `deadline`; then the child is
/// reaped by its exact pid. The wait is the testkit's `reap_within`: it blocks on the child's exit event (a pidfd on Linux,
/// kqueue on macOS), never on a sleep. A worker is a std `Child`, not an `OwnedChild`, because `OwnedWorker` ends it with
/// SIGTERM first. When the deadline comes first, the report names `step` and the pid, and the child stays unreaped, so its
/// owner's drop still ends it (and the payload guard still ends the payload group).
fn status_within(
    child: &mut Child,
    step: &str,
    deadline: Deadline,
) -> Result<std::process::ExitStatus, String> {
    let id = child.id();
    match botster_test_process::reap_within(child, deadline) {
        Ok(Some(status)) => Ok(status),
        Ok(None) => Err(format!(
            "{step}: the child {id} did not end within {:?}",
            deadline.limit()
        )),
        Err(error) => Err(format!(
            "{step}: the child {id} cannot be observed or reaped: {error}"
        )),
    }
}

/// [`status_within`] with the cleanup bound (`Deadline::cleanup`), and a failure of the test at `step` when the child does
/// not end within it.
fn status_by_bound(child: &mut Child, step: &str) -> std::process::ExitStatus {
    status_within(child, step, Deadline::cleanup()).unwrap_or_else(|report| panic!("{report}"))
}

/// The host end of one control link, written with the link's own codec. It checks only what the worker sends.
struct Link {
    stream: UnixStream,
    decoder: FrameDecoder,
    /// Bytes read from the socket that the decoder has not taken yet: the frames after the first one of a read.
    pending: Vec<u8>,
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

    /// The next frame; the read times out at the socket (a marked deadline set at the accept). Every byte of a read is
    /// kept until the decoder takes it, so several frames in one read are all delivered, in order.
    fn frame(&mut self) -> (FrameType, Vec<u8>) {
        let mut buf = [0u8; 4096];
        loop {
            let took = self.decoder.push(&self.pending);
            self.pending.drain(..took);
            if let Some(frame) = self.decoder.next_frame().unwrap() {
                return (frame.kind, frame.payload);
            }
            // No complete frame: the decoder took every pending byte, and needs more.
            let n = self
                .stream
                .read(&mut buf)
                .expect("a frame before the deadline");
            assert!(n > 0, "the worker closed the link");
            self.pending.extend_from_slice(&buf[..n]);
        }
    }

    /// The next report that is not an `Output` observation: these checks are about the lifecycle, and the payload's output
    /// (Core 6.2, `Activity`) comes in reads whose count the kernel chooses.
    fn report(&mut self) -> WorkerMsg {
        loop {
            let (kind, payload) = self.frame();
            assert_eq!(kind, FrameType::WORKER_MSG);
            match WorkerMsg::decode(&payload).unwrap() {
                WorkerMsg::Observed {
                    observation: Observation::Output { .. },
                } => {}
                msg => return msg,
            }
        }
    }
}

/// A started worker, linked to the test's host end, with its payload launched.
struct Session {
    worker: OwnedWorker,
    link: Link,
}

impl Session {
    /// Starts a worker, proves the hello both ways (AD-6), and launches `sh -c script` (AD-7 step 4).
    fn launch(root: &Path, script: &str, stop_grace_ms: u64) -> Session {
        let payload_guard = PayloadGuard::new(root);
        let script = format!("{}{script}", payload_guard.prefix());
        let socket = root.join("c");
        let listener = UnixListener::bind(&socket).unwrap();
        let launch = WorkerLaunch {
            control: socket,
            instance: InstanceId("1-1".into()),
            host_epoch: 1,
            token: [5; 32],
            endpoint: root.join("e"),
            startup_ms: WorkerLaunch::millis(CoreLimits::default().startup),
        };
        let observer_guard = crate::DRIVER_OBSERVER.map(|_| process_guard::GroupGuard::new(root));
        let mut command = if let Some(observer) = crate::DRIVER_OBSERVER {
            use std::os::unix::process::CommandExt;
            let mut command = Command::new("/bin/sh");
            command.args([
                "-c",
                &format!("{}exec \"$@\"", observer_guard.as_ref().unwrap().prefix()),
                "observer",
            ]);
            command.arg(std::env::current_exe().unwrap());
            command.args(["--exact", observer, "--nocapture"]);
            command
                .env_clear()
                .env("BOTSTER_DRIVER_CONTROL", &launch.control)
                .process_group(0);
            command
        } else {
            let mut command = Command::new(worker_binary());
            command.args(launch.args()).env_clear().envs(launch.env());
            command
        };
        let worker = OwnedWorker {
            worker: command.spawn().unwrap(),
            payload_guard: Some(payload_guard),
            observer_guard,
        };
        let (stream, _) = listener.accept().unwrap();
        let (link, payload) = hello_and_launch(
            stream,
            &launch,
            vec!["/bin/sh".into(), "-c".into(), script],
            stop_grace_ms,
            None,
            CoreLimits::default(),
        );
        assert!(payload.pid > 0);
        Session { worker, link }
    }

    fn signal_worker(&self, signal: rustix::process::Signal) {
        botster_core_sys::signal::signal_process(self.worker.worker.id(), signal).unwrap();
    }

    /// LC-7: `Remove` gives the complete result, and then the worker ends with code 0.
    fn remove(mut self) {
        self.link.msg(&HostMsg::Remove);
        let report = self.link.report();
        assert!(
            matches!(report, WorkerMsg::RemoveResult { .. }),
            "{report:?}"
        );
        let status = status_by_bound(
            &mut self.worker.worker,
            "LC-7: the worker ends after RemoveResult",
        );
        assert_eq!(
            status.code(),
            Some(0),
            "LC-7: the worker ends after the result"
        );
    }
}

/// On the worker's accepted link: proves the hello both ways (AD-6), and launches `argv` (AD-7 step 4). With `route`, an
/// `AttachRoute` frame of route 1 follows the `Launch` frame in the same send, with the route's stream: the descriptor's
/// read holds the first byte of that one `AttachRoute` frame (DP-2, plan 23ae). Returns the link and the payload's
/// identity from `Launched`.
fn hello_and_launch(
    stream: UnixStream,
    launch: &WorkerLaunch,
    argv: Vec<String>,
    stop_grace_ms: u64,
    route: Option<std::os::fd::BorrowedFd<'_>>,
    limits: CoreLimits,
) -> (Link, PayloadId) {
    // timer: deadline — the limit of a wait for a real worker's frame; not a contract value.
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut link = Link {
        stream,
        decoder: FrameDecoder::new(1 << 20),
        pending: Vec::new(),
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
        proof: botster_core_link::proof::host_proof(
            &launch.token,
            &launch.instance,
            launch.host_epoch,
        ),
        host_epoch: 1,
    }
    .encode(&mut reply)
    .unwrap();
    link.send(FrameType::HELLO, &reply);
    let launch_msg = HostMsg::Launch(Box::new(LaunchSpec {
        argv,
        env: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        cwd: "/".into(),
        size: Size {
            rows: 24,
            cols: 80,
            cell_px: None,
        },
        color_profile: None,
        notification_policy: NotificationPolicy::All,
        size_policy: SizePolicy::Latest,
        link_frame_bound: 1 << 20,
        stop_grace_ms,
        limits: limits.clone(),
    }));
    match route {
        None => link.msg(&launch_msg),
        Some(route) => {
            // A worker that reads the hello and a descriptor in one read refuses the descriptor (plan 23ae). The answer
            // to a request comes only after the hello passed, so the descriptor's read comes after it (F118). Before the
            // launch, a signal request changes nothing: no process group is live.
            link.msg(&HostMsg::Op {
                req: u64::MAX,
                op: Op::Signal {
                    id: SessionId("s".into()),
                    sig: Signal::Term,
                },
            });
            let answer = link.report();
            assert!(
                matches!(
                    answer,
                    WorkerMsg::Done {
                        req: u64::MAX,
                        result: OpResult::Err(_)
                    }
                ),
                "{answer:?}"
            );
            let mut bytes = Vec::new();
            for msg in [launch_msg, attach_route(&limits)] {
                let mut payload = Vec::new();
                msg.encode(&mut payload);
                encode_frame(FrameType::HOST_MSG, &payload, u32::MAX, &mut bytes).unwrap();
            }
            let sent =
                botster_core_sys::fd_pass::send_with_fd(&link.stream, &bytes, route).unwrap();
            link.stream.write_all(&bytes[sent..]).unwrap();
            // The worker reports the bound descriptor (R-65) and the launch, in the order of its turns: both, once each.
            let (mut launched, mut received) = (None, false);
            while launched.is_none() || !received {
                match link.report() {
                    WorkerMsg::Launched { payload, .. } if launched.is_none() => {
                        launched = Some(payload);
                    }
                    WorkerMsg::RouteReceived { route: RouteId(1) } if !received => received = true,
                    other => panic!("not launched with route 1 received: {other:?}"),
                }
            }
            return (link, launched.unwrap());
        }
    }
    let WorkerMsg::Launched { payload, .. } = link.report() else {
        panic!("not launched");
    };
    (link, payload)
}

/// The `AttachRoute` of route 1, with the limits that the host applies for default options under `core` (`admit.rs`).
fn attach_route(core: &CoreLimits) -> HostMsg {
    HostMsg::AttachRoute {
        route: RouteId(1),
        options: AttachOptions {
            file_directory: "/tmp".into(),
            file_permissions: None,
            route_features: Vec::new(),
            terminal_formats: Vec::new(),
            owner: None,
            query_deadline: None,
            route_tag: None,
            route_limits: None,
            history: None,
            stall_deadline: None,
            answers_queries: false,
            input: true,
        },
        limits: AppliedRouteLimits {
            max_frame_bytes: core.effective_max_route_frame_bytes(),
            max_screen_frame_bytes: core.max_snapshot_bytes + 1,
            max_history_page_bytes: core.max_history_page_bytes,
            max_paste_bytes: core.max_paste_bytes,
            max_query_bytes: core.max_query_bytes,
            max_query_reply_bytes: core.max_query_reply_bytes,
            max_file_bytes: core.max_file_bytes,
            query_deadline: Duration::ZERO,
            stall_deadline: core.reader_progress_deadline,
            stall_close_after: core.stall_close_after,
            route_input_queue_bytes: core.route_input_queue_bytes,
        },
    }
}

/// Reads the first line that the payload writes to `fifo` (the open blocks until the payload opens it for writing).
fn first_line(fifo: &Path) -> (BufReader<std::fs::File>, String) {
    let mut reader = BufReader::new(std::fs::File::open(fifo).unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    (reader, line)
}

fn exited(code: Option<i32>, signal: Option<i32>) -> WorkerMsg {
    WorkerMsg::Exited { code, signal }
}

/// Core EV-4: a real payload that exits with a code is `Exited{code}` with no signal; one that dies by a signal is
/// `Exited{signal}` with no code.
#[test]
fn ev_4_a_real_exit_carries_the_code_or_the_signal() {
    let root = temp_root();
    let mut s = Session::launch(root.path(), "echo out; exit 3", 5000);
    assert_eq!(s.link.report(), exited(Some(3), None));
    s.remove();
    let root = temp_root();
    let mut s = Session::launch(root.path(), "kill -TERM $$", 5000);
    assert_eq!(s.link.report(), exited(None, Some(15)));
    s.remove();
}

/// Core LC-5: `Stop` is the graceful request to the payload group; a payload that ignores it is ended by `Kill`, the group
/// kill of `stop_grace`; the worker lives on after the payload (it still answers `Remove`, LC-7).
#[test]
fn lc_5_stop_asks_then_kill_ends_and_the_worker_survives() {
    let root = temp_root();
    let mut s = Session::launch(root.path(), "exec sleep 30", 5000);
    s.link.msg(&HostMsg::Stop);
    assert_eq!(s.link.report(), exited(None, Some(15)));
    s.remove();

    let root = temp_root();
    let ready = fifo(root.path(), "f");
    let script = format!(
        "trap '' TERM; /bin/echo up > {}; exec sleep 30",
        ready.display()
    );
    let mut s = Session::launch(root.path(), &script, 5000);
    // The payload ignores TERM once it has written to the FIFO, so the Stop cannot race the trap.
    assert_eq!(first_line(&ready).1, "up\n");
    s.link.msg(&HostMsg::Stop);
    s.link.msg(&HostMsg::Kill);
    assert_eq!(
        s.link.report(),
        exited(None, Some(9)),
        "TERM is ignored, so only the kill ends it"
    );
    s.remove();
}

/// Core LC-6, A2-1: `Signal` reaches the payload's whole group and is confirmed by `Done`. A background child in the group
/// holds the FIFO open, so the end of file of the FIFO proves that the child died too.
#[test]
fn lc_6_signal_reaches_the_whole_group() {
    let root = temp_root();
    let held = fifo(root.path(), "f");
    let script = format!(
        "exec 3> {}; sleep 30 & /bin/echo up >&3; wait",
        held.display()
    );
    let mut s = Session::launch(root.path(), &script, 5000);
    let (mut reader, line) = first_line(&held);
    assert_eq!(line, "up\n");
    s.link.msg(&HostMsg::Op {
        req: 1,
        op: Op::Signal {
            id: SessionId("s".into()),
            sig: Signal::Term,
        },
    });
    let mut reports = vec![s.link.report(), s.link.report()];
    reports.sort_by_key(|r| matches!(r, WorkerMsg::Exited { .. }));
    assert_eq!(
        reports,
        [
            WorkerMsg::Done {
                req: 1,
                result: OpResult::Ok(OpOutput::Unit)
            },
            exited(None, Some(15)),
        ]
    );
    let started = Instant::now();
    let mut rest = Vec::new();
    reader.read_to_end(&mut rest).unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the background child held the FIFO: it outlived the group signal"
    );
    s.remove();
}

/// Core LC-5 and the P1 interface (F7): `SIGUSR1` on the worker ends its payload without a host request: the graceful
/// request, then the group kill at `stop_grace`. The worker keeps running, reports the exit, and ends on `Remove` (LC-7).
#[test]
fn lc_5_the_worker_control_signal_ends_the_payload_and_the_worker_stays() {
    let root = temp_root();
    let ready = fifo(root.path(), "f");
    let script = format!(
        "trap '' TERM; /bin/echo up > {}; exec sleep 30",
        ready.display()
    );
    let mut s = Session::launch(root.path(), &script, 200);
    assert_eq!(first_line(&ready).1, "up\n");
    s.signal_worker(rustix::process::Signal::USR1);
    assert_eq!(
        s.link.report(),
        exited(None, Some(9)),
        "TERM is ignored, so the kill of the grace ends it"
    );
    s.remove();
}

/// DESIGN.md "Adoption (P5)" parts 1, 3 and 7; AD-6, DP-8: the real worker binds its endpoint before its first hello. A new
/// host connects there and proves the host role at a higher epoch. The worker answers on that link with its own proof at
/// that epoch and an `Adopted` report of its running payload, and it closes the old link (the fence). The new link obeys
/// the new host: `Remove` ends the worker, and the worker's end removes its endpoint.
#[test]
fn a_new_host_adopts_the_real_worker_at_its_endpoint() {
    let root = temp_root();
    // The payload waits on its PTY's input without a timer; the stop below ends it.
    let mut s = GuardedSession::launch(root.path(), "exec /bin/cat", 200);
    let endpoint = root.path().join("e");
    let stream = UnixStream::connect(&endpoint).expect("the worker listens at its endpoint");
    let mut adopter = adopt(stream, 2);
    // The fence: the old host's link ends.
    let mut rest = [0u8; 64];
    loop {
        match s.link.stream.read(&mut rest) {
            Ok(0) => break,
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => break,
            Ok(_) => {}
            Err(error) => panic!("the old link did not end: {error}"),
        }
    }
    // The new link obeys the new host: `Stop` ends the payload (LC-5), and `Remove` ends the worker (LC-7).
    adopter.msg(&HostMsg::Stop);
    let report = adopter.report();
    assert!(matches!(report, WorkerMsg::Exited { .. }), "{report:?}");
    s.link = adopter;
    s.remove();
    assert!(
        !endpoint.exists(),
        "the worker removes its endpoint at its end"
    );
}

/// AD-6: a new host's link at the worker's endpoint proves the host role at `epoch`. The worker answers on it with its own
/// proof at that epoch and an `Adopted` report of its running payload.
fn adopt(stream: UnixStream, epoch: u64) -> Link {
    let (instance, token) = (InstanceId("1-1".into()), [5; 32]);
    // timer: deadline — the limit of a wait for a real worker's frame; not a contract value.
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut adopter = Link {
        stream,
        decoder: FrameDecoder::new(1 << 20),
        pending: Vec::new(),
    };
    let mut hello = Vec::new();
    Hello {
        protocol: 1,
        instance: instance.clone(),
        proof: botster_core_link::proof::host_proof(&token, &instance, epoch),
        host_epoch: epoch,
    }
    .encode(&mut hello)
    .unwrap();
    adopter.send(FrameType::HELLO, &hello);
    let (kind, payload) = adopter.frame();
    assert_eq!(kind, FrameType::HELLO);
    let answer = Hello::decode(&payload).unwrap();
    assert_eq!(answer.host_epoch, epoch);
    assert_eq!(answer.proof, token_proof(&token, &instance, epoch), "AD-6");
    match adopter.report() {
        WorkerMsg::Adopted { report } => assert!(
            matches!(report.payload, AdoptedPayload::Running { .. }),
            "{report:?}"
        ),
        other => panic!("{other:?}"),
    }
    adopter
}

/// DESIGN.md "Adoption (P5)" part 3: a new host's hello can come after the worker's first read of the connection found
/// nothing. Then only the poll's readiness of that candidate brings the hello to the worker. The worker holds one candidate
/// at a time and closes a later one; it accepts one connection per turn and reads it in the same turn. So when the second
/// connection ends, the worker has read the first one and found nothing, and the test sends the first hello only then.
#[test]
fn a_new_host_whose_hello_comes_after_the_first_read_adopts_the_worker() {
    let root = temp_root();
    // The payload waits on its PTY's input without a timer; the stop below ends it.
    let mut s = GuardedSession::launch(root.path(), "exec /bin/cat", 200);
    let endpoint = root.path().join("e");
    let first = UnixStream::connect(&endpoint).expect("the worker listens at its endpoint");
    let second = UnixStream::connect(&endpoint).expect("the worker listens at its endpoint");
    assert_eq!(
        Bounded::new(second).to_eof(Deadline::cleanup()).unwrap(),
        b"",
        "the worker holds one candidate and closes the second"
    );
    let mut adopter = adopt(first, 2);
    // The new link obeys the new host: `Stop` ends the payload (LC-5), and `Remove` ends the worker (LC-7).
    adopter.msg(&HostMsg::Stop);
    let report = adopter.report();
    assert!(matches!(report, WorkerMsg::Exited { .. }), "{report:?}");
    s.link = adopter;
    s.remove();
}

/// Plan R12 (teardown is TERM, then KILL, then reap): `SIGTERM` on the worker kills the payload group that it holds at
/// once, reports the exit, and ends the worker, whatever the payload does with `SIGTERM`.
#[test]
fn sigterm_on_the_worker_ends_its_payload_group_then_the_worker() {
    let root = temp_root();
    let ready = fifo(root.path(), "f");
    let script = format!(
        "trap '' TERM; /bin/echo up > {}; exec sleep 30",
        ready.display()
    );
    let mut s = Session::launch(root.path(), &script, 5000);
    assert_eq!(first_line(&ready).1, "up\n");
    s.signal_worker(rustix::process::Signal::TERM);
    assert_eq!(s.link.report(), exited(None, Some(9)));
    let status = status_by_bound(&mut s.worker.worker, "R12: the worker ends after SIGTERM");
    assert_eq!(status.code(), Some(0));
}

/// The test's own wait for a worker: a worker that does not end (its payload runs) fails the wait at its deadline, with the
/// step and its pid. A plain `Child::wait` would block here, because the worker never ends on its own. The worker stays the
/// test's unreaped child: the link still works, and the bounded wait of `remove` then reaps it with code 0.
#[test]
fn a_worker_that_does_not_end_fails_the_bounded_wait_with_its_pid() {
    let root = temp_root();
    let hold = Blocker::new(root.path(), "hold").unwrap();
    let mut s = Session::launch(root.path(), &format!("exec {}", hold.shell()), 5000);
    let id = s.worker.worker.id();
    // timer: deadline — the proof's own short bound, so that the test does not wait the cleanup bound; not a contract value.
    let deadline = Deadline::after(Duration::from_millis(100));
    let report = status_within(&mut s.worker.worker, "the proof step", deadline).unwrap_err();
    assert_eq!(
        report,
        format!("the proof step: the child {id} did not end within 100ms")
    );
    s.link.msg(&HostMsg::Stop);
    assert_eq!(s.link.report(), exited(None, Some(15)));
    s.remove();
}

/// The test's own link helper (F8): two complete frames and the start of a third in ONE read are all delivered, in order,
/// and the third completes with the next read.
#[test]
fn the_link_helper_keeps_every_frame_of_one_read() {
    let (stream, mut peer) = UnixStream::pair().unwrap();
    // timer: deadline — the limit of a read that a test writes itself; not a contract value.
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut link = Link {
        stream,
        decoder: FrameDecoder::new(1 << 20),
        pending: Vec::new(),
    };
    let mut wire = Vec::new();
    for payload in [&b"one"[..], b"two", b"three"] {
        encode_frame(FrameType::WORKER_MSG, payload, u32::MAX, &mut wire).unwrap();
    }
    let split = wire.len() - 2;
    peer.write_all(&wire[..split]).unwrap();
    assert_eq!(link.frame().1, b"one");
    assert_eq!(link.frame().1, b"two");
    peer.write_all(&wire[split..]).unwrap();
    assert_eq!(link.frame().1, b"three");
}

/// BUILD.md rule 10: a panic still ends the payload after worker cleanup cannot run.
#[test]
fn a_panic_after_worker_sigkill_ends_the_payload_group() {
    let root = temp_root();
    let held = fifo(root.path(), "f");
    let script = format!(
        "trap '' HUP TERM; exec 3> {}; /bin/echo up >&3; exec sleep 30",
        held.display()
    );
    let mut session = Session::launch(root.path(), &script, 5000);
    let (mut reader, line) = first_line(&held);
    assert_eq!(line, "up\n");
    // The graceful group signal must not remove independent test ownership.
    session.link.msg(&HostMsg::Op {
        req: 1,
        op: Op::Signal {
            id: SessionId("s".into()),
            sig: Signal::Term,
        },
    });
    assert_eq!(
        session.link.report(),
        WorkerMsg::Done {
            req: 1,
            result: OpResult::Ok(OpOutput::Unit)
        }
    );
    // SIGKILL prevents every production cleanup action in the worker.
    session.signal_worker(rustix::process::Signal::KILL);
    status_by_bound(&mut session.worker.worker, "the worker ends after SIGKILL");
    let mut fds = [rustix::event::PollFd::new(
        reader.get_ref(),
        rustix::event::PollFlags::IN,
    )];
    assert_eq!(
        rustix::event::poll(
            &mut fds,
            Some(&rustix::event::Timespec {
                tv_sec: 0,
                tv_nsec: 0
            })
        )
        .unwrap(),
        0,
        "the payload still holds the FIFO before test cleanup"
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _session = session;
        panic!("the test failed after the worker died");
    }));
    assert!(result.is_err());
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut rest = Vec::new();
        let _ = sent.send(reader.read_to_end(&mut rest));
    });
    received
        // timer: deadline — the payload must release the FIFO after test cleanup.
        .recv_timeout(Duration::from_secs(10))
        .expect("the payload group ended")
        .unwrap();
}

/// DP-2, plan 23ae condition 2: a route descriptor that the worker receives is close-on-exec, so the payload holds no copy.
/// The worker holds the route's stream when it spawns the payload. The worker then dies by `SIGKILL`, and the client end of
/// the route reads the end of file: no process holds the worker's end. The payload ignores the hang-up and is still alive
/// after the end of file: it answers the release with its marker. The proof is by identity (the end of file), not by a
/// list of the payload's descriptors.
#[test]
fn a_received_route_descriptor_does_not_reach_the_payload() {
    use std::os::fd::AsFd;
    let root = temp_root();
    let mut ready = marker_fifo(root.path(), "ready");
    let mut alive = marker_fifo(root.path(), "alive");
    let mut release = Blocker::new(root.path(), "release").unwrap();
    let hold = Blocker::new(root.path(), "hold").unwrap();
    let script = format!(
        "trap '' HUP TERM; /bin/echo ready > {}; /usr/bin/head -c 1 {} > /dev/null; /bin/echo alive > {}; exec {}",
        quoted(&root.path().join("ready")),
        quoted(release.path()),
        quoted(&root.path().join("alive")),
        hold.shell(),
    );
    let (client, worker_end) = UnixStream::pair().unwrap();
    let mut s = GuardedSession::launch_with_route(
        root.path(),
        &script,
        5000,
        Some(worker_end.as_fd()),
        CoreLimits::default(),
    );
    drop(worker_end);
    assert_eq!(
        ready.line(Deadline::cleanup()).unwrap().as_deref(),
        Some("ready\n")
    );
    // SIGKILL: the worker runs no cleanup, and the kernel closes its descriptors.
    s.worker.kill().unwrap();
    s.worker.status_by(Deadline::cleanup());
    Bounded::new(client)
        .to_eof(Deadline::cleanup())
        .expect("the end of file: no process holds the worker's end of the route");
    // The test holds both ends of the FIFO, so the byte waits in it even when `head` has not opened it yet.
    release.send(b"x").unwrap();
    assert_eq!(
        alive.line(Deadline::cleanup()).unwrap().as_deref(),
        Some("alive\n"),
        "the payload lived through the end of file"
    );
    release.release();
}

/// The client end of a real route: it reads the worker's frames by a deadline and decodes them with the bounds of
/// `attach_route` (DP-3).
struct RouteClient {
    stream: UnixStream,
    reader: botster_route_codec::prelude::StreamReader,
    bounds: botster_route_codec::prelude::FrameBounds,
}

impl RouteClient {
    fn new(stream: UnixStream, core: &CoreLimits) -> RouteClient {
        let HostMsg::AttachRoute { limits, .. } = attach_route(core) else {
            unreachable!("attach_route is an AttachRoute")
        };
        // timer: deadline — the limit of a wait for a real worker's route frame; not a contract value.
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        RouteClient {
            stream,
            reader: botster_route_codec::prelude::StreamReader::new(),
            bounds: botster_core_link::route::frame_bounds(&limits),
        }
    }

    /// The worker's next frame on the route.
    fn frame(&mut self) -> botster_route_codec::prelude::ToClient {
        use botster_route_codec::prelude::{Decoded, ToClient};
        loop {
            if let Some(frame) = self.reader.next_frame(&self.bounds).unwrap() {
                match ToClient::decode(&frame, &self.bounds).unwrap() {
                    Decoded::Frame(frame) => return frame,
                    other => panic!("not a frame of this protocol: {other:?}"),
                }
            }
            let mut buf = [0u8; 4096];
            let n = self
                .stream
                .read(&mut buf)
                .expect("a route frame before the deadline");
            assert_ne!(n, 0, "the worker closed the route");
            self.reader.push(&buf[..n]);
        }
    }

    /// Reads frames up to `live`, the end of the attach (OU-9). The first one is `attached`.
    fn read_to_live(&mut self) {
        use botster_route_codec::prelude::{TYPE_ATTACHED, TYPE_LIVE};
        assert_eq!(self.frame().type_byte(), TYPE_ATTACHED);
        while self.frame().type_byte() != TYPE_LIVE {}
    }

    fn send(&mut self, frame: &botster_route_codec::prelude::ToWorker) {
        self.stream
            .write_all(&botster_route_codec::prelude::stream_wrap(&frame.encode()))
            .unwrap();
    }
}

/// DP-2, DP-5, OU-9 on the real driver (plan section 6.3 item 1): the route's stream carries `attached`, the baseline and
/// `live`; then the client's input reaches the payload, and the payload's output reaches the client.
#[test]
fn a_real_route_carries_the_attach_the_input_and_the_output() {
    use botster_route_codec::prelude::{Op, ToClient, ToWorker};
    use std::os::fd::AsFd;
    let root = temp_root();
    let hold = Blocker::new(root.path(), "hold").unwrap();
    let script = format!(
        "IFS= read -r line; /bin/echo \"got:$line\"; exec {}",
        hold.shell()
    );
    let (client, worker_end) = UnixStream::pair().unwrap();
    let _s = GuardedSession::launch_with_route(
        root.path(),
        &script,
        5000,
        Some(worker_end.as_fd()),
        CoreLimits::default(),
    );
    drop(worker_end);
    let mut client = RouteClient::new(client, &CoreLimits::default());
    client.read_to_live();
    client.send(&ToWorker::Text {
        op: Op(0),
        text: "hi\n".into(),
    });
    let mut output = Vec::new();
    while !output.windows(6).any(|w| w == b"got:hi") {
        if let ToClient::Output { payload } = client.frame() {
            output.extend_from_slice(&payload);
        }
    }
}

/// OU-3 on the real driver: output far larger than the socket's buffer, and within the route's queue, reaches a client
/// that reads only after the payload wrote it all: whole and in order. The client reads nothing until then, so the route's
/// last write waits on a full socket with no more output to come, and only `RouteWritable` sends the rest.
#[test]
fn a_real_route_delivers_large_output_whole_and_in_order() {
    use botster_route_codec::prelude::ToClient;
    use std::os::fd::AsFd;
    const BYTES: usize = 3_000_000;
    let root = temp_root();
    let mut done = marker_fifo(root.path(), "done");
    let hold = Blocker::new(root.path(), "hold").unwrap();
    let script = format!(
        "/usr/bin/head -c {BYTES} /dev/zero | /usr/bin/tr '\\0' a; /bin/echo end; /bin/echo done > {}; exec {}",
        quoted(&root.path().join("done")),
        hold.shell()
    );
    let (client, worker_end) = UnixStream::pair().unwrap();
    let _s = GuardedSession::launch_with_route(
        root.path(),
        &script,
        5000,
        Some(worker_end.as_fd()),
        CoreLimits::default(),
    );
    drop(worker_end);
    assert_eq!(
        done.line(Deadline::cleanup()).unwrap().as_deref(),
        Some("done\n")
    );
    let mut client = RouteClient::new(client, &CoreLimits::default());
    client.read_to_live();
    let mut output = Vec::new();
    // Only the new bytes, and the two before them, can complete the marker: the search stays linear in the output.
    let end = loop {
        let from = output.len().saturating_sub(2);
        if let ToClient::Output { payload } = client.frame() {
            output.extend_from_slice(&payload);
        }
        if let Some(at) = output[from..].windows(3).position(|w| w == b"end") {
            break from + at;
        }
    };
    let body: Vec<u8> = output[..end]
        .iter()
        .copied()
        .filter(|b| !b"\r\n".contains(b))
        .collect();
    assert_eq!(body.len(), BYTES, "every byte, once");
    assert!(body.iter().all(|&b| b == b'a'), "only the payload's bytes");
}

/// OU-3d on the real driver (F121): a route queue that fills stops the PTY reads until the client reads, and the reads then
/// go on. The route queue is 64 KiB (`route_queue_bytes`; R-45 exempts the baseline from it). The payload writes 3,000,000
/// bytes and then its `done` marker. The client reads nothing first: the queue, the socket's buffers and the PTY's buffer
/// hold far less than the output, so the payload cannot write it all, and no `done` comes by the cleanup bound. A driver
/// that ignores the budget reads the whole output into the queue, and `done` comes. Then the client reads: `done` comes,
/// and every byte reaches the client, once and in order.
#[test]
fn a_full_real_route_queue_stops_the_pty_reads_until_the_client_reads() {
    use botster_route_codec::prelude::ToClient;
    use botster_test_process::ReadError;
    use std::os::fd::AsFd;
    const BYTES: usize = 3_000_000;
    let root = temp_root();
    let mut done = marker_fifo(root.path(), "done");
    let hold = Blocker::new(root.path(), "hold").unwrap();
    let script = format!(
        "/usr/bin/head -c {BYTES} /dev/zero | /usr/bin/tr '\\0' a; /bin/echo end; /bin/echo done > {}; exec {}",
        quoted(&root.path().join("done")),
        hold.shell()
    );
    // 9B: the route queue is at least the snapshot bound, so both are 64 KiB (a 24x80 screen is far smaller).
    let limits = CoreLimits {
        route_queue_bytes: 64 * 1024,
        max_snapshot_bytes: 64 * 1024,
        ..CoreLimits::default()
    };
    limits.validate().expect("legal limits");
    let (client, worker_end) = UnixStream::pair().unwrap();
    let _s = GuardedSession::launch_with_route(
        root.path(),
        &script,
        5000,
        Some(worker_end.as_fd()),
        limits.clone(),
    );
    drop(worker_end);
    match done.line(Deadline::cleanup()) {
        Err(ReadError::Deadline { .. }) => {}
        other => {
            panic!("the payload wrote all its output to a client that read nothing: {other:?}")
        }
    }
    let mut client = RouteClient::new(client, &limits);
    client.read_to_live();
    let mut output = Vec::new();
    // Only the new bytes, and the two before them, can complete the marker: the search stays linear in the output.
    let end = loop {
        let from = output.len().saturating_sub(2);
        if let ToClient::Output { payload } = client.frame() {
            output.extend_from_slice(&payload);
        }
        if let Some(at) = output[from..].windows(3).position(|w| w == b"end") {
            break from + at;
        }
    };
    assert_eq!(
        done.line(Deadline::cleanup()).unwrap().as_deref(),
        Some("done\n")
    );
    let body: Vec<u8> = output[..end]
        .iter()
        .copied()
        .filter(|b| !b"\r\n".contains(b))
        .collect();
    assert_eq!(body.len(), BYTES, "every byte, once");
    assert!(body.iter().all(|&b| b == b'a'), "only the payload's bytes");
}

/// OU-5 on the real driver: the client's close of its end ends the route, and the host hears `RouteClosed{PeerClosed}`.
#[test]
fn a_client_close_ends_its_real_route() {
    use std::os::fd::AsFd;
    let root = temp_root();
    let hold = Blocker::new(root.path(), "hold").unwrap();
    let (client, worker_end) = UnixStream::pair().unwrap();
    let mut s = GuardedSession::launch_with_route(
        root.path(),
        &hold.shell(),
        5000,
        Some(worker_end.as_fd()),
        CoreLimits::default(),
    );
    drop(worker_end);
    let mut client = RouteClient::new(client, &CoreLimits::default());
    client.read_to_live();
    drop(client);
    loop {
        if let WorkerMsg::RouteClosed { route, reason, .. } = s.link.report() {
            assert_eq!((route, reason), (RouteId(1), RouteCloseReason::PeerClosed));
            break;
        }
    }
}

/// The driver observer ends when its test parent dies without running Drop.
#[test]
fn parent_death_ends_the_driver_observer() {
    if crate::DRIVER_OBSERVER.is_none() {
        return;
    }
    let root = temp_root();
    let name = format!(
        "{}::observer_parent",
        module_path!().split_once("::").unwrap().1
    );
    // The drop of an OwnedChild kills the parent and reaps it within the cleanup bound.
    let mut parent = OwnedChild::spawn(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env("BOTSTER_DRIVER_PARENT_ROOT", root.path())
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null()),
    )
    .unwrap();
    let mut reader = BufReader::new(parent.take_stderr().unwrap());
    let mut ready = String::new();
    reader.read_line(&mut ready).unwrap();
    assert!(
        ready.trim().parse::<u32>().is_ok(),
        "observer ready: {ready}"
    );
    parent.kill().unwrap();
    drop(parent);
    let (done, result) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut rest = Vec::new();
        let _ = done.send(reader.read_to_end(&mut rest));
    });
    result
        // timer: deadline — the observer must close the inherited pipe after parent death.
        .recv_timeout(Duration::from_secs(10))
        .expect("the observer ended after its test parent died")
        .unwrap();
}

#[test]
fn observer_parent() {
    let Some(root) = std::env::var_os("BOTSTER_DRIVER_PARENT_ROOT") else {
        return;
    };
    let session = Session::launch(Path::new(&root), "exec sleep 30", 5000);
    eprintln!("{}", session.worker.worker.id());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    drop(session);
}

/// A started worker whose worker and payload both start through the group guard of `botster-test-process`. Each one's
/// anchor ends its group on every path: a panic, and the death of the test process too. The worker keeps running when its
/// link closes (DP-8), so only its anchor ends it when the test dies. The test owns the worker as an [`OwnedChild`] in group
/// mode: the worker leads its own group, which its anchor holds. The control link is the same as [`Session`]'s.
struct GuardedSession {
    link: Link,
    worker: OwnedChild,
    /// The payload's identity from `Launched`. The worker starts the payload in its own session, so this pid is also the
    /// payload's group.
    payload: PayloadId,
    guard: Guard,
}

impl GuardedSession {
    /// Starts a worker and launches `sh -c script`, each through the guard's wrapper. Before this returns, one anchor
    /// reports the worker as its group's leader, and one the payload.
    fn launch(root: &Path, script: &str, stop_grace_ms: u64) -> GuardedSession {
        GuardedSession::launch_with_route(root, script, stop_grace_ms, None, CoreLimits::default())
    }

    /// As [`GuardedSession::launch`]; with `route`, the `Launch` frame and an `AttachRoute` frame go in one send with the
    /// route's stream (DP-2), so the worker holds the descriptor when it spawns the payload. `limits` are the `Launch`
    /// frame's.
    fn launch_with_route(
        root: &Path,
        script: &str,
        stop_grace_ms: u64,
        route: Option<std::os::fd::BorrowedFd<'_>>,
        limits: CoreLimits,
    ) -> GuardedSession {
        let mut guard = Guard::new(root)
            .unwrap()
            .grace(Duration::from_millis(stop_grace_ms));
        let wrapper = root.join("payload");
        guard
            .wrapper(&wrapper, Path::new("/bin/sh"), &["-c", script])
            .unwrap();
        let socket = root.join("c");
        let listener = UnixListener::bind(&socket).unwrap();
        let launch = WorkerLaunch {
            control: socket,
            instance: InstanceId("1-1".into()),
            host_epoch: 1,
            token: [5; 32],
            endpoint: root.join("e"),
            startup_ms: WorkerLaunch::millis(CoreLimits::default().startup),
        };
        let (program, args) = if let Some(observer) = crate::DRIVER_OBSERVER {
            (
                std::env::current_exe().unwrap(),
                vec!["--exact".into(), observer.into(), "--nocapture".into()],
            )
        } else {
            (worker_binary(), launch.args())
        };
        let args: Vec<&str> = args.iter().map(|arg| arg.to_str().unwrap()).collect();
        let worker_wrapper = root.join("worker");
        guard.wrapper(&worker_wrapper, &program, &args).unwrap();
        let mut command = Command::new(&worker_wrapper);
        command.env_clear();
        if crate::DRIVER_OBSERVER.is_some() {
            command.env("BOTSTER_DRIVER_CONTROL", &launch.control);
        } else {
            command.envs(launch.env());
        }
        let worker = OwnedChild::spawn_group(&mut command).unwrap();
        let reports = guard.anchors(1, Deadline::cleanup()).unwrap();
        assert_eq!(
            reports[0].leader.pid,
            worker.id(),
            "an anchor holds the worker's group"
        );
        // botster-test-process has no bounded accept yet (P6 adds `botster_test_process::accept` in its next crate PR, which
        // replaces this loop). The listener is non-blocking, so the accept never blocks; a poll by the deadline waits for the
        // connection, and an accept that finds none (`WouldBlock`, a peer that reset first) polls again.
        listener.set_nonblocking(true).unwrap();
        let deadline = Deadline::cleanup();
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("the worker's connection cannot be accepted: {error}"),
            }
            assert!(
                !deadline.expired(),
                "the worker connects within {:?}",
                deadline.limit()
            );
            let mut ready = [rustix::event::PollFd::new(
                &listener,
                rustix::event::PollFlags::IN,
            )];
            let left = rustix::event::Timespec::try_from(deadline.remaining()).unwrap();
            // timer: deadline — bounds the wait for the worker's connection.
            match rustix::event::poll(&mut ready, Some(&left)) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => panic!("the listener cannot be polled: {error}"),
            }
        };
        // macOS: an accepted socket inherits the listener's non-blocking mode. The link reads block, with a read timeout.
        stream.set_nonblocking(false).unwrap();
        let (link, payload) = hello_and_launch(
            stream,
            &launch,
            vec![wrapper.display().to_string()],
            stop_grace_ms,
            route,
            limits,
        );
        let reports = guard.anchors(2, Deadline::cleanup()).unwrap();
        assert_eq!(
            reports[1].leader.pid, payload.pid,
            "an anchor holds the payload's group"
        );
        GuardedSession {
            link,
            worker,
            payload,
            guard,
        }
    }

    /// LC-7: `Remove` gives the complete result, and then the worker ends with code 0.
    fn remove(mut self) {
        self.link.msg(&HostMsg::Remove);
        let report = self.link.report();
        assert!(
            matches!(report, WorkerMsg::RemoveResult { .. }),
            "{report:?}"
        );
        let status = self.worker.status_by(Deadline::cleanup());
        assert_eq!(
            status.code(),
            Some(0),
            "LC-7: the worker ends after the result"
        );
    }
}

impl Drop for GuardedSession {
    /// The anchors start to end the worker's and the payload's groups first. Then the fields drop in order: the worker,
    /// which holds the PTY master, ends and is reaped; and the guard reads each anchor's outcome (`Guard::release`).
    fn drop(&mut self) {
        self.guard.release();
    }
}

/// A FIFO in `root` that the payload writes and the test reads, by deadlines. The test holds it open for reading and
/// writing, so the payload's open for writing does not wait, and the test never sees an end of file. Close-on-exec: the
/// worker and its payload do not inherit it.
fn marker_fifo(root: &Path, name: &str) -> Bounded<std::fs::File> {
    let path = root.join(name);
    let made = OwnedChild::spawn(Command::new("/usr/bin/mkfifo").arg(&path))
        .unwrap()
        .status();
    assert!(made.success());
    Bounded::new(std::fs::File::from(
        rustix::fs::open(
            &path,
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    ))
}

/// Core AM-2, IN-2, IN-6: a real PTY keeps exact counts through cancellation and resumes the next transaction.
///
/// The payload starts through the shared group guard ([`GuardedSession`]). It blocks on fixture children of
/// `botster-test-process` ([`Blocker`]): one `cat` that waits for the count to read, and one that holds the payload until
/// the test kills it. The test reads the payload's markers by deadlines ([`Bounded`]).
#[test]
fn in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write() {
    let root = temp_root();
    let mut ready = marker_fifo(root.path(), "ready");
    let mut done = marker_fifo(root.path(), "done");
    let mut release = Blocker::new(root.path(), "release").unwrap();
    let hold = Blocker::new(root.path(), "hold").unwrap();
    let received = root.path().join("received");
    let script = format!(
        "stty raw -echo; /bin/echo ready > {}; count=$({}); /usr/bin/head -c \"$count\" > {}; /bin/echo done > {}; exec {}",
        quoted(&root.path().join("ready")),
        release.shell(),
        quoted(&received),
        quoted(&root.path().join("done")),
        hold.shell(),
    );
    let mut s = GuardedSession::launch(root.path(), &script, 5000);
    assert_eq!(
        ready.line(Deadline::cleanup()).unwrap().as_deref(),
        Some("ready\n")
    );
    let text = "a".repeat(196_608);
    let write = |req, text: String| HostMsg::Op {
        req,
        op: Op::WriteInput {
            session: SessionId("s".into()),
            payload: InputPayload::Text { text },
            guard: None,
        },
    };
    s.link.msg(&write(1, text.clone()));
    assert!(matches!(s.link.report(), WorkerMsg::Observed { .. }));
    s.link.msg(&HostMsg::Cancel { req: 1 });
    let WorkerMsg::Done {
        req: 1,
        result: OpResult::Ok(OpOutput::Input(cancelled)),
    } = s.link.report()
    else {
        panic!("the active write must report its cancellation");
    };
    assert_eq!(cancelled.outcome, WriteOutcome::Cancelled);
    assert_eq!(cancelled.payload_bytes_written, cancelled.pty_bytes_written);
    let count = usize::try_from(cancelled.payload_bytes_written).unwrap();
    assert!(
        count > 0 && count < text.len(),
        "the real PTY must take a prefix"
    );
    s.link.msg(&write(2, "b".into()));
    assert!(matches!(s.link.report(), WorkerMsg::Observed { .. }));
    release.send(format!("{}\n", count + 1).as_bytes()).unwrap();
    release.release();
    let WorkerMsg::Done {
        req: 2,
        result: OpResult::Ok(OpOutput::Input(written)),
    } = s.link.report()
    else {
        panic!("the next transaction must finish after the program reads");
    };
    assert_eq!(written.outcome, WriteOutcome::Written);
    assert_eq!(written.payload_bytes_written, 1);
    assert_eq!(written.pty_bytes_written, 1);
    assert_eq!(
        done.line(Deadline::cleanup()).unwrap().as_deref(),
        Some("done\n")
    );
    let mut expected = text.as_bytes()[..count].to_vec();
    expected.push(b'b');
    assert_eq!(std::fs::read(&received).unwrap(), expected);
    // The payload still runs (the `cat` of `hold`): it ends first, so `remove` sees the complete result (LC-7).
    s.link.msg(&HostMsg::Kill);
    assert_eq!(s.link.report(), exited(None, Some(9)));
    s.remove();
}

/// BUILD.md rule 10: a [`GuardedSession`] ends when its test process dies without running Drop. A fixture parent (this test
/// binary, running [`guarded_parent`]) starts the session and is then killed. The anchors end the worker's group (the worker
/// does not end when its link closes, DP-8) and the payload's group, and the test observes both ends without a signal.
#[test]
fn a_guarded_session_ends_when_its_test_parent_dies() {
    let root = temp_root();
    let name = format!(
        "{}::guarded_parent",
        module_path!().split_once("::").unwrap().1
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &name, "--nocapture"])
        .env("BOTSTER_GUARDED_PARENT_ROOT", root.path())
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null());
    let mut parent = OwnedChild::spawn(&mut command).unwrap();
    let _held = parent.take_stdin();
    let (_rest, line) = botster_test_process::first_line(parent.take_stderr().unwrap());
    let groups: Vec<u32> = line
        .split_whitespace()
        .map(|group| group.parse().unwrap())
        .collect();
    assert_eq!(groups.len(), 2, "the parent reports two groups: {line}");
    parent.kill().unwrap();
    assert!(!parent.status().success(), "the parent was killed");
    for group in groups {
        botster_test_process::rounds::await_group_end(
            botster_test_process::platform::pid(group).unwrap(),
            Deadline::cleanup(),
        )
        .unwrap();
    }
}

/// The fixture parent of [`a_guarded_session_ends_when_its_test_parent_dies`]: it starts a guarded session, reports the
/// worker's group and the payload's group, and waits for its death (bounded: it ends by itself at the cleanup bound).
#[test]
fn guarded_parent() {
    let Some(root) = std::env::var_os("BOTSTER_GUARDED_PARENT_ROOT") else {
        return;
    };
    let session = GuardedSession::launch(Path::new(&root), "exec /bin/cat", 5000);
    eprintln!("{} {}", session.worker.id(), session.payload.pid);
    let _ = Bounded::new(std::io::stdin()).line(Deadline::cleanup());
    drop(session);
}
