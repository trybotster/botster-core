//! The probe binary: a session program for real-Core runs (design 6.2). `argv[1]` is the probe script, the same JSON that FakeCore reads.
//!
//! Steps run in order (design 6.2, Core A5-1). `print` writes bytes to stdout, `print_after_input` waits until stdin has produced the
//! bytes and then writes, `ignore_sigterm` makes SIGTERM without effect, and `fork_child` runs a script in a child of the same process
//! group. Each of these four continues with the next step. `exit` ends with a code, `signal_self` ends by a signal, and `hold` runs until
//! a signal ends it, so no step after one of these three runs.
//!
//! `print_after_input` consumes the stdin bytes up to the end of its match. A later `print_after_input` matches only bytes that stdin
//! produces after that point. Bytes that came after the match in the same read stay for the next step. The Core testkit follows the same
//! rule (see `Step::PrintAfterInput` in `botster-probe-script`).
//!
//! The workspace forbids `unsafe`, so `ignore_sigterm` registers a handler that does nothing (through `signal-hook`) instead of
//! installing `SIG_IGN`. In this process both give the same result: SIGTERM does not end the program. `signal_self` calls `raise`.
//!
//! When stdin is a terminal (a session's PTY), the probe sets it to non-canonical mode with no echo before its first step, as an
//! interactive program does (steward ruling R-48). Then a byte with no newline reaches `print_after_input` at once, and the terminal
//! does not echo input. Only `ICANON`, `ECHO`, `ECHOE`, `ECHOK` and `ECHONL` are cleared, and `VMIN` is 1 and `VTIME` 0. `ISIG`,
//! `ICRNL` and `OPOST` stay as they are. A pipe or a file on stdin is not changed.
//!
//! With `BOTSTER_PROBE_CONTROL` set (`botster_probe_script::control`), the probe connects to the harness's control socket after
//! that setup and before its first step, and serves the program controls (module `control`). Its reader thread then owns stdin,
//! and `print_after_input` matches the reader's log with the same rule. Without the variable, nothing here changes.

mod control;

use botster_probe_script::control::{CONNECT_FAILED_EXIT, CONTROL_ENV, TERMINAL_SETUP_FAILED_EXIT};
use botster_probe_script::{Script, Step};
use botster_route_codec::prelude::hex_decode;
use rustix::termios::{
    isatty, tcgetattr, tcsetattr, LocalModes, OptionalActions, SpecialCodeIndex,
};
use signal_hook::consts::SIGTERM;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{exit, Command};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;

/// The end of the first match of `want` in `seen`; an empty `want` matches at once (design 6.2).
fn match_end(seen: &[u8], want: &[u8]) -> Option<usize> {
    if want.is_empty() {
        return Some(0);
    }
    seen.windows(want.len())
        .position(|w| w == want)
        .map(|at| at + want.len())
}

/// Sets a terminal stdin to non-canonical mode with no echo (R-48). An error ends the probe with
/// [`TERMINAL_SETUP_FAILED_EXIT`] before its first step: it never runs on a canonical terminal in silence (steward ruling R-74).
fn interactive_stdin() {
    let stdin = std::io::stdin();
    if !isatty(&stdin) {
        return;
    }
    let mut modes = tcgetattr(&stdin).unwrap_or_else(|e| terminal_setup_failed("tcgetattr", e));
    // One removal per flag: the flags are distinct bits, so a `|` chain would have `^` as an equivalent mutant.
    for flag in [
        LocalModes::ICANON,
        LocalModes::ECHO,
        LocalModes::ECHOE,
        LocalModes::ECHOK,
        LocalModes::ECHONL,
    ] {
        modes.local_modes.remove(flag);
    }
    modes.special_codes[SpecialCodeIndex::VMIN] = 1;
    modes.special_codes[SpecialCodeIndex::VTIME] = 0;
    if let Err(e) = tcsetattr(&stdin, OptionalActions::Now, &modes) {
        terminal_setup_failed("tcsetattr", e);
    }
}

/// Ends the probe after a failed R-48 terminal setup, with the call and the error on stderr.
fn terminal_setup_failed(call: &str, error: rustix::io::Errno) -> ! {
    eprintln!("botster-conformance-probe: the terminal setup failed (R-48): {call}: {error}");
    exit(TERMINAL_SETUP_FAILED_EXIT)
}

