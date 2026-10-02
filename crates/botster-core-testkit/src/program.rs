//! The scripted program edge (plan 2.3, Core A5-1): a probe script interpreted in-process, in place of the PTY child.
//!
//! The script is the `botster-probe-script` format of the contracts. The interpreter follows the probe binary of the
//! contracts step by step (`botster-conformance-probe`, the program of the slow tier), so one script gives the same program in
//! both tiers. It supplies program output only, never a terminal state: every terminal byte and state comes from libghostty in
//! the worker (A5-1, steward rule R-7).
//!
//! A script is run lazily, on the first call that touches the program, and runs every step that can proceed. The visible
//! timing (when output is read, when the exit is seen) is chosen by the driver and the scheduler, not by the script.

use crate::scheduler::SchedulerHandle;
use botster_core_edges::edges::{ExitStatus, WindowSize};
use botster_core_edges::Program;
use botster_probe_script::{Script, ScriptError, Step};
use botster_route_codec::prelude::hex_decode;
use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// A script that the in-process program cannot run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    Script(ScriptError),
    /// A step that needs a real process (`fork_child`: a child in the process group of the program). The ids that use it are
    /// real-process tests (Core A6-1, SV-9).
    RealOnly {
        path: String,
        step: &'static str,
    },
}

impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgramError::Script(e) => e.fmt(f),
            ProgramError::RealOnly { path, step } => {
                write!(f, "{path}: `{step}` needs a real process")
            }
        }
    }
}

impl std::error::Error for ProgramError {}

#[derive(Debug, Clone)]
enum Op {
    Print(Vec<u8>),
    PrintAfterInput {
        wanted: Vec<u8>,
        bytes: Vec<u8>,
    },
    Exit(ExitStatus),
    Hold,
    /// Sets the flag when execution reaches it: the in-process program receives no signal, and the Process edge reads the flag
    /// to decide whether a SIGTERM ends the program.
    IgnoreSigterm,
}

/// What a test scripts about the program edge (the program controls of `docs/core-testkit-controls.md`). It is state of the edge,
/// shared with a [`ProgramControl`] handle so a test can script a program that the worker owns.
#[derive(Debug, Default)]
struct Controls {
    /// Writes return `WouldBlock` (`pty_blocked`).
    blocked: bool,
    /// At most this many more bytes of input are taken, then none (`pty_accept`).
    accept: Option<usize>,
    /// This many more bytes are taken, then the next write fails (`pty_fail_after`).
    fail_after: Option<usize>,
    /// At most this many bytes of input reach the PTY per step, a pump of the host (`pty_chunk`).
    input_cap: Option<usize>,
    /// The bytes of the cap that the current step has left.
    input_left: usize,
    /// A write found the step's cap spent: the next step takes more of it (the host's next pump has work).
    step_refused: bool,
    /// Every read returns at most this many bytes (`program_write_size`).
    write_cap: Option<usize>,
    /// Output that the program writes besides its script: `(bytes, atomic)` (`program_write_once`, `uncarriable_sequence`).
    inject: VecDeque<(Vec<u8>, bool)>,
    /// Every byte of input that the program took (the log behind `pty_input`).
    input_log: Vec<u8>,
    /// Output bytes that the worker has not read: the program's queue and the injected bytes (`pty_output_unread`).
    unread: usize,
}

/// A handle on the controls of a [`ScriptedProgram`]. It outlives the move of the program into a worker.
#[derive(Debug, Clone)]
pub struct ProgramControl(Arc<Mutex<Controls>>);

impl ProgramControl {
    fn lock(&self) -> MutexGuard<'_, Controls> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `pty_blocked`: writes return `WouldBlock` while on. Turning it off also lifts a `pty_accept` limit.
    pub fn set_blocked(&self, on: bool) {
        let mut controls = self.lock();
        controls.blocked = on;
        if !on {
            controls.accept = None;
        }
    }

    /// `pty_accept`: the program takes at most `bytes` more bytes of input, then none until `set_blocked(false)`. A write that
    /// straddles the limit has its accepted prefix written (Core A5-2, IN-2: `Partial`).
    pub fn accept_at_most(&self, bytes: usize) {
        self.lock().accept = Some(bytes);
    }

    /// `pty_fail_after`: the program takes `bytes` more bytes, then the next write fails with an OS error (Core IN-2).
    pub fn fail_after(&self, bytes: usize) {
        self.lock().fail_after = Some(bytes);
    }

    /// `pty_chunk`: at most `bytes` bytes of input reach the PTY per step (a pump of the host), so a host write reaches the
    /// PTY in pieces across pumps (Core AM-2, IN-6: the definition of the contracts' FakeCore). `None` lifts the cap.
    pub fn input_chunk(&self, bytes: Option<usize>) {
        let mut controls = self.lock();
        controls.input_cap = bytes;
        controls.input_left = bytes.unwrap_or(0);
    }

