//! The control mode of the probe (`botster_probe_script::control`): the real-tier side channel of the program controls.
//!
//! One event loop thread owns stdin and the control socket (steward condition 3: one stdin reader). It polls the socket always,
//! and stdin only while the [`Gate`] allows a read, so a stopped reader makes no `read` call at all and the kernel buffer fills
//! (R-47 item 7). It applies each request, then acknowledges it, so after an acknowledged `Blocked(true)` no read happens. Each
//! read goes to the harness as `Input` and to the buffer that `print_after_input` matches ([`Shared`]).
//!
//! Stdin and the socket are read with plain `read` calls on their descriptors, never through a user-space buffer: a buffer would
//! take more bytes from the kernel than the gate allows, and bytes left in it would be invisible to `poll`.

use botster_probe_script::control::{Report, Request, CONTROL_FAILED_EXIT};
use botster_route_codec::prelude::{hex_decode, HexBytes};
use rustix::event::{poll, PollFd, PollFlags};
use rustix::io::Errno;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// The size of one read of stdin before the gate's bounds, as in the probe's direct mode.
const READ_BUF: usize = 256;

/// What bounds the next read of stdin: `pty_blocked`, `pty_accept` and `pty_chunk`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Gate {
    blocked: bool,
    /// The bytes that may still be read, while an `Accept` limit is set.
    accept: Option<usize>,
    /// The largest single read, while a `Chunk` cap is set.
    chunk: Option<usize>,
}

impl Gate {
    /// Applies a request. `Output` and `Queued` do not change the gate.
    pub fn apply(&mut self, request: &Request) {
        match request {
            Request::Output(_) | Request::Queued {} => {}
            Request::Blocked(on) => {
                self.blocked = *on;
                if !on {
                    self.accept = None;
                }
            }
            Request::Accept(bytes) => self.accept = Some(*bytes),
            Request::Chunk(bytes) => self.chunk = Some(*bytes),
        }
    }

    /// The most that the next read may take from a buffer of `room` bytes; 0 means no read.
    pub fn read_size(&self, room: usize) -> usize {
        if self.blocked {
            return 0;
        }
        let capped = self.chunk.map_or(room, |chunk| room.min(chunk));
        self.accept.map_or(capped, |left| capped.min(left))
    }

    /// Counts a read of `bytes` against an `Accept` limit.
    pub fn took(&mut self, bytes: usize) {
        if let Some(left) = self.accept.as_mut() {
            *left -= bytes;
        }
    }
}

/// The bytes that stdin gave and no `print_after_input` consumed, and whether stdin ended.
#[derive(Debug, Default)]
struct Input {
    seen: Vec<u8>,
    ended: bool,
}

/// The reader's log, shared with the steps of the script.
#[derive(Debug, Default)]
pub struct Shared {
    input: Mutex<Input>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Input> {
        self.input.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, bytes: &[u8]) {
        self.lock().seen.extend_from_slice(bytes);
        self.changed.notify_all();
    }

    fn end(&self) {
        self.lock().ended = true;
        self.changed.notify_all();
    }

    /// The decision of `print_after_input` now: `Some(true)` when the bytes read so far contain `want` (they are consumed up
    /// to the end of the match), `Some(false)` when stdin ended with no match, `None` while more input may come. The match rule
    /// is the direct mode's ([`crate::match_end`]). [`Shared::take_through`] waits on the same decision; a test reads it without
    /// waiting, so a log that never fills fails the test instead of hanging it.
    #[cfg(test)]
    pub fn take_now(&self, want: &[u8]) -> Option<bool> {
        Self::take_from(&mut self.lock(), want)
    }

    fn take_from(input: &mut Input, want: &[u8]) -> Option<bool> {
        if let Some(end) = crate::match_end(&input.seen, want) {
            input.seen.drain(..end);
            return Some(true);
        }
        input.ended.then_some(false)
    }

