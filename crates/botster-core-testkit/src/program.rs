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
    PrintAfterInput { wanted: Vec<u8>, bytes: Vec<u8> },
    Exit(ExitStatus),
    Hold,
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
                // A flag only: the in-process program receives no signal. The Process edge reads it to decide whether a
                // SIGTERM ends the program.
                Step::IgnoreSigterm {} => continue,
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
            ignores_sigterm: script.ignores_sigterm(),
            blocked: false,
            size: None,
            scheduler: scheduler.clone(),
        })
    }

    /// True when a step of the script ignores `SIGTERM`.
    pub fn ignores_sigterm(&self) -> bool {
        self.ignores_sigterm
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
}