    /// A new step begins: the cap of `pty_chunk` is available again.
    pub fn new_step(&self) {
        let mut controls = self.lock();
        if let Some(cap) = controls.input_cap {
            controls.input_left = cap;
        }
        controls.step_refused = false;
    }

    /// True when a write found this step's `pty_chunk` cap spent: the next step continues it.
    pub fn waits_for_next_step(&self) -> bool {
        self.lock().step_refused
    }

    /// `pty_output`: the program writes these plain bytes, after its script's output that can run now. A read may end
    /// anywhere inside them (Core A5-2).
    pub fn write_plain(&self, bytes: &[u8]) {
        let mut controls = self.lock();
        controls.unread += bytes.len();
        controls.inject.push_back((bytes.to_vec(), false));
    }

    /// `program_write_size`: every read returns at most `bytes` bytes; `None` gives the choice back to the scheduler.
    pub fn write_size(&self, bytes: Option<usize>) {
        self.lock().write_cap = bytes;
    }

    /// `program_write_once`: the program writes these bytes as one write, so the worker reads them in one piece (Core A5-2, E2-3).
    pub fn write_once(&self, bytes: &[u8]) {
        let mut controls = self.lock();
        controls.unread += bytes.len();
        controls.inject.push_back((bytes.to_vec(), true));
    }

    /// `uncarriable_sequence`: the program writes the start of an OSC or DCS string and more than `limit` bytes of it, with no
    /// terminator (Core A8-2). `kind` is `osc` or `dcs`; the caller names the continuation limit of the snapshot format.
    pub fn write_unterminated(&self, kind: UnterminatedKind, limit: usize) {
        let mut bytes = match kind {
            UnterminatedKind::Osc => b"\x1b]0;".to_vec(),
            UnterminatedKind::Dcs => b"\x1bP1$r".to_vec(),
        };
        bytes.extend(std::iter::repeat_n(b'a', limit + 1));
        let mut controls = self.lock();
        controls.unread += bytes.len();
        controls.inject.push_back((bytes, false));
    }

    /// Every byte of input that the program took, in order.
    pub fn input_log(&self) -> Vec<u8> {
        self.lock().input_log.clone()
    }

    /// `pty_output_unread`: the output bytes that the worker has not read.
    pub fn output_unread(&self) -> usize {
        self.lock().unread
    }
}

/// The string that `uncarriable_sequence` leaves open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnterminatedKind {
    Osc,
    Dcs,
}

/// The output that the worker has not read: pieces in the order that the program wrote them. An atomic piece is read in one
/// piece when the buffer holds it (`program_write_once`), and is never read together with the bytes around it. Plain bytes that
/// follow plain bytes are one piece, so the scheduler chooses where a read ends within them.
#[derive(Debug, Default)]
struct Output {
    pieces: VecDeque<Piece>,
    len: usize,
}

#[derive(Debug)]
struct Piece {
    bytes: VecDeque<u8>,
    atomic: bool,
}

