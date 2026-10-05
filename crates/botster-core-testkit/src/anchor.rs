//! The protocol between `botster-test-anchor` and its guard (`real::AnchorGuard`, slow tier). Lead ruling (2026-10-04,
//! RealCoreHarness): the real harness reaches real processes only through public launch inputs, `OpenConfig.worker_path` and
//! the session program, so the process that Core or the worker spawns is a wrapper that execs the real binary.
//!
//! - **Wrapper.** The guard puts a symbolic link to the prebuilt anchor binary in a directory of its own, with [`CONFIG_FILE`]
//!   beside it. The wrapper finds that file from its `argv[0]`, the convention of a multi-call binary: Core and the worker
//!   start a program by its path, and neither the environment (A2-1: it is exact) nor the arguments can carry it.
//! - **Stages.** The wrapper starts the two helper stages by running its own binary with the `argv[0]` of [`STAGE_INTERMEDIATE`]
//!   or [`STAGE_ANCHOR`], so no argument of the real program can select a stage.
//! - **Lines.** Each process writes [`Line`]s to its guard connection, one JSON object per line.
//!
//! This module has no process code. It is compiled in every tier so that the default tier checks the encoding.

use botster_core_edges::edges::ProcessIdentity;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

/// The configuration of a wrapper: the file beside its link.
pub const CONFIG_FILE: &str = "anchor.json";
/// `argv[0]` of the stage that starts the anchor and exits at once.
pub const STAGE_INTERMEDIATE: &str = "botster-test-anchor:intermediate";
/// `argv[0]` of the detached anchor.
pub const STAGE_ANCHOR: &str = "botster-test-anchor:anchor";
/// The exit code of a wrapper that could not exec the real binary (the shell convention for a command that cannot run).
pub const WRAP_FAILED: i32 = 127;

/// What a wrapper needs: its guard, the cleanup grace and the real binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The guard's listening socket.
    pub socket: PathBuf,
    /// The time between the group `TERM` and the group `KILL`: the `stop_grace` of the Core limits that the harness opened
    /// with (Core LC-5). The anchor adds no timeout of its own.
    pub grace: Duration,
    /// The verified prebuilt binary that the wrapper execs.
    pub binary: PathBuf,
}

fn path_text(path: &std::path::Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("{} is not UTF-8", path.display()))
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    value.get(name).ok_or_else(|| format!("no `{name}`"))
}

fn text(value: &Value, name: &str) -> Result<String, String> {
    field(value, name)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("`{name}` is not a string"))
}

fn number(value: &Value, name: &str) -> Result<u64, String> {
    field(value, name)?
        .as_u64()
        .ok_or_else(|| format!("`{name}` is not a number"))
}

fn pid(value: &Value, name: &str) -> Result<u32, String> {
    u32::try_from(number(value, name)?).map_err(|_| format!("`{name}` is not a pid"))
}

fn identity(value: &Value, name: &str) -> Result<ProcessIdentity, String> {
    let value = field(value, name)?;
    Ok(ProcessIdentity {
        pid: pid(value, "pid")?,
        start_time: number(value, "start_time")?,
    })
}

fn identity_json(identity: ProcessIdentity) -> Value {
    json!({"pid": identity.pid, "start_time": identity.start_time})
}

impl Config {
    pub fn encode(&self) -> Result<String, String> {
        let grace = u64::try_from(self.grace.as_nanos())
            .map_err(|_| "the grace does not fit in u64 nanoseconds".to_string())?;
        Ok(json!({
            "socket": path_text(&self.socket)?,
            "grace_ns": grace,
            "binary": path_text(&self.binary)?,
        })
        .to_string())
    }

    pub fn decode(text_in: &str) -> Result<Config, String> {
        let value: Value = serde_json::from_str(text_in).map_err(|e| e.to_string())?;
        Ok(Config {
            socket: text(&value, "socket")?.into(),
            grace: Duration::from_nanos(number(&value, "grace_ns")?),
            binary: text(&value, "binary")?.into(),
        })
    }
}

