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
use botster_core_link::msg::{HostMsg, LaunchSpec, WorkerMsg};
use botster_core_link::proof::token_proof;
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

    fn report(&mut self) -> WorkerMsg {
        let (kind, payload) = self.frame();
        assert_eq!(kind, FrameType::WORKER_MSG);
        WorkerMsg::decode(&payload).unwrap()
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
            argv: vec!["/bin/sh".into(), "-c".into(), script],
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
        })));
        let WorkerMsg::Launched { payload, .. } = link.report() else {
            panic!("not launched");
        };
        assert!(payload.pid > 0);
        Session { worker, link }
    }

    fn signal_worker(&self, signal: rustix::process::Signal) {
        botster_core_sys::signal::signal_process(self.worker.worker.id(), signal).unwrap();
    }

    /// LC-7: `Remove` gives the complete result, and then the worker ends with code 0.
    fn remove(mut self) {
        self.link.msg(&HostMsg::Remove);
        assert!(matches!(self.link.report(), WorkerMsg::RemoveResult { .. }));
        let status = self.worker.worker.wait().unwrap();
        assert_eq!(
            status.code(),
            Some(0),
            "LC-7: the worker ends after the result"
        );
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