impl Output {
    fn push(&mut self, bytes: &[u8], atomic: bool) {
        if bytes.is_empty() {
            return;
        }
        self.len += bytes.len();
        match self.pieces.back_mut() {
            Some(back) if !atomic && !back.atomic => back.bytes.extend(bytes),
            _ => self.pieces.push_back(Piece {
                bytes: bytes.iter().copied().collect(),
                atomic,
            }),
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Moves the bytes of one read into `buf`: at most the front piece and the buffer. `choose` picks the size of a read of a plain
    /// piece (at least 1, at most its argument). Returns the bytes moved.
    fn take(&mut self, buf: &mut [u8], choose: impl FnOnce(usize) -> usize) -> usize {
        let Some(front) = self.pieces.front_mut() else {
            return 0;
        };
        let fits = front.bytes.len().min(buf.len());
        let n = if front.atomic { fits } else { choose(fits) };
        for slot in &mut buf[..n] {
            if let Some(byte) = front.bytes.pop_front() {
                *slot = byte;
            }
        }
        if front.bytes.is_empty() {
            self.pieces.pop_front();
        }
        self.len -= n;
        n
    }
}

/// The program of a session, run from a probe script.
///
/// Clause: Core A5-1 (the program edge), Core A5-2 (write-size variation), Core A5-3 (`pty_blocked`).
#[derive(Debug)]
pub struct ScriptedProgram {
    ops: Vec<Op>,
    cursor: usize,
    /// Everything that the program has read, as the probe's `seen` buffer.
    seen: Vec<u8>,
    output: Output,
    exit: Option<ExitStatus>,
    exit_taken: bool,
    ignores_sigterm: bool,
    controls: ProgramControl,
    size: Option<WindowSize>,
    scheduler: SchedulerHandle,
}

impl ScriptedProgram {
    /// The program of a spawn `argv`. An `argv` that does not name the probe binary gives the default script, which holds
    /// (Core A5-1); a probe script ends with exit code 0 after its last step, as the probe binary does.
    pub fn from_argv(
        argv: &[String],
        scheduler: &SchedulerHandle,
    ) -> Result<ScriptedProgram, ProgramError> {
        let script = Script::<serde_json::Value>::from_argv(argv).map_err(ProgramError::Script)?;
        let probe = argv
            .first()
            .is_some_and(|a| a.rsplit('/').next() == Some(botster_probe_script::PROBE_BINARY));
        ScriptedProgram::new(&script, probe, scheduler)
    }

    /// `ends` is true when the program exits with code 0 after its last step (a probe script), false when it holds.
    pub fn new(
        script: &Script,
        ends: bool,
        scheduler: &SchedulerHandle,
    ) -> Result<ScriptedProgram, ProgramError> {
        script.validate().map_err(ProgramError::Script)?;
        let mut ops = Vec::new();
        for (i, step) in script.program.iter().enumerate() {
            let decode = |text: &str| hex_decode(text).unwrap_or_default();
            ops.push(match step {
                Step::Print { bytes_hex, .. } => Op::Print(decode(bytes_hex)),
                Step::PrintAfterInput {
                    match_hex,
                    bytes_hex,
                    ..
                } => Op::PrintAfterInput {
                    wanted: decode(match_hex),
                    bytes: decode(bytes_hex),
                },
                Step::Exit { code } => Op::Exit(ExitStatus::Code(*code)),
                Step::SignalSelf { n } => Op::Exit(ExitStatus::Signal(*n)),
                Step::Hold {} => Op::Hold,
                Step::IgnoreSigterm {} => Op::IgnoreSigterm,
                Step::ForkChild { .. } => {
                    return Err(ProgramError::RealOnly {
                        path: format!("program[{i}]"),
                        step: "fork_child",
                    })
                }
            });
        }
        ops.push(if ends {
            Op::Exit(ExitStatus::Code(0))
        } else {
            Op::Hold
        });
        Ok(ScriptedProgram {
            ops,
            cursor: 0,
            seen: Vec::new(),
            output: Output::default(),
            exit: None,
            exit_taken: false,
            ignores_sigterm: false,
            controls: ProgramControl(Arc::default()),
            size: None,
            scheduler: scheduler.clone(),
        })
    }

    /// True once execution has reached a step that ignores `SIGTERM` (the steps run in order, and a waiting step holds the
    /// later ones back, Core A5-1).
    pub fn ignores_sigterm(&mut self) -> bool {
        self.advance();
        self.ignores_sigterm
    }

    /// A signal that reaches the program's process group (Core LC-5, LC-6), with its default disposition: `SIGKILL` ends the
    /// program; `SIGTERM` ends it unless execution reached `ignore_sigterm`; `SIGHUP`, `SIGINT`, `SIGQUIT`, `SIGABRT`,
    /// `SIGPIPE` and `SIGALRM` end it. The in-process program installs no other handler, and a signal outside this list is
    /// not modelled: it changes nothing. A program that already ended is not changed.
    pub fn signal(&mut self, signal: i32) {
        const SIGHUP: i32 = 1;
        const SIGINT: i32 = 2;
        const SIGQUIT: i32 = 3;
        const SIGABRT: i32 = 6;
        const SIGKILL: i32 = 9;
        const SIGPIPE: i32 = 13;
        const SIGALRM: i32 = 14;
        const SIGTERM: i32 = 15;
        self.advance();
        if self.exit.is_some() {
            return;
        }
        let ends = match signal {
            SIGTERM => !self.ignores_sigterm,
            SIGHUP | SIGINT | SIGQUIT | SIGABRT | SIGKILL | SIGPIPE | SIGALRM => true,
            _ => false,
        };
        if ends {
            self.exit = Some(ExitStatus::Signal(signal));
        }
    }

    /// Makes writes to the program return `WouldBlock` (A5-3, `pty_blocked`), or lets them proceed again.
    pub fn set_blocked(&mut self, blocked: bool) {
        self.controls.set_blocked(blocked);
    }

    /// The handle on this program's controls.
    pub fn control(&self) -> ProgramControl {
        self.controls.clone()
    }

    /// The size that the worker last set.
    pub fn window_size(&self) -> Option<WindowSize> {
        self.size
    }

    /// True when a write would not return `WouldBlock`: it takes a byte, or fails at once (the write readiness of the program
    /// edge, plan 2.5 rule 8).
    pub fn is_writable(&mut self) -> bool {
        self.advance();
        if self.exit.is_some() {
            return true;
        }
        let controls = self.controls.lock();
        let step_spent = controls.input_cap.is_some() && controls.input_left == 0;
        controls.fail_after == Some(0)
            || (!controls.blocked && controls.accept != Some(0) && !step_spent)
    }

    /// True when a read would return bytes or the end of the output (readiness flag of the program edge, plan 2.5 rule 8).
    pub fn is_readable(&mut self) -> bool {
        self.advance();
        !self.output.is_empty() || self.exit.is_some()
    }

    /// Takes the output that a control injected.
    fn take_injected(&mut self) {
        let injected: Vec<(Vec<u8>, bool)> = self.controls.lock().inject.drain(..).collect();
        for (bytes, atomic) in injected {
            self.output.push(&bytes, atomic);
        }
    }

    /// Runs every step that can proceed.
    fn advance(&mut self) {
        // The script's output that can run now comes first; the bytes that a control injected follow it.
        self.run_steps();
        self.take_injected();
        self.controls.lock().unread = self.output.len();
    }

    fn run_steps(&mut self) {
        while self.exit.is_none() {
            match &self.ops[self.cursor] {
                Op::Print(bytes) => self.output.push(bytes, false),
                Op::PrintAfterInput { wanted, bytes } => {
                    let found = self
                        .seen
                        .windows(wanted.len().max(1))
                        .any(|w| w == wanted.as_slice());
                    if !found {
                        return;
                    }
                    self.output.push(bytes, false);
                }
                Op::Exit(status) => {
                    self.exit = Some(*status);
                    return;
                }
                Op::Hold => return,
                Op::IgnoreSigterm => self.ignores_sigterm = true,
            }
            self.cursor += 1;
        }
    }
}

impl Program for ScriptedProgram {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.advance();
        if self.exit.is_some() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let taken = {
            let mut controls = self.controls.lock();
            if controls.fail_after == Some(0) {
                return Err(io::Error::from_raw_os_error(5));
            }
            let mut room = bytes.len();
            if controls.input_cap.is_some() {
                room = room.min(controls.input_left);
            }
            if let Some(accept) = controls.accept {
                room = room.min(accept);
            }
            if let Some(left) = controls.fail_after {
                room = room.min(left);
            }
            if (controls.blocked || room == 0) && !bytes.is_empty() {
                if !controls.blocked && controls.input_cap.is_some() && controls.input_left == 0 {
                    controls.step_refused = true;
                }
                return Err(io::ErrorKind::WouldBlock.into());
            }
            if let Some(accept) = &mut controls.accept {
                *accept -= room;
            }
            if let Some(left) = &mut controls.fail_after {
                *left -= room;
            }
            if controls.input_cap.is_some() {
                controls.input_left -= room;
            }
            controls.input_log.extend_from_slice(&bytes[..room]);
            room
        };
        self.seen.extend_from_slice(&bytes[..taken]);
        self.advance();
        Ok(taken)
    }

    /// Reads a seed-chosen number of the bytes that the program has written (A5-2, "the program edge varies its write sizes").
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.advance();
        let available = self.output.len().min(buf.len());
        if available == 0 {
            // The end of the output (the program ended and every byte was read), a zero-length buffer, or nothing yet.
            let ended = self.output.is_empty() && self.exit.is_some();
            return if ended || buf.is_empty() {
                Ok(0)
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            };
        }
        let scheduler = self.scheduler.clone();
        let cap = self.controls.lock().write_cap;
        let n = self.output.take(buf, |fits| match cap {
            Some(cap) => fits.min(cap).max(1),
            None => scheduler.with(|s| s.program_write_size(fits)),
        });
        self.controls.lock().unread = self.output.len();
        Ok(n)
    }