/// Where `print_after_input` finds its input: stdin itself (the probe's own reads, as without a control socket), or the log of
/// the control mode's reader, which then owns stdin.
enum Source<'a> {
    Direct,
    Reader(&'a control::Shared),
}

fn run(steps: &[Step], source: &Source<'_>) {
    let mut seen: Vec<u8> = vec![];
    for step in steps {
        match step {
            Step::Print { bytes_hex, .. } => {
                let bytes = hex_decode(bytes_hex).unwrap_or_default();
                let mut out = std::io::stdout();
                let _ = out.write_all(&bytes).and_then(|_| out.flush());
            }
            Step::PrintAfterInput {
                match_hex,
                bytes_hex,
                ..
            } => {
                let want = hex_decode(match_hex).unwrap_or_default();
                if let Source::Reader(shared) = source {
                    if !shared.take_through(&want) {
                        exit(2);
                    }
                    let bytes = hex_decode(bytes_hex).unwrap_or_default();
                    let mut out = std::io::stdout();
                    let _ = out.write_all(&bytes).and_then(|_| out.flush());
                    continue;
                }
                let mut buf = [0u8; 256];
                let end = loop {
                    if let Some(end) = match_end(&seen, &want) {
                        break end;
                    }
                    // signal-hook-registry installs the ignore_sigterm handler with SA_RESTART, so this read never fails with EINTR and no retry is
                    // needed (lead ruling 2026-10-01 allows dropping the retry). The slow test ignore_sigterm_continues_and_survives_sigterm proves it.
                    match std::io::stdin().read(&mut buf) {
                        Ok(0) => exit(2),
                        Ok(n) => seen.extend_from_slice(&buf[..n]),
                        Err(_) => exit(2),
                    }
                };
                seen.drain(..end);
                let bytes = hex_decode(bytes_hex).unwrap_or_default();
                let mut out = std::io::stdout();
                let _ = out.write_all(&bytes).and_then(|_| out.flush());
            }
            Step::Exit { code } => exit(*code),
            Step::SignalSelf { n } => {
                let _ = signal_hook::low_level::raise(*n);
                // A signal that does not end the process leaves it here: signal_self never continues. park can return early, so loop.
                loop {
                    std::thread::park();
                }
            }
            Step::IgnoreSigterm {} => {
                let _ = signal_hook::flag::register(SIGTERM, Arc::new(AtomicBool::new(false)));
            }
            Step::ForkChild { script } => {
                let json = serde_json::json!({ "program": script }).to_string();
                let exe = std::env::current_exe().expect("own path");
                // The child never takes the parent's control socket: one probe, one reader of stdin. Without the variable this
                // removes nothing.
                let _ = Command::new(exe).arg(json).env_remove(CONTROL_ENV).spawn();
            }
            Step::Hold {} => loop {
                std::thread::park();
            },
        }
    }
    exit(0)
}

fn main() {
    let text = std::env::args().nth(1).unwrap_or_else(|| "{}".to_string());
    let Ok(script) = serde_json::from_str::<Script>(&text) else {
        exit(64)
    };
    interactive_stdin();
    let Some(path) = std::env::var_os(CONTROL_ENV) else {
        run(&script.program, &Source::Direct);
        return;
    };
    // Control mode: connect after the terminal setup (R-48) and before the first step, so the harness's accept is its fence.
    let Ok(socket) = UnixStream::connect(&path) else {
        exit(CONNECT_FAILED_EXIT)
    };
    if control::hello(&socket).is_err() {
        exit(CONNECT_FAILED_EXIT);
    }
    let shared = Arc::new(control::Shared::default());
    let (output, to_write) = channel::<Vec<u8>>();
    // The writer thread: `pty_output` bytes, in order. A write that the terminal blocks holds only this thread (Core OU-3d).
    std::thread::spawn(move || {
        for bytes in to_write {
            let mut out = std::io::stdout();
            let _ = out.write_all(&bytes).and_then(|_| out.flush());
        }
    });
    let reader = Arc::clone(&shared);
    std::thread::spawn(move || control::serve(&socket, &reader, &output));
    run(&script.program, &Source::Reader(&shared));
}
