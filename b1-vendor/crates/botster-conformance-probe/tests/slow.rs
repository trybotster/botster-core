//! Slow tier: the probe binary runs a script (design 9; Core 11 "scripted output and exits"). It proves that the binary that real-Core
//! runs start with prints what the script says and exits as it says, so the same script drives FakeCore and real Core.
#![cfg(feature = "slow")]

use botster_probe_script::control::{
    Report, Request, CONNECT_FAILED_EXIT, CONTROL_ENV, CONTROL_FAILED_EXIT,
};
use botster_route_codec::prelude::hex_decode;
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::poll::{poll, PollFd, PollFlags};
use nix::pty::openpty;
use nix::sys::signal::{kill, killpg, Signal};
use nix::sys::termios::{tcgetattr, LocalFlags, SpecialCharacterIndices};
use nix::unistd::ttyname;
use nix::unistd::Pid;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

/// The group leader. Its Drop kills the group and reaps the leader, so a panic anywhere after its spawn cleans up.
struct Sentinel(Child);

impl Drop for Sentinel {
    fn drop(&mut self) {
        let _ = killpg(Pid::from_raw(self.0.id() as i32), Signal::SIGKILL);
        let _ = self.0.wait();
    }
}

/// A probe in its own process group with piped stdin and stdout. A sentinel (a probe that holds, with no pipes) leads the group and is
/// reaped last, so the group id stays ours while the probe is alive, after it is reaped, and while the watchdog runs (BUILD.md rule 10).
struct Probe {
    child: Child,
    /// Dropped after Probe::drop has run (field drop follows it), so the group id is valid until the very end.
    _sentinel: Sentinel,
    pgid: Pid,
    /// Stopped and joined in Drop before any group signal of Drop and before the sentinel is reaped.
    watchdog: Option<(Sender<()>, JoinHandle<()>)>,
}

impl Probe {
    fn start(script: &str) -> Probe {
        Probe::spawn(script, Stdio::piped(), Stdio::piped())
    }

    /// A probe whose stdin and stdout are given (a terminal, for example). The group, the sentinel and the watchdog are as for `start`.
    fn spawn(script: &str, stdin: Stdio, stdout: Stdio) -> Probe {
        Probe::spawn_env(script, stdin, stdout, &[])
    }

