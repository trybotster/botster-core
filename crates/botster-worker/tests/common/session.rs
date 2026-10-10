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
        let status = self.worker.worker.wait().unwrap();
        assert_eq!(
            status.code(),
            Some(0),
            "LC-7: the worker ends after the result"
        );
    }
}

/// On the worker's accepted link: proves the hello both ways (AD-6), and launches `argv` (AD-7 step 4). Returns the link and
/// the payload's identity from `Launched`.
fn hello_and_launch(
    stream: UnixStream,
    launch: &WorkerLaunch,
    argv: Vec<String>,
    stop_grace_ms: u64,
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
    link.msg(&HostMsg::Launch(Box::new(LaunchSpec {
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
        limits: CoreLimits::default(),
    })));
    let WorkerMsg::Launched { payload, .. } = link.report() else {
        panic!("not launched");
    };
    (link, payload)
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
    let ready = fifo(root.path(), "f");
    let script = format!("/bin/echo up > {}; exec sleep 30", ready.display());
    let mut s = Session::launch(root.path(), &script, 200);
    assert_eq!(first_line(&ready).1, "up\n");
    let endpoint = root.path().join("e");
    let (instance, token, epoch) = (InstanceId("1-1".into()), [5; 32], 2);
    let stream = UnixStream::connect(&endpoint).expect("the worker listens at its endpoint");
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
    s.link = adopter;
    s.remove();
    assert!(
        !endpoint.exists(),
        "the worker removes its endpoint at its end"
    );
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
    let status = s.worker.worker.wait().unwrap();
    assert_eq!(status.code(), Some(0));
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
    session.worker.worker.wait().unwrap();
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
    let mut parent = ObserverParent(Some(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env("BOTSTER_DRIVER_PARENT_ROOT", root.path())
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    ));
    let child = parent.0.as_mut().unwrap();
    let mut reader = BufReader::new(child.stderr.take().unwrap());
    let mut ready = String::new();
    reader.read_line(&mut ready).unwrap();
    assert!(
        ready.trim().parse::<u32>().is_ok(),
        "observer ready: {ready}"
    );
    child.kill().unwrap();
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

struct ObserverParent(Option<Child>);

impl Drop for ObserverParent {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
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
