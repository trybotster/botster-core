//! The probe-script format: the scripted program that a session runs (Core A5-1, design 6.2).
//!
//! One script drives every consumer: the probe binary against real Core (slow tier), the in-memory scripted program backend of the Core
//! testkit, and FakeCore. This crate holds the types, the parser and the validator only. It has no execution logic: an interpreter that
//! drives a real PTY belongs to the probe binary, and an in-memory interpreter belongs to the testkit.
//!
//! The script is JSON in `argv[1]` of a spawn whose `argv[0]` names the probe binary.
//!
//! A script carries an opaque `fake` part, at the top and on a step. It holds data that a terminal library and a PTY program supply for
//! a fake (R-5). Every consumer except a fake ignores it. A fake gives its own type for the top part through the parameter of [`Script`].

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod control;
mod schema;
pub use schema::{render, schema};

/// The file name of the probe binary in `argv[0]` (Core A5-1, design 6.2).
pub const PROBE_BINARY: &str = "botster-conformance-probe";

/// Script data of one step for a fake (R-5): what a terminal library would find in the printed bytes. A real subject ignores it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StepFake {
    /// The modes that the printed bytes set, as a partial `ModeFlags`.
    pub modes: Option<serde_json::Value>,
}

/// One step of the program. The program runs its steps in order; a step that waits holds the later ones back (Core A5-1). Every step
/// continues with the next one, except `Exit`, `SignalSelf` and `Hold`, which end the program or never finish (design 6.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Writes bytes to the terminal.
    Print {
        bytes_hex: String,
        #[serde(default)]
        fake: StepFake,
    },
    /// Waits until the program has read the matching input, then writes bytes. The match consumes the input up to its end: a later
    /// `print_after_input` matches only input that comes after that point (design 6.2). Input after the match stays for the later step. An empty `match_hex` matches at once and consumes nothing.
    PrintAfterInput {
        match_hex: String,
        bytes_hex: String,
        #[serde(default)]
        fake: StepFake,
    },
    /// Exits with a code.
    Exit { code: i32 },
    /// Kills itself with a signal.
    SignalSelf { n: i32 },
    /// Ignores `SIGTERM`. This applies to this process only; a `fork_child` process follows only its own script (design 6.2).
    IgnoreSigterm {},
    /// Starts a child process that runs a script in the same process group.
    ForkChild { script: Vec<Step> },
    /// Runs until a signal ends it.
    Hold {},
}

/// A probe script. `F` is the type of the top-level `fake` part: the opaque JSON value by default, or a fake's own type.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Script<F: Default = serde_json::Value> {
    #[serde(default)]
    pub program: Vec<Step>,
    /// Script data for a fake (R-5). Real Core and the probe binary ignore it.
    #[serde(default)]
    pub fake: F,
}

impl<F: Default> Default for Script<F> {
    fn default() -> Self {
        Script {
            program: Vec::new(),
            fake: F::default(),
        }
    }
}

/// A script that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// The text is not a script.
    Parse(String),
    /// A step is not valid. `path` names it, for example `program[2].script[0]`.
    Invalid { path: String, why: String },
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::Parse(why) => f.write_str(why),
            ScriptError::Invalid { path, why } => write!(f, "{path}: {why}"),
        }
    }
}

impl std::error::Error for ScriptError {}

impl<F: DeserializeOwned + Default> Script<F> {
    /// Parses the JSON text of a script. This checks the shape only; [`Script::validate`] checks the values.
    pub fn parse(text: &str) -> Result<Self, ScriptError> {
        serde_json::from_str(text).map_err(|e| ScriptError::Parse(e.to_string()))
    }

    /// The script of a spawn `argv`. An `argv` that does not name the probe binary gives the default script, which holds (Core A5-1).
    /// A script that does not parse is an error.
    pub fn from_argv(argv: &[String]) -> Result<Self, ScriptError> {
        let probe = argv
            .first()
            .is_some_and(|a| a.rsplit('/').next() == Some(PROBE_BINARY));
        match (probe, argv.get(1)) {
            (true, Some(text)) => Self::parse(text),
            _ => Ok(Self::default()),
        }
    }
}

impl<F: Default> Script<F> {
    /// True when a step of the program ignores `SIGTERM`.
    pub fn ignores_sigterm(&self) -> bool {
        self.program
            .iter()
            .any(|s| matches!(s, Step::IgnoreSigterm {}))
    }

    /// Checks the values of every step, children included: hex fields decode, a signal number is in 1 to 64, and a child script is
    /// checked in turn. A parse does not call this: a consumer that must keep its old leniency parses only.
    pub fn validate(&self) -> Result<(), ScriptError> {
        validate_steps(&self.program, "program")
    }
}

fn validate_steps(steps: &[Step], at: &str) -> Result<(), ScriptError> {
    for (i, step) in steps.iter().enumerate() {
        let path = format!("{at}[{i}]");
        let bad = |field: &str, why: String| ScriptError::Invalid {
            path: format!("{path}.{field}"),
            why,
        };
        let hex = |field: &str, text: &str| {
            botster_route_codec::prelude::hex_decode(text)
                .map(|_| ())
                .map_err(|why| bad(field, why))
        };
        match step {
            Step::Print { bytes_hex, .. } => hex("bytes_hex", bytes_hex)?,
            Step::PrintAfterInput {
                match_hex,
                bytes_hex,
                ..
            } => {
                hex("match_hex", match_hex)?;
                hex("bytes_hex", bytes_hex)?;
            }
            Step::SignalSelf { n } if !(1..=64).contains(n) => {
                return Err(bad("n", "a signal number is 1 to 64".to_string()))
            }
            Step::ForkChild { script } => validate_steps(script, &format!("{path}.script"))?,
            Step::SignalSelf { .. }
            | Step::Exit { .. }
            | Step::IgnoreSigterm {}
            | Step::Hold {} => {}
        }
    }
    Ok(())
}