    /// As `spawn`, with these variables added to the probe's environment.
    fn spawn_env(script: &str, stdin: Stdio, stdout: Stdio, env: &[(&str, &OsStr)]) -> Probe {
        let exe = env!("CARGO_BIN_EXE_botster-conformance-probe");
        let sentinel = Sentinel(
            Command::new(exe)
                .arg(r#"{"program":[{"hold":{}}]}"#)
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let pgid = Pid::from_raw(sentinel.0.id() as i32);
        let child = Command::new(exe)
            .arg(script)
            .envs(env.iter().copied())
            .process_group(pgid.as_raw())
            .stdin(stdin)
            .stdout(stdout)
            .spawn()
            .unwrap();
        // From here Probe::drop and the Sentinel drop own every cleanup, so a panic in the thread spawn still reaps both.
        let mut probe = Probe {
            child,
            _sentinel: sentinel,
            pgid,
            watchdog: None,
        };
        // A probe that hangs (a broken step, or a mutant) must fail the test, not stall it: the watchdog kills the group, the pipe
        // reaches EOF and the read in the test fails.
        let (stop_watchdog, stopped) = channel::<()>();
        let watchdog = std::thread::spawn(move || {
            // timer: deadline — a hung probe fails the test instead of stalling it
            if stopped.recv_timeout(Duration::from_secs(5)).is_err() {
                let _ = killpg(pgid, Signal::SIGKILL);
            }
        });
        probe.watchdog = Some((stop_watchdog, watchdog));
        probe
    }

    /// The pid of the probe itself (the group id is the sentinel's).
    fn pid(&self) -> Pid {
        Pid::from_raw(self.child.id() as i32)
    }

    fn read_exactly(&mut self, n: usize) -> Vec<u8> {
        let mut buf = vec![0u8; n];
        self.child
            .stdout
            .as_mut()
            .unwrap()
            .read_exact(&mut buf)
            .unwrap();
        buf
    }

    fn send(&mut self, bytes: &[u8]) {
        let stdin = self.child.stdin.as_mut().unwrap();
        stdin.write_all(bytes).and_then(|_| stdin.flush()).unwrap();
    }

    fn close_stdin(&mut self) {
        self.child.stdin = None;
    }

    fn rest_of_stdout(&mut self) -> Vec<u8> {
        let mut rest = vec![];
        self.child
            .stdout
            .as_mut()
            .unwrap()
            .read_to_end(&mut rest)
            .unwrap();
        rest
    }

    fn wait(&mut self) -> ExitStatus {
        self.child.wait().unwrap()
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        if let Some((stop, thread)) = self.watchdog.take() {
            let _ = stop.send(());
            let _ = thread.join();
        }
        let _ = killpg(self.pgid, Signal::SIGKILL);
        let _ = self.child.wait();
        // Every probe in the group holds the stdout pipe, so EOF is the event that says no descendant lives on.
        if let Some(out) = self.child.stdout.as_mut() {
            let _ = out.read_to_end(&mut Vec::new());
        }
        // The sentinel keeps the group id valid; its own drop reaps it after this function returns.
    }
}

// Each test owns its process group and reaps it on drop (BUILD.md rules 5 and 10). The tests prove that a step does not end the
// program early, so one script behaves the same on the probe and on the testkit (design 6.2, Core A5-1 and A5-4).

#[test]
fn the_probe_prints_scripted_bytes_and_exits_with_the_scripted_code() {
    // exit ends the program: the print after it does not run.
    let mut p = Probe::start(
        r#"{"program":[{"print":{"bytes_hex":"68690a"}},{"exit":{"code":7}},{"print":{"bytes_hex":"78"}}]}"#,
    );
    assert_eq!(p.rest_of_stdout(), b"hi\n");
    assert_eq!(p.wait().code(), Some(7));
}

#[test]
fn the_probe_dies_by_the_scripted_signal() {
    let mut p = Probe::start(r#"{"program":[{"signal_self":{"n":9}}]}"#);
    assert_eq!(p.rest_of_stdout(), b"");
    assert_eq!(p.wait().signal(), Some(9));
}

#[test]
fn hold_runs_until_a_signal_ends_it_and_no_later_step_runs() {
    let mut p = Probe::start(
        r#"{"program":[{"print":{"bytes_hex":"72"}},{"hold":{}},{"print":{"bytes_hex":"78"}}]}"#,
    );
    assert_eq!(p.read_exactly(1), b"r");
    kill(p.pid(), Signal::SIGTERM).unwrap();
    assert_eq!(p.rest_of_stdout(), b"");
    assert_eq!(p.wait().signal(), Some(15));
}

#[test]
fn fork_child_stays_in_the_process_group_and_the_parent_continues() {
    // The child prints "c" and holds. The parent prints "p" and exits 7. The child still holds the stdout pipe, so EOF comes only
    // when the group signal ends the child: that proves the child is in the parent's group.
    let mut p = Probe::start(
        r#"{"program":[{"fork_child":{"script":[{"print":{"bytes_hex":"63"}},{"hold":{}}]}},{"print":{"bytes_hex":"70"}},{"exit":{"code":7}}]}"#,
    );
    let mut got = p.read_exactly(2);
    got.sort_unstable();
    assert_eq!(got, b"cp");
    assert_eq!(p.wait().code(), Some(7));
    killpg(p.pgid, Signal::SIGTERM).unwrap();
    assert_eq!(p.rest_of_stdout(), b"");
}

#[test]
fn print_after_input_continues_with_the_next_step() {
    // 6f6b is "ok", 6e65 is "ne"; the script prints "ok", then "ne", then exits 7.
    let mut p = Probe::start(
        r#"{"program":[{"print_after_input":{"match_hex":"67","bytes_hex":"6f6b"}},{"print":{"bytes_hex":"6e65"}},{"exit":{"code":7}}]}"#,
    );
    p.send(b"g");
    assert_eq!(p.rest_of_stdout(), b"okne");
    assert_eq!(p.wait().code(), Some(7));
}

#[test]
fn print_after_input_does_not_match_consumed_bytes_again() {
    // Both steps match "ab". One "ab" arrives, then stdin closes: the second step must wait for new input, so it finds the end of
    // input and the probe exits 2 after the first output only.
    let mut p = Probe::start(
        r#"{"program":[{"print_after_input":{"match_hex":"6162","bytes_hex":"31"}},{"print_after_input":{"match_hex":"6162","bytes_hex":"32"}},{"exit":{"code":7}}]}"#,
    );
    p.send(b"ab");
    p.close_stdin();
    assert_eq!(p.rest_of_stdout(), b"1");
    assert_eq!(p.wait().code(), Some(2));
}

#[test]
fn print_after_input_keeps_input_that_follows_the_match() {
    // "abcd" arrives in one write; the first step takes "ab", and the second matches the "cd" that stayed.
    let mut p = Probe::start(
        r#"{"program":[{"print_after_input":{"match_hex":"6162","bytes_hex":"31"}},{"print_after_input":{"match_hex":"6364","bytes_hex":"32"}},{"exit":{"code":7}}]}"#,
    );
    p.send(b"abcd");
    assert_eq!(p.rest_of_stdout(), b"12");
    assert_eq!(p.wait().code(), Some(7));
}

#[test]
fn ignore_sigterm_continues_and_survives_sigterm() {
    // "r" is printed after ignore_sigterm, so it shows the step ran and the next step started. The probe then waits for input in
    // print_after_input. SIGTERM arrives during that read and must neither end the probe nor make the read fail (exit 2).
    let mut p = Probe::start(
        r#"{"program":[{"ignore_sigterm":{}},{"print":{"bytes_hex":"72"}},{"print_after_input":{"match_hex":"67","bytes_hex":"64"}},{"exit":{"code":5}}]}"#,
    );
    assert_eq!(p.read_exactly(1), b"r");
    kill(p.pid(), Signal::SIGTERM).unwrap();
    p.send(b"g");
    assert_eq!(p.read_exactly(1), b"d");
    assert_eq!(p.wait().code(), Some(5));
}

#[test]
fn without_ignore_sigterm_sigterm_ends_the_probe() {
    // The control for the test above: without the step, SIGTERM ends a waiting probe.
    let mut p = Probe::start(
        r#"{"program":[{"print":{"bytes_hex":"72"}},{"print_after_input":{"match_hex":"67","bytes_hex":"64"}}]}"#,
    );
    assert_eq!(p.read_exactly(1), b"r");
    kill(p.pid(), Signal::SIGTERM).unwrap();
    assert_eq!(p.wait().signal(), Some(15));
}

#[test]
fn fork_child_continues_with_the_next_step() {
    // The child prints "c" and the parent prints "p" and exits 7. The two writers share one pipe, so the order is open.
    let mut p = Probe::start(
        r#"{"program":[{"fork_child":{"script":[{"print":{"bytes_hex":"63"}}]}},{"print":{"bytes_hex":"70"}},{"exit":{"code":7}}]}"#,
    );
    let mut out = p.rest_of_stdout();
    out.sort_unstable();
    assert_eq!(out, b"cp");
    assert_eq!(p.wait().code(), Some(7));
}

#[test]
fn signal_self_ends_the_program_and_no_later_step_runs() {
    // A step after signal_self never runs: SIGTERM ends the probe, so the later print does not appear.
    let mut p =
        Probe::start(r#"{"program":[{"signal_self":{"n":15}},{"print":{"bytes_hex":"78"}}]}"#);
    assert_eq!(p.rest_of_stdout(), b"");
    assert_eq!(p.wait().signal(), Some(15));
}

#[test]
fn print_after_input_with_an_empty_match_prints_at_once_and_consumes_nothing() {
    // The empty match needs no input. The second step then matches "ab", which arrives later and was not consumed by the first.
    let mut p = Probe::start(
        r#"{"program":[{"print_after_input":{"match_hex":"","bytes_hex":"31"}},{"print_after_input":{"match_hex":"6162","bytes_hex":"32"}},{"exit":{"code":7}}]}"#,
    );
    assert_eq!(p.read_exactly(1), b"1");
    p.send(b"ab");
    assert_eq!(p.rest_of_stdout(), b"2");
    assert_eq!(p.wait().code(), Some(7));
}

/// Reads from a terminal's controller until `n` bytes have come, or fails when the terminal ends first (the watchdog ended the probe).
fn read_terminal(controller: &mut File, n: usize) -> Vec<u8> {
    let mut got = vec![];
    let mut buf = [0u8; 64];
    while got.len() < n {
        match controller.read(&mut buf) {
            Ok(0) | Err(_) => panic!("the terminal ended after {got:?}"),
            Ok(read) => got.extend_from_slice(&buf[..read]),
        }
    }
    got
}

#[test]
fn on_a_terminal_print_after_input_takes_a_byte_with_no_newline_and_nothing_is_echoed() {
    // R-48: a session's PTY starts in canonical mode with echo. The probe sets non-canonical mode with no echo before its first step,
    // so "1" with no newline reaches print_after_input, and the controller reads only the bell (07), not an echo of "1". 72 is "r".
    let pty = openpty(None, None).unwrap();
    let before = tcgetattr(&pty.slave).unwrap();
    let path = ttyname(&pty.slave).unwrap();
    let _probe = Probe::spawn(
        r#"{"program":[{"print":{"bytes_hex":"72"}},{"print_after_input":{"match_hex":"31","bytes_hex":"07"}},{"hold":{}}]}"#,
        Stdio::from(pty.slave.try_clone().unwrap()),
        Stdio::from(pty.slave),
    );
    // Only the probe holds the terminal now, so its end ends the controller's reads.
    let mut controller = File::from(pty.master);
    assert_eq!(read_terminal(&mut controller, 1), b"r");
    // The first step ran, so the modes are set: only the five echo and line flags, VMIN and VTIME change.
    let after = tcgetattr(OpenOptions::new().read(true).open(&path).unwrap()).unwrap();
    let cleared = LocalFlags::ICANON
        | LocalFlags::ECHO
        | LocalFlags::ECHOE
        | LocalFlags::ECHOK
        | LocalFlags::ECHONL;
    assert_eq!(after.local_flags, before.local_flags - cleared);
    assert_eq!(after.input_flags, before.input_flags);
    assert_eq!(after.output_flags, before.output_flags);
    assert_eq!(after.control_flags, before.control_flags);
    assert_eq!(
        after.control_chars[SpecialCharacterIndices::VMIN as usize],
        1
    );
    assert_eq!(
        after.control_chars[SpecialCharacterIndices::VTIME as usize],
        0
    );
    controller.write_all(b"1").unwrap();
    assert_eq!(read_terminal(&mut controller, 1), b"\x07");
    // The probe holds; the drop of `_probe` ends its group.
}

// Control mode (`botster_probe_script::control`, steward approval of 2026-10-10): with `BOTSTER_PROBE_CONTROL` set, the probe
// connects to the harness's socket and serves the program controls. Each test plays the harness on a real terminal (`openpty`).

/// A probe that holds and never reads stdin itself.
const HOLD: &str = r#"{"program":[{"hold":{}}]}"#;
/// "r" (72), then hold: reading "r" from the controller shows that the first step ran.
const PRINT_R_HOLD: &str = r#"{"program":[{"print":{"bytes_hex":"72"}},{"hold":{}}]}"#;

/// A fresh directory for a control socket, short enough for a Unix socket path, removed on drop.
struct SocketDir(PathBuf);

impl SocketDir {
    fn new() -> SocketDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "bpc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        SocketDir(dir)
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The harness's end: a listener on a socket in its own directory.
struct Listener {
    listener: UnixListener,
    path: PathBuf,
    _dir: SocketDir,
}

impl Listener {
    fn new() -> Listener {
        let dir = SocketDir::new();
        let path = dir.0.join("c");
        Listener {
            listener: UnixListener::bind(&path).unwrap(),
            path,
            _dir: dir,
        }
    }

    /// Whether a connection waits now. It never waits for one.
    fn pending(&self) -> bool {
        let mut fds = [PollFd::new(self.listener.as_fd(), PollFlags::POLLIN)];
        poll(&mut fds, 0u16).unwrap() == 1
    }

    /// The probe's connection and the pid and parent pid of its hello. A probe that never connects fails the test.
    fn accept(&self) -> (Control, (u32, u32)) {
        let mut fds = [PollFd::new(self.listener.as_fd(), PollFlags::POLLIN)];
        // timer: deadline — a probe that never connects fails the test instead of stalling it
        assert_eq!(
            poll(&mut fds, 5000u16).unwrap(),
            1,
            "the probe did not connect"
        );
        let (stream, _) = self.listener.accept().unwrap();
        let mut control = Control::new(stream);
        match control.report() {
            Report::Hello { pid, parent } => (control, (pid, parent)),
            other => panic!("the first report is {other:?}, not the hello"),
        }
    }
}

/// The harness's side of one probe's control socket. It keeps every input byte that the probe reported.
struct Control {
    stream: UnixStream,
    lines: BufReader<UnixStream>,
    input: Vec<u8>,
}

impl Control {
    fn new(stream: UnixStream) -> Control {
        // timer: deadline — a probe that stops answering fails the test instead of stalling it
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Control {
            lines: BufReader::new(stream.try_clone().unwrap()),
            stream,
            input: vec![],
        }
    }

    fn report(&mut self) -> Report {
        let mut line = String::new();
        assert!(
            self.lines.read_line(&mut line).unwrap() > 0,
            "the probe closed its socket"
        );
        serde_json::from_str(&line).unwrap()
    }

    fn keep_input(&mut self, hex: &str) -> Vec<u8> {
        let bytes = hex_decode(hex).unwrap();
        self.input.extend_from_slice(&bytes);
        bytes
    }

    /// Sends a request and returns when the probe acknowledged it, with the input that the probe reported before the
    /// acknowledgement.
    fn request(&mut self, request: &Request) -> Vec<u8> {
        let mut line = serde_json::to_vec(request).unwrap();
        line.push(b'\n');
        self.stream.write_all(&line).unwrap();
        let mut before = vec![];
        loop {
            match self.report() {
                Report::Ack {} => return before,
                Report::Input(hex) => before.extend(self.keep_input(&hex)),
                other => panic!("{other:?} before the acknowledgement"),
            }
        }
    }

    /// Sends `Queued` and returns the probe's count of unread stdin bytes (R-74).
    fn queued(&mut self) -> u64 {
        let mut line = serde_json::to_vec(&Request::Queued {}).unwrap();
        line.push(b'\n');
        self.stream.write_all(&line).unwrap();
        loop {
            match self.report() {
                Report::Queued { bytes } => return bytes,
                Report::Input(hex) => {
                    self.keep_input(&hex);
                }
                other => panic!("{other:?} before the queued count"),
            }
        }
    }

    /// Every input byte that the probe reported, once there are at least `n`.
    fn input_until(&mut self, n: usize) -> Vec<u8> {
        while self.input.len() < n {
            match self.report() {
                Report::Input(hex) => {
                    self.keep_input(&hex);
                }
                other => panic!("{other:?} while waiting for input"),
            }
        }
        self.input.clone()
    }
}

/// A probe on a fresh terminal, with the control socket of `listener` when one is given. The test holds the controller.
fn on_terminal(script: &str, listener: Option<&Listener>) -> (Probe, File) {
    let pty = openpty(None, None).unwrap();
    let env: Vec<(&str, &OsStr)> = listener
        .map(|l| (CONTROL_ENV, l.path.as_os_str()))
        .into_iter()
        .collect();
    let probe = Probe::spawn_env(
        script,
        Stdio::from(pty.slave.try_clone().unwrap()),
        Stdio::from(pty.slave),
        &env,
    );
    (probe, File::from(pty.master))
}

/// Writes the pattern a, b, ..., z, a, ... to the terminal without blocking, until the terminal takes no more, and returns what
/// it wrote. The pattern has no byte that the line discipline changes or acts on (R-48: `ICRNL`, `ISIG`, `IXON` stay on).
fn fill(controller: &File) -> Vec<u8> {
    let flags = OFlag::from_bits_truncate(fcntl(controller, FcntlArg::F_GETFL).unwrap());
    fcntl(controller, FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK)).unwrap();
    let mut wrote = vec![];
    loop {
        let chunk: Vec<u8> = (0..64)
            .map(|k| b'a' + ((wrote.len() + k) % 26) as u8)
            .collect();
        match (&*controller).write(&chunk) {
            Ok(n) => wrote.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => panic!("the terminal failed: {e}"),
        }
    }
    fcntl(controller, FcntlArg::F_SETFL(flags)).unwrap();
    wrote
}

#[test]
fn without_the_variable_a_holding_probe_never_reads_its_terminal() {
    // As before control mode: no reader thread, so nothing drains the terminal while the probe holds, and the kernel buffer fills.
    let (_probe, mut controller) = on_terminal(PRINT_R_HOLD, None);
    assert_eq!(read_terminal(&mut controller, 1), b"r");
    assert!(!fill(&controller).is_empty());
}

#[test]
fn with_the_variable_the_probe_connects_before_its_first_step_and_names_its_pid_and_parent() {
    let listener = Listener::new();
    let (probe, mut controller) = on_terminal(PRINT_R_HOLD, Some(&listener));
    assert_eq!(read_terminal(&mut controller, 1), b"r");
    // The first step ran, so the connection is already there.
    assert!(listener.pending());
    let (_control, (pid, parent)) = listener.accept();
    assert_eq!(pid, probe.pid().as_raw() as u32);
    // The test spawned the probe, as a worker spawns its payload.
    assert_eq!(parent, std::process::id());
}

#[test]
fn the_probe_connects_only_after_its_terminal_setup_so_input_after_the_connection_is_not_echoed() {
    // R-60: `program_ready` on the real tier is the probe's connection. When the harness accepts it, the R-48 setup is complete:
    // echo and canonical mode are already off, and a byte that the terminal receives after it is not echoed. HOLD has no first
    // step that prints, so nothing but the connection orders the read of the modes.
    let listener = Listener::new();
    let pty = openpty(None, None).unwrap();
    let path = ttyname(&pty.slave).unwrap();
    let before = tcgetattr(&pty.slave).unwrap();
    assert!(before
        .local_flags
        .contains(LocalFlags::ECHO | LocalFlags::ICANON));
    let _probe = Probe::spawn_env(
        HOLD,
        Stdio::from(pty.slave.try_clone().unwrap()),
        Stdio::from(pty.slave),
        &[(CONTROL_ENV, listener.path.as_os_str())],
    );
    let mut controller = File::from(pty.master);
    let (mut control, _) = listener.accept();
    let modes = tcgetattr(OpenOptions::new().read(true).open(&path).unwrap()).unwrap();
    assert!(!modes
        .local_flags
        .intersects(LocalFlags::ECHO | LocalFlags::ICANON));
    // A byte after the connection reaches the probe, and the terminal does not echo it: the first byte that the controller reads
    // is the probe's own output.
    controller.write_all(b"x").unwrap();
    assert_eq!(control.input_until(1), b"x");
    control.request(&Request::Output("72".into()));
    assert_eq!(read_terminal(&mut controller, 1), b"r");
}

#[test]
fn a_probe_that_cannot_connect_exits_with_the_connect_failed_code() {
    let dir = SocketDir::new();
    let missing = dir.0.join("none");
    let mut p = Probe::spawn_env(
        HOLD,
        Stdio::null(),
        Stdio::piped(),
        &[(CONTROL_ENV, missing.as_os_str())],
    );
    assert_eq!(p.wait().code(), Some(CONNECT_FAILED_EXIT));
}

#[test]
fn pty_output_the_probe_writes_the_bytes_to_its_terminal() {
    let listener = Listener::new();
    let (_probe, mut controller) = on_terminal(HOLD, Some(&listener));
    let (mut control, _) = listener.accept();
    control.request(&Request::Output("6869".into()));
    assert_eq!(read_terminal(&mut controller, 2), b"hi");
}

#[test]
fn pty_input_every_byte_that_the_probe_read_in_order() {
    let listener = Listener::new();
    let (_probe, mut controller) = on_terminal(HOLD, Some(&listener));
    let (mut control, _) = listener.accept();
    controller.write_all(b"ab").unwrap();
    assert_eq!(control.input_until(2), b"ab");
}

#[test]
fn print_after_input_matches_the_readers_log_in_control_mode() {
    // One reader: the reader thread takes "1" and reports it, and print_after_input finds it in the reader's log.
    let listener = Listener::new();
    let (_probe, mut controller) = on_terminal(
        r#"{"program":[{"print_after_input":{"match_hex":"31","bytes_hex":"07"}},{"hold":{}}]}"#,
        Some(&listener),
    );
    let (mut control, _) = listener.accept();
    controller.write_all(b"1").unwrap();
    assert_eq!(read_terminal(&mut controller, 1), b"\x07");
    assert_eq!(control.input_until(1), b"1");
}

#[test]
fn pty_blocked_stops_reads_so_the_kernel_buffer_fills_and_off_delivers_every_byte_in_order() {
    let listener = Listener::new();
    let (_probe, controller) = on_terminal(HOLD, Some(&listener));
    let (mut control, _) = listener.accept();
    assert!(control.request(&Request::Blocked(true)).is_empty());
    let wrote = fill(&controller);
    assert!(!wrote.is_empty());
    // A request that changes nothing: the probe served the socket with the bytes waiting, and read none of them.
    assert!(control.request(&Request::Output(String::new())).is_empty());
    control.request(&Request::Blocked(false));
    assert_eq!(control.input_until(wrote.len()), wrote);
}

#[test]
fn pty_queued_counts_the_unread_bytes_and_reads_none_of_them() {
    // R-74: the held-write fence. The count is the kernel's, and it is never asserted exactly (R-43 item 7): Linux counts only
    // the line discipline's buffer (4095 bytes on the pool's 6.12), not the bytes that wait in the tty's flip buffers, and it
    // moves bytes into the line discipline asynchronously. After the fill, the write side is full, so bytes wait: the count is
    // at least 1 and at most what the test wrote.
    let listener = Listener::new();
    let (_probe, mut controller) = on_terminal(HOLD, Some(&listener));
    let (mut control, _) = listener.accept();
    control.request(&Request::Blocked(true));
    assert_eq!(control.queued(), 0, "nothing written yet");
    controller.write_all(b"abc").unwrap();
    let wrote = fill(&controller);
    let queued = control.queued();
    assert!(
        (1..=3 + wrote.len() as u64).contains(&queued),
        "{queued} of {} bytes",
        3 + wrote.len()
    );
    // Queued is read-only: the probe still reads nothing until `Blocked(false)`, then reads every byte.
    assert!(control.input.is_empty());
    control.request(&Request::Blocked(false));
    let mut all = b"abc".to_vec();
    all.extend_from_slice(&wrote);
    assert_eq!(control.input_until(all.len()), all);
    assert_eq!(control.queued(), 0);
}

#[test]
fn pty_accept_reads_exactly_n_more_bytes_then_none_until_unblocked() {
    let listener = Listener::new();
    let (_probe, mut controller) = on_terminal(HOLD, Some(&listener));
    let (mut control, _) = listener.accept();
    control.request(&Request::Accept(2));
    controller.write_all(b"abcd").unwrap();
    assert_eq!(control.input_until(2), b"ab");
    // "cd" waits on the terminal, and the probe serves a request without reading it.
    assert!(control.request(&Request::Output(String::new())).is_empty());
    assert_eq!(control.input, b"ab");
    control.request(&Request::Blocked(false));
    assert_eq!(control.input_until(4), b"abcd");
}

#[test]
fn pty_chunk_each_read_takes_at_most_n_bytes() {
    let listener = Listener::new();
    let (_probe, mut controller) = on_terminal(HOLD, Some(&listener));
    let (mut control, _) = listener.accept();
    control.request(&Request::Chunk(1));
    controller.write_all(b"abc").unwrap();
    for byte in ["61", "62", "63"] {
        assert_eq!(control.report(), Report::Input(byte.into()));
    }
}

#[test]
fn an_unreadable_request_ends_a_holding_probe_with_the_control_failed_code() {
    // The probe holds, so only its control socket can end it. It never drops the socket while it lives: the end of the
    // socket that the harness reads means that the probe exited.
    let listener = Listener::new();
    let (mut probe, _controller) = on_terminal(HOLD, Some(&listener));
    let (mut control, _) = listener.accept();
    control.stream.write_all(b"{\"flush\":{}}\n").unwrap();
    assert_eq!(probe.wait().code(), Some(CONTROL_FAILED_EXIT));
    let mut rest = String::new();
    assert_eq!(control.lines.read_line(&mut rest).unwrap(), 0);
}

#[test]
fn the_harness_closing_its_end_ends_a_holding_probe_with_the_control_failed_code() {
    let listener = Listener::new();
    let (mut probe, _controller) = on_terminal(HOLD, Some(&listener));
    let (control, _) = listener.accept();
    drop(control);
    assert_eq!(probe.wait().code(), Some(CONTROL_FAILED_EXIT));
}

/// How many reports of `line` a fresh socket holds while its reader reads nothing: the count of whole writes before the first
/// write that does not fit. The probe's control socket has the same defaults, so its reports fill it at the same count.
fn reports_that_fit(line: &[u8]) -> usize {
    let (writer, _reader) = UnixStream::pair().unwrap();
    writer.set_nonblocking(true).unwrap();
    let mut whole = 0;
    while let Ok(n) = (&writer).write(line) {
        if n < line.len() {
            break;
        }
        whole += 1;
    }
    whole
}

#[test]
fn a_read_is_reported_before_the_script_acts_on_it_while_the_harness_does_not_read() {
    // The script exits as soon as it reads "x" (78). The harness first fills the probe's report direction with acknowledgements
    // and reads none of them, so the report of the read must wait for the harness. The probe sends that report before the script
    // can see the read, so the script cannot exit with the read unreported: the report comes before the end of the socket.
    const EXIT_AFTER_X: &str = r#"{"program":[{"print_after_input":{"match_hex":"78","bytes_hex":""}},{"exit":{"code":0}}]}"#;
    let listener = Listener::new();
    let (mut probe, mut controller) = on_terminal(EXIT_AFTER_X, Some(&listener));
    let (mut control, _) = listener.accept();
    let ack = [serde_json::to_vec(&Report::Ack {}).unwrap(), b"\n".to_vec()].concat();
    let count = reports_that_fit(&ack);
    let mut line = serde_json::to_vec(&Request::Blocked(false)).unwrap();
    line.push(b'\n');
    let requests = line.repeat(count);
    // The probe reads every request, because each acknowledgement still fits; a thread writes them so that a socket that holds
    // less than the count fails the test (the probe stops reading) instead of stalling it.
    let mut stream = control.stream.try_clone().unwrap();
    let (wrote, written) = channel();
    let writer = std::thread::spawn(move || {
        stream.write_all(&requests).unwrap();
        let _ = wrote.send(());
    });
    // timer: deadline — a probe that stops reading requests fails the test instead of stalling it
    written
        .recv_timeout(Duration::from_secs(3))
        .expect("the probe read every request");
    writer.join().unwrap();
    controller.write_all(b"x").unwrap();
    // The probe reads "x" at once. A probe that let the script see it before the report went out would exit within this wait
    // (the report never sent); a correct probe waits for the harness, whatever the wait.
    // timer: deadline — give a probe that publishes the read too early the time to exit
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        probe.child.try_wait().unwrap(),
        None,
        "the script acted on a read that the harness had not seen"
    );
    for _ in 0..count {
        assert_eq!(control.report(), Report::Ack {});
    }
    assert_eq!(control.report(), Report::Input("78".into()));
    assert_eq!(probe.wait().code(), Some(0));
    let mut rest = String::new();
    assert_eq!(control.lines.read_line(&mut rest).unwrap(), 0);
}
