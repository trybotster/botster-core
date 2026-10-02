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

/// The program of a session, run from a probe script.
///
/// Clause: Core A5-1 (the program edge), Core A5-2 (write-size variation), Core A5-3 (`pty_blocked`).
#[derive(Debug)]
pub struct ScriptedProgram {
    ops: Vec<Op>,
    cursor: usize,
    /// Everything that the program has read, as the probe's `seen` buffer.
    seen: Vec<u8>,
    output: VecDeque<u8>,
    exit: Option<ExitStatus>,
    exit_taken: bool,
    ignores_sigterm: bool,
    blocked: bool,
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
            output: VecDeque::new(),
            exit: None,
            exit_taken: false,
            ignores_sigterm: false,
            blocked: false,
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
        self.blocked = blocked;
    }

    /// The size that the worker last set.
    pub fn window_size(&self) -> Option<WindowSize> {
        self.size
    }

    /// True when a read would return bytes or the end of the output (readiness flag of the program edge, plan 2.5 rule 8).
    pub fn is_readable(&mut self) -> bool {
        self.advance();
        !self.output.is_empty() || self.exit.is_some()
    }

    /// Runs every step that can proceed.
    fn advance(&mut self) {
        while self.exit.is_none() {
            match &self.ops[self.cursor] {
                Op::Print(bytes) => self.output.extend(bytes),
                Op::PrintAfterInput { wanted, bytes } => {
                    let found = self
                        .seen
                        .windows(wanted.len().max(1))
                        .any(|w| w == wanted.as_slice());
                    if !found {
                        return;
                    }
                    self.output.extend(bytes);
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
        if self.blocked {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.seen.extend_from_slice(bytes);
        self.advance();
        Ok(bytes.len())
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
        let n = self.scheduler.with(|s| s.program_write_size(available));
        for slot in &mut buf[..n] {
            *slot = self.output.pop_front().unwrap_or_default();
        }
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
}