/// The group that an anchor holds, reported before the wrapper execs the real binary (ruling item 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The anchor process: a member of `group` for as long as it lives, so the group id cannot be reused meanwhile.
    pub anchor: ProcessIdentity,
    /// The process group id.
    pub group: u32,
    /// The wrapper, which becomes the real binary with the same pid and start time.
    pub leader: ProcessIdentity,
    /// The real binary that the wrapper execs.
    pub binary: PathBuf,
}

/// One line on a guard connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// The anchor holds the group. The wrapper execs only after this line.
    Anchor(Report),
    /// The guard ended: the anchor sends `TERM` to the group and waits the grace.
    Term,
    /// The anchor sends `KILL` to the group, its last act: the signal ends the anchor too.
    Kill,
    /// The anchor signals nothing (ruling item 13), for example because the real binary moved to another group.
    Refused { reason: String },
    /// A stage failed. `stage` is `wrap`, `intermediate` or `anchor`.
    Error { stage: String, error: String },
}

impl Line {
    pub fn encode(&self) -> String {
        match self {
            Line::Anchor(report) => json!({
                "kind": "anchor",
                "anchor": identity_json(report.anchor),
                "group": report.group,
                "leader": identity_json(report.leader),
                "binary": report.binary.to_string_lossy(),
            }),
            Line::Term => json!({"kind": "term"}),
            Line::Kill => json!({"kind": "kill"}),
            Line::Refused { reason } => json!({"kind": "refused", "reason": reason}),
            Line::Error { stage, error } => json!({"kind": "error", "stage": stage, "error": error}),
        }
        .to_string()
    }

    pub fn decode(line: &str) -> Result<Line, String> {
        let value: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        match text(&value, "kind")?.as_str() {
            "anchor" => Ok(Line::Anchor(Report {
                anchor: identity(&value, "anchor")?,
                group: pid(&value, "group")?,
                leader: identity(&value, "leader")?,
                binary: text(&value, "binary")?.into(),
            })),
            "term" => Ok(Line::Term),
            "kill" => Ok(Line::Kill),
            "refused" => Ok(Line::Refused {
                reason: text(&value, "reason")?,
            }),
            "error" => Ok(Line::Error {
                stage: text(&value, "stage")?,
                error: text(&value, "error")?,
            }),
            other => Err(format!("unknown line kind `{other}`")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wrapper reads the file that the guard wrote: every field survives the round trip, the grace to the nanosecond.
    #[test]
    fn a_config_survives_its_file() {
        let config = Config {
            socket: "/tmp/bt1-0/guard".into(),
            grace: botster_core_contract::prelude::CoreLimits::default().stop_grace
                + Duration::from_nanos(1),
            binary: "/target/candidate/botster-worker".into(),
        };
        assert_eq!(Config::decode(&config.encode().unwrap()), Ok(config));
        assert!(Config::decode("{}").is_err());
        assert!(Config::decode("not json").is_err());
    }

    /// The guard reads every line that an anchor writes, and refuses a line that names no known kind.
    #[test]
    fn every_line_survives_the_connection() {
        let me = ProcessIdentity {
            pid: std::process::id(),
            start_time: 7,
        };
        let lines = [
            Line::Anchor(Report {
                anchor: me,
                group: me.pid,
                leader: ProcessIdentity {
                    pid: me.pid + 1,
                    start_time: me.start_time + 1,
                },
                binary: "/target/candidate/botster-conformance-probe".into(),
            }),
            Line::Term,
            Line::Kill,
            Line::Refused {
                reason: "moved".into(),
            },
            Line::Error {
                stage: "wrap".into(),
                error: "no guard".into(),
            },
        ];
        for line in lines {
            let text = line.encode();
            assert!(!text.contains('\n'), "one line: {text}");
            assert_eq!(Line::decode(&text), Ok(line));
        }
        assert!(Line::decode(r#"{"kind":"other"}"#).is_err());
        assert!(Line::decode(r#"{"kind":"anchor","group":1}"#).is_err());
    }
}