    /// `print_after_input` in control mode: waits until [`Shared::take_now`] decides.
    pub fn take_through(&self, want: &[u8]) -> bool {
        let mut input = self.lock();
        loop {
            if let Some(found) = Self::take_from(&mut input, want) {
                return found;
            }
            input = self
                .changed
                .wait(input)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// Writes one report line, and waits while the harness does not read.
fn write_report(sink: &mut impl Write, report: &Report) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(report).expect("a report serializes");
    line.push(b'\n');
    sink.write_all(&line)
}

/// Sends one report. A report that cannot be sent ends the probe ([`control_failed`]).
fn send(mut socket: &UnixStream, report: &Report) {
    if write_report(&mut socket, report).is_err() {
        control_failed();
    }
}

/// Delivers one read of stdin (`None`: stdin ended). The report goes to the harness first, and only then do the bytes (or the
/// end) go to the script: so the script never acts on a read that the harness cannot see, even when it exits at once. An error
/// means that the report was not sent, and the script got nothing.
fn deliver(sink: &mut impl Write, shared: &Shared, read: Option<&[u8]>) -> std::io::Result<()> {
    match read {
        Some(bytes) => {
            write_report(sink, &Report::Input(HexBytes(bytes.to_vec()).to_hex()))?;
            shared.push(bytes);
        }
        None => {
            write_report(sink, &Report::InputEnd {})?;
            shared.end();
        }
    }
    Ok(())
}

/// Sends the first report, the probe's pid and its parent's. An error means that the harness cannot be reached.
pub fn hello(mut socket: &UnixStream) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(&Report::Hello {
        pid: std::process::id(),
        parent: std::os::unix::process::parent_id(),
    })
    .expect("a report serializes");
    line.push(b'\n');
    socket.write_all(&line)
}

/// The probe's end when its control socket fails: the harness closed its end, the socket or the poll failed, a request did not
/// parse, or a report could not be sent. The probe exits instead of dropping the socket, so the end of the socket always means
/// that the probe exited.
fn control_failed() -> ! {
    std::process::exit(CONTROL_FAILED_EXIT)
}

/// The event loop: it runs until the probe exits. `output` takes the bytes of each `Output` request, in order, for the writer
/// thread.
pub fn serve(socket: &UnixStream, shared: &Shared, output: &Sender<Vec<u8>>) -> ! {
    let stdin = rustix::stdio::stdin();
    let mut sink = socket;
    let mut gate = Gate::default();
    let mut ended = false;
    let mut pending: Vec<u8> = vec![];
    let mut buf = [0u8; READ_BUF];
    loop {
        let read_stdin = !ended && gate.read_size(READ_BUF) > 0;
        let mut fds = vec![PollFd::new(socket, PollFlags::IN)];
        if read_stdin {
            fds.push(PollFd::new(&stdin, PollFlags::IN));
        }
        match poll(&mut fds, None) {
            Ok(_) => {}
            Err(Errno::INTR) => continue,
            Err(_) => control_failed(),
        }
        let socket_ready = !fds[0].revents().is_empty();
        let stdin_ready = read_stdin && !fds[1].revents().is_empty();
        drop(fds);
        if socket_ready {
            // The same plain read as for stdin: no user-space buffer, so a second request line is never hidden from `poll`.
            let mut chunk = [0u8; READ_BUF];
            match rustix::io::read(socket, &mut chunk) {
                Ok(0) => control_failed(),
                Ok(n) => pending.extend_from_slice(&chunk[..n]),
                Err(Errno::INTR) => {}
                Err(_) => control_failed(),
            }
            while let Some(at) = pending.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = pending.drain(..=at).collect();
                let Ok(request) = serde_json::from_slice::<Request>(&line) else {
                    // A harness of another version: the probe does not guess.
                    control_failed()
                };
                if let Request::Output(hex) = &request {
                    let _ = output.send(hex_decode(hex).unwrap_or_default());
                }
                if let Request::Queued {} = request {
                    // The kernel's count of unread stdin bytes; the gate is unchanged, so no read follows from it (R-74).
                    let bytes =
                        rustix::io::ioctl_fionread(stdin).unwrap_or_else(|_| control_failed());
                    send(socket, &Report::Queued { bytes });
                    continue;
                }
                gate.apply(&request);
                send(socket, &Report::Ack {});
            }
            // The gate may have changed: decide again before any read of stdin.
            continue;
        }
        if stdin_ready {
            let size = gate.read_size(buf.len());
            match rustix::io::read(stdin, &mut buf[..size]) {
                Ok(0) => {
                    ended = true;
                    deliver(&mut sink, shared, None).unwrap_or_else(|_| control_failed());
                }
                Ok(n) => {
                    gate.took(n);
                    deliver(&mut sink, shared, Some(&buf[..n]))
                        .unwrap_or_else(|_| control_failed());
                }
                Err(Errno::INTR) => {}
                Err(_) => {
                    ended = true;
                    deliver(&mut sink, shared, None).unwrap_or_else(|_| control_failed());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh gate reads a full buffer: no request bounds a read.
    #[test]
    fn a_fresh_gate_reads_the_whole_room() {
        assert_eq!(Gate::default().read_size(256), 256);
    }

    /// `pty_blocked`: on stops every read; off resumes them and removes an accept limit, but keeps a chunk cap.
    #[test]
    fn blocked_stops_reads_and_off_lifts_an_accept_limit() {
        let mut gate = Gate::default();
        gate.apply(&Request::Chunk(4));
        gate.apply(&Request::Accept(2));
        gate.apply(&Request::Blocked(true));
        assert_eq!(gate.read_size(256), 0);
        gate.apply(&Request::Blocked(false));
        assert_eq!(gate.read_size(256), 4);
    }

    /// `pty_accept`: the reads take at most the limit in total, then none.
    #[test]
    fn accept_bounds_the_total_of_the_reads() {
        let mut gate = Gate::default();
        gate.apply(&Request::Accept(3));
        assert_eq!(gate.read_size(256), 3);
        assert_eq!(gate.read_size(2), 2);
        gate.took(2);
        assert_eq!(gate.read_size(256), 1);
        gate.took(1);
        assert_eq!(gate.read_size(256), 0);
    }

    /// `pty_chunk`: each read takes at most the cap, and a smaller room still bounds it.
    #[test]
    fn chunk_caps_each_read() {
        let mut gate = Gate::default();
        gate.apply(&Request::Chunk(2));
        assert_eq!(gate.read_size(256), 2);
        assert_eq!(gate.read_size(1), 1);
        gate.took(2);
        assert_eq!(gate.read_size(256), 2);
    }

    /// The smaller of a chunk cap and an accept limit bounds a read.
    #[test]
    fn chunk_and_accept_together_take_the_smaller() {
        let mut gate = Gate::default();
        gate.apply(&Request::Chunk(2));
        gate.apply(&Request::Accept(5));
        assert_eq!(gate.read_size(256), 2);
        gate.apply(&Request::Accept(1));
        assert_eq!(gate.read_size(256), 1);
    }

    /// `Output` does not change the gate.
    #[test]
    fn output_leaves_the_gate_unchanged() {
        let mut gate = Gate::default();
        gate.apply(&Request::Blocked(true));
        let before = gate;
        gate.apply(&Request::Output("61".into()));
        assert_eq!(gate, before);
    }

    /// A report sink that records, at each write, what `print_after_input` of "x" would decide then.
    struct Watch<'a> {
        shared: &'a Shared,
        decided: Vec<Option<bool>>,
        wrote: Vec<u8>,
    }

    impl Write for Watch<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.decided.push(self.shared.take_now(b"x"));
            self.wrote.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A report sink whose harness went away.
    struct Gone;

    impl Write for Gone {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A read goes to the harness before the script can match it, and the end of stdin before the script can see it.
    #[test]
    fn a_read_and_the_end_are_reported_before_the_script_sees_them() {
        let shared = Shared::default();
        let mut sink = Watch {
            shared: &shared,
            decided: vec![],
            wrote: vec![],
        };
        deliver(&mut sink, &shared, Some(b"x")).unwrap();
        assert_eq!(shared.take_now(b"x"), Some(true));
        deliver(&mut sink, &shared, None).unwrap();
        assert_eq!(shared.take_now(b"x"), Some(false));
        assert_eq!(sink.decided, [None, None]);
        assert_eq!(sink.wrote, b"{\"input\":\"78\"}\n{\"input_end\":{}}\n");
    }

    /// A report that cannot be sent gives the script nothing: no bytes and no end.
    #[test]
    fn an_unsent_report_gives_the_script_nothing() {
        let shared = Shared::default();
        assert!(deliver(&mut Gone, &shared, Some(b"x")).is_err());
        assert!(deliver(&mut Gone, &shared, None).is_err());
        assert_eq!(shared.take_now(b"x"), None);
    }

    /// `print_after_input` in control mode: it waits while no match and no end came, consumes through a match and keeps what
    /// follows, and fails at the end of stdin. An empty match needs no input.
    #[test]
    fn print_after_input_decides_on_the_readers_log() {
        let shared = Shared::default();
        assert_eq!(shared.take_now(b"ab"), None);
        shared.push(b"abcd");
        assert_eq!(shared.take_now(b"ab"), Some(true));
        assert_eq!(shared.take_now(b"ab"), None);
        assert!(shared.take_through(b"cd"));
        assert_eq!(shared.take_now(b"cd"), None);
        shared.end();
        assert_eq!(shared.take_now(b"cd"), Some(false));
        assert!(!shared.take_through(b"cd"));
        assert!(shared.take_through(b""));
    }
}