    fn resize(&mut self, size: WindowSize) -> io::Result<()> {
        self.size = Some(size);
        Ok(())
    }

    fn poll_exit(&mut self) -> Option<ExitStatus> {
        self.advance();
        if self.exit_taken {
            return None;
        }
        self.exit_taken = self.exit.is_some();
        self.exit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn program(script: serde_json::Value, ends: bool, seed: u64) -> ScriptedProgram {
        let script: Script = serde_json::from_value(script).expect("a script");
        ScriptedProgram::new(&script, ends, &SchedulerHandle::with_seed(seed)).expect("runs here")
    }

    fn read_all(p: &mut ScriptedProgram) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = [0u8; 64];
        while let Ok(n) = p.read(&mut buf) {
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        out
    }

    /// A5-1: the program writes output bytes, in order, and exits with a code at its script point.
    #[test]
    fn it_prints_then_exits_with_a_code() {
        let mut p = program(
            json!({"program": [{"print": {"bytes_hex": "6869"}}, {"print": {"bytes_hex": "21"}}, {"exit": {"code": 7}}]}),
            true,
            0,
        );
        assert_eq!(read_all(&mut p), b"hi!");
        assert_eq!(p.poll_exit(), Some(ExitStatus::Code(7)));
        assert_eq!(p.poll_exit(), None, "the exit is reported once");
        assert_eq!(p.read(&mut [0u8; 4]).unwrap(), 0, "the output ended");
    }

    /// A5-1: it exits with a signal at a script point.
    #[test]
    fn it_ends_with_a_signal() {
        let mut p = program(json!({"program": [{"signal_self": {"n": 9}}]}), true, 0);
        assert_eq!(p.poll_exit(), Some(ExitStatus::Signal(9)));
    }

    /// A5-1: it reads the bytes written to it. A step that waits holds the later ones back.
    #[test]
    fn a_step_that_waits_holds_the_later_ones_back() {
        let mut p = program(
            json!({"program": [
                {"print_after_input": {"match_hex": "6162", "bytes_hex": "6f6b"}},
                {"exit": {"code": 0}}]}),
            true,
            0,
        );
        assert_eq!(
            p.read(&mut [0u8; 4]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(p.poll_exit(), None);
        p.write(b"xa").unwrap();
        assert_eq!(p.poll_exit(), None, "half of the match");
        p.write(b"b").unwrap();
        assert_eq!(read_all(&mut p), b"ok");
        assert_eq!(p.poll_exit(), Some(ExitStatus::Code(0)));
        assert_eq!(p.write(b"z").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    /// The default script holds (A5-1); a probe script that ends exits with code 0, as the probe binary does.
    #[test]
    fn the_default_script_holds_and_a_probe_script_ends() {
        let sched = SchedulerHandle::with_seed(0);
        let mut held = ScriptedProgram::from_argv(&["/bin/sh".to_string()], &sched).unwrap();
        assert_eq!(held.poll_exit(), None);
        assert!(!held.is_readable());
        let argv = [
            "/x/botster-conformance-probe".to_string(),
            r#"{"program":[]}"#.to_string(),
        ];
        let mut ended = ScriptedProgram::from_argv(&argv, &sched).unwrap();
        assert_eq!(ended.poll_exit(), Some(ExitStatus::Code(0)));
        let hold = [
            "botster-conformance-probe".to_string(),
            r#"{"program":[{"hold":{}}]}"#.to_string(),
        ];
        assert_eq!(
            ScriptedProgram::from_argv(&hold, &sched)
                .unwrap()
                .poll_exit(),
            None
        );
    }

    /// A5-3: `pty_blocked` blocks writes until it is lifted.
    #[test]
    fn a_blocked_program_refuses_writes() {
        let mut p = program(json!({"program": [{"hold": {}}]}), true, 0);
        p.set_blocked(true);
        assert_eq!(p.write(b"a").unwrap_err().kind(), io::ErrorKind::WouldBlock);
        p.set_blocked(false);
        assert_eq!(p.write(b"a").unwrap(), 1);
    }

    /// A5-2: a seed chooses the size of each chunk that the program writes. Every byte arrives, in order.
    #[test]
    fn a_seed_chooses_the_write_sizes() {
        let sizes = |seed| {
            let hex = "00".repeat(30);
            let mut p = program(
                json!({"program": [{"print": {"bytes_hex": hex}}]}),
                true,
                seed,
            );
            let mut sizes = Vec::new();
            let mut buf = [0u8; 16];
            loop {
                match p.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => sizes.push(n),
                    Err(e) => panic!("{e}"),
                }
            }
            assert_eq!(sizes.iter().sum::<usize>(), 30);
            sizes
        };
        assert_eq!(sizes(3), sizes(3));
        assert!((4..16).any(|seed| sizes(seed) != sizes(3)));
    }

    /// A script that needs a real process is refused, with the step named. A bad hex value is a script error.
    #[test]
    fn a_script_that_cannot_run_in_process_is_refused() {
        let sched = SchedulerHandle::with_seed(0);
        let fork: Script =
            serde_json::from_value(json!({"program": [{"fork_child": {"script": []}}]})).unwrap();
        assert_eq!(
            ScriptedProgram::new(&fork, true, &sched).unwrap_err(),
            ProgramError::RealOnly {
                path: "program[0]".into(),
                step: "fork_child"
            }
        );
        let bad: Script =
            serde_json::from_value(json!({"program": [{"print": {"bytes_hex": "zz"}}]})).unwrap();
        assert!(matches!(
            ScriptedProgram::new(&bad, true, &sched),
            Err(ProgramError::Script(_))
        ));
        let sigterm: Script =
            serde_json::from_value(json!({"program": [{"ignore_sigterm": {}}]})).unwrap();
        assert!(ScriptedProgram::new(&sigterm, true, &sched)
            .unwrap()
            .ignores_sigterm());
    }

    /// A5-1: `ignore_sigterm` takes effect when execution reaches it, not before. A waiting step before it holds it back, and a
    /// step after a `hold` is never reached.
    #[test]
    fn ignore_sigterm_applies_at_its_script_step() {
        let mut waits = program(
            json!({"program": [
                {"print_after_input": {"match_hex": "61", "bytes_hex": "62"}},
                {"ignore_sigterm": {}},
                {"hold": {}}]}),
            true,
            0,
        );
        assert!(!waits.ignores_sigterm());
        waits.write(b"a").unwrap();
        assert!(waits.ignores_sigterm());
        let mut never = program(
            json!({"program": [{"hold": {}}, {"ignore_sigterm": {}}]}),
            true,
            0,
        );
        assert!(!never.ignores_sigterm());
    }

    /// The text of each error names the step or the script fault.
    #[test]
    fn an_error_names_its_cause() {
        let real = ProgramError::RealOnly {
            path: "program[2]".into(),
            step: "fork_child",
        };
        assert_eq!(
            real.to_string(),
            "program[2]: `fork_child` needs a real process"
        );
        let sched = SchedulerHandle::with_seed(0);
        let bad: Script =
            serde_json::from_value(json!({"program": [{"print": {"bytes_hex": "zz"}}]})).unwrap();
        let text = ScriptedProgram::new(&bad, true, &sched)
            .unwrap_err()
            .to_string();
        assert!(text.starts_with("program[0].bytes_hex: "), "{text}");
    }

    /// LC-5, LC-6: a signal ends the program by its default disposition; `ignore_sigterm` keeps it alive through `SIGTERM`
    /// only; an unmodelled signal and a signal after the end change nothing.
    #[test]
    fn a_signal_ends_the_program_by_its_default_disposition() {
        let ended = |script: serde_json::Value, signal: i32| {
            let mut p = program(script, true, 0);
            p.signal(signal);
            p.poll_exit()
        };
        let holds = json!({"program": [{"hold": {}}]});
        for signal in [1, 2, 3, 6, 9, 13, 14, 15] {
            assert_eq!(
                ended(holds.clone(), signal),
                Some(ExitStatus::Signal(signal))
            );
        }
        assert_eq!(ended(holds.clone(), 28), None, "not modelled");
        let ignores = json!({"program": [{"ignore_sigterm": {}}, {"hold": {}}]});
        assert_eq!(ended(ignores.clone(), 15), None);
        assert_eq!(ended(ignores, 9), Some(ExitStatus::Signal(9)));
        let mut done = program(json!({"program": [{"exit": {"code": 4}}]}), true, 0);
        done.signal(9);
        assert_eq!(done.poll_exit(), Some(ExitStatus::Code(4)));
    }

    /// The window size is the last size that the worker set.
    #[test]
    fn the_window_size_is_kept() {
        let mut p = program(json!({"program": [{"hold": {}}]}), true, 0);
        assert_eq!(p.window_size(), None);
        let size = WindowSize {
            cols: 80,
            rows: 24,
            width_px: 0,
            height_px: 0,
        };
        p.resize(size).unwrap();
        assert_eq!(p.window_size(), Some(size));
    }

    /// The program is readable when output waits, and when it has ended (the end of the output); it is not readable while it
    /// holds with nothing to read.
    #[test]
    fn readiness_is_output_or_the_end() {
        let mut holds = program(json!({"program": [{"hold": {}}]}), true, 0);
        assert!(!holds.is_readable());
        let mut output = program(
            json!({"program": [{"print": {"bytes_hex": "61"}}, {"hold": {}}]}),
            true,
            0,
        );
        assert!(output.is_readable());
        let mut ended = program(json!({"program": [{"exit": {"code": 0}}]}), true, 0);
        assert!(ended.is_readable());
    }

    fn holds() -> ScriptedProgram {
        program(json!({"program": [{"hold": {}}]}), true, 0)
    }

    /// `pty_accept`: at most N more bytes, a straddling write has its accepted prefix, then none until the block is lifted.
    #[test]
    fn pty_accept_takes_a_prefix_then_blocks_until_lifted() {
        let mut p = holds();
        let control = p.control();
        control.accept_at_most(3);
        assert_eq!(p.write(b"abcdef").unwrap(), 3);
        assert_eq!(p.write(b"d").unwrap_err().kind(), io::ErrorKind::WouldBlock);
        control.set_blocked(false);
        assert_eq!(
            p.write(b"def").unwrap(),
            3,
            "lifting the block lifts the limit"
        );
        assert_eq!(control.input_log(), b"abcdef");
    }

    /// `pty_fail_after`: N more bytes, then the next write fails with an OS error; the log holds what was taken.
    #[test]
    fn pty_fail_after_fails_the_write_after_n_bytes() {
        let mut p = holds();
        let control = p.control();
        control.fail_after(2);
        assert_eq!(p.write(b"abc").unwrap(), 2);
        let error = p.write(b"c").unwrap_err();
        assert_eq!(error.raw_os_error(), Some(5));
        assert_eq!(control.input_log(), b"ab");
    }

    /// `pty_blocked` through the handle reaches a program that the worker owns.
    #[test]
    fn the_handle_blocks_a_program_that_moved() {
        let p = holds();
        let control = p.control();
        let mut moved = Box::new(p);
        control.set_blocked(true);
        assert_eq!(
            moved.write(b"a").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        control.set_blocked(false);
        assert_eq!(moved.write(b"a").unwrap(), 1);
    }

    /// `program_write_size`: every read returns at most N bytes, whatever the seed chooses.
    #[test]
    fn the_write_size_control_caps_every_read() {
        let hex = "61".repeat(10);
        let mut p = program(
            json!({"program": [{"print": {"bytes_hex": hex}}, {"hold": {}}]}),
            true,
            5,
        );
        p.control().write_size(Some(3));
        let mut buf = [0u8; 16];
        let mut sizes = Vec::new();
        for _ in 0..4 {
            sizes.push(p.read(&mut buf).unwrap());
        }
        assert_eq!(sizes, [3, 3, 3, 1]);
    }

    /// `program_write_once`: the bytes are read in one piece, after the bytes before them, and the pieces before are never merged
    /// into it.
    #[test]
    fn write_once_is_read_in_one_piece() {
        let mut p = program(
            json!({"program": [{"print": {"bytes_hex": "6162"}}, {"hold": {}}]}),
            true,
            3,
        );
        let control = p.control();
        control.write_once(b"WXYZ");
        let mut buf = [0u8; 16];
        let mut pieces = Vec::new();
        while let Ok(n) = p.read(&mut buf) {
            pieces.push(buf[..n].to_vec());
        }
        let flat: Vec<u8> = pieces.concat();
        assert_eq!(flat, b"abWXYZ");
        assert!(pieces.contains(&b"WXYZ".to_vec()), "{pieces:?}");
    }

    /// An atomic write that does not fit the buffer is split only because it must be.
    #[test]
    fn a_small_buffer_splits_an_atomic_write_without_loss() {
        let mut p = holds();
        p.control().write_once(b"abcdef");
        let mut buf = [0u8; 4];
        assert_eq!(p.read(&mut buf).unwrap(), 4);
        assert_eq!(&buf, b"abcd");
        assert_eq!(p.read(&mut buf).unwrap(), 2);
        assert_eq!(&buf[..2], b"ef");
    }

    /// `pty_output_unread`: the bytes that the worker has not read, the injected ones included.
    #[test]
    fn output_unread_counts_what_the_worker_has_not_read() {
        let mut p = program(
            json!({"program": [{"print": {"bytes_hex": "61626364"}}, {"hold": {}}]}),
            true,
            0,
        );
        let control = p.control();
        p.control().write_size(Some(1));
        assert!(p.is_readable());
        assert_eq!(control.output_unread(), 4);
        p.read(&mut [0u8; 8]).unwrap();
        assert_eq!(control.output_unread(), 3);
        control.write_once(b"xy");
        assert_eq!(
            control.output_unread(),
            5,
            "before the program takes them in"
        );
    }

    /// `uncarriable_sequence`: an open OSC or DCS string longer than the limit, with no terminator.
    #[test]
    fn an_unterminated_sequence_exceeds_the_limit() {
        for (kind, start) in [
            (UnterminatedKind::Osc, &b"\x1b]"[..]),
            (UnterminatedKind::Dcs, &b"\x1bP"[..]),
        ] {
            let mut p = holds();
            p.control().write_unterminated(kind, 100);
            let mut out = Vec::new();
            let mut buf = [0u8; 256];
            while let Ok(n) = p.read(&mut buf) {
                out.extend_from_slice(&buf[..n]);
            }
            assert!(out.starts_with(start));
            assert!(out.len() > 100 + start.len());
            assert!(
                !out.contains(&0x07) && !out.windows(2).any(|w| w == b"\x1b\\"),
                "no terminator"
            );
        }
    }

    /// Every read of the program until it would block, as separate pieces.
    fn read_pieces(p: &mut ScriptedProgram, buf_len: usize) -> Vec<Vec<u8>> {
        let mut buf = vec![0u8; buf_len];
        let mut pieces = Vec::new();
        while let Ok(n) = p.read(&mut buf) {
            assert!(
                n > 0 || pieces.is_empty() || p.control().output_unread() == 0,
                "Ok(0) while output remains"
            );
            if n == 0 {
                break;
            }
            pieces.push(buf[..n].to_vec());
        }
        pieces
    }

    /// Several atomic writes are each read whole and never merged with each other or with plain bytes; one that does not fit the
    /// buffer is split without loss; a write injected after the queue drained is read whole too; no byte is invented.
    #[test]
    fn queued_atomic_writes_keep_their_boundaries() {
        let mut p = holds();
        let control = p.control();
        control.write_size(Some(3));
        control.write_once(b"abcdef");
        control.write_once(b"XYZ");
        assert_eq!(
            read_pieces(&mut p, 4),
            [b"abcd".to_vec(), b"ef".to_vec(), b"XYZ".to_vec()]
        );
        assert_eq!(control.output_unread(), 0);
        control.write_once(b"pq");
        assert_eq!(
            read_pieces(&mut p, 4),
            [b"pq".to_vec()],
            "one read, no padding"
        );
        // Plain bytes between atomic writes stay between them, and are read at the scripted size.
        let mut mixed = program(
            json!({"program": [{"print": {"bytes_hex": "6162636465"}}, {"hold": {}}]}),
            true,
            0,
        );
        let control = mixed.control();
        control.write_size(Some(2));
        control.write_once(b"WX");
        control.write_once(b"YZ");
        assert_eq!(
            read_pieces(&mut mixed, 8),
            [
                b"ab".to_vec(),
                b"cd".to_vec(),
                b"e".to_vec(),
                b"WX".to_vec(),
                b"YZ".to_vec()
            ]
        );
    }

    /// An empty atomic write is nothing: it is no piece, it does not end a read with `Ok(0)`, and the output behind it is read.
    #[test]
    fn an_empty_atomic_write_is_no_piece() {
        let mut p = holds();
        let control = p.control();
        control.write_once(b"");
        assert!(!p.is_readable());
        assert_eq!(
            p.read(&mut [0u8; 4]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        control.write_once(b"");
        control.write_once(b"ab");
        control.write_once(b"");
        assert_eq!(read_pieces(&mut p, 8), [b"ab".to_vec()]);
        assert_eq!(control.output_unread(), 0);
    }

    /// `uncarriable_sequence`: the open string is the start plus exactly `limit + 1` bytes, and the unread count includes them.
    #[test]
    fn an_unterminated_sequence_is_one_byte_over_the_limit() {
        let mut p = holds();
        let control = p.control();
        control.write_unterminated(UnterminatedKind::Osc, 10);
        assert_eq!(
            control.output_unread(),
            4 + 11,
            "the start `ESC ] 0 ;` and limit + 1 bytes"
        );
        control.write_unterminated(UnterminatedKind::Dcs, 3);
        assert_eq!(control.output_unread(), 15 + 5 + 4);
        let mut buf = [0u8; 64];
        let mut total = 0;
        while let Ok(n) = p.read(&mut buf) {
            total += n;
        }
        assert_eq!(total, 24);
    }

    /// `Output::push`: plain bytes after plain bytes are one piece; an atomic piece is never joined to its neighbours.
    #[test]
    fn plain_bytes_merge_and_an_atomic_piece_stands_alone() {
        let mut output = Output::default();
        output.push(b"ab", false);
        output.push(b"cd", false);
        assert_eq!(output.pieces.len(), 1);
        output.push(b"EF", true);
        output.push(b"gh", false);
        output.push(b"IJ", true);
        output.push(b"kl", false);
        output.push(b"mn", false);
        let shape: Vec<(usize, bool)> = output
            .pieces
            .iter()
            .map(|p| (p.bytes.len(), p.atomic))
            .collect();
        assert_eq!(
            shape,
            [(4, false), (2, true), (2, false), (2, true), (4, false)]
        );
        assert_eq!(output.len(), 14);
        output.push(b"", true);
        output.push(b"", false);
        assert_eq!(output.pieces.len(), 5);
    }

    /// `pty_chunk` (Core AM-2, IN-6): at most the cap reaches the PTY per step; the rest waits for the next step, and the
    /// input log holds every byte once, in order.
    #[test]
    fn pty_chunk_caps_the_input_of_a_step() {
        let mut p = program(json!({"program": [{"hold": {}}]}), true, 0);
        let control = p.control();
        control.input_chunk(Some(2));
        assert_eq!(p.write(b"abcde").unwrap(), 2);
        assert_eq!(
            p.write(b"cde").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(!p.is_writable(), "the step's cap is spent");
        assert!(control.waits_for_next_step());
        control.new_step();
        assert!(!control.waits_for_next_step());
        assert!(p.is_writable());
        assert_eq!(p.write(b"cde").unwrap(), 2);
        control.new_step();
        assert_eq!(p.write(b"e").unwrap(), 1);
        control.input_chunk(None);
        assert_eq!(p.write(b"xyz").unwrap(), 3);
        assert_eq!(control.input_log(), b"abcdexyz");
    }

    /// Plan 2.5 rule 8: the program is writable unless it is blocked or its accept limit is spent; a write that would fail
    /// at once, and an ended program, count as writable (the write returns at once).
    #[test]
    fn writability_follows_the_controls() {
        let mut p = program(json!({"program": [{"hold": {}}]}), true, 0);
        let control = p.control();
        assert!(p.is_writable());
        control.set_blocked(true);
        assert!(!p.is_writable());
        control.set_blocked(false);
        control.accept_at_most(1);
        assert!(p.is_writable());
        p.write(b"ab").unwrap();
        assert!(!p.is_writable(), "the accept limit is spent");
        control.set_blocked(false);
        control.fail_after(0);
        assert!(p.is_writable(), "the next write fails at once");
        let mut ended = program(json!({"program": [{"exit": {"code": 0}}]}), true, 0);
        assert!(ended.is_writable());
    }

    /// `pty_output` (Core A5-2): plain bytes follow the script's output, every byte arrives in order, and the unread count
    /// covers them.
    #[test]
    fn write_plain_adds_plain_output() {
        let mut p = program(
            json!({"program": [{"print": {"bytes_hex": "6869"}}, {"hold": {}}]}),
            true,
            0,
        );
        let control = p.control();
        control.write_plain(b"!!");
        assert!(p.is_readable());
        assert_eq!(control.output_unread(), 4);
        let mut out = Vec::new();
        let mut buf = [0u8; 8];
        while let Ok(n) = p.read(&mut buf) {
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        assert_eq!(out, b"hi!!");
        assert_eq!(control.output_unread(), 0);
    }
}
