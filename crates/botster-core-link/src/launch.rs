//! How a worker is started and finds its host (plan section 3, Core AD-6, DP-8).
//!
//! The host starts `botster-worker` with fixed arguments and an exact environment, and the worker connects to the host's
//! control socket and says hello. Both ends use these functions, so the command line has one definition.
//!
//! The token is in the environment, never in an argument: other users can read arguments, and only this user can read the
//! environment of its own process (AD-6).
//!
//! Clause: Core AD-6, Core DP-8, Core AD-7 (the worker exits by itself when no host attaches within `startup`).

use crate::proof::TOKEN_LEN;
use botster_core_contract::prelude::InstanceId;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// The environment variable that holds the per-worker token as 64 lowercase hex digits.
pub const TOKEN_VAR: &str = "BOTSTER_WORKER_TOKEN";

/// What a worker is told at its start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerLaunch {
    /// The control socket of the host that started the worker.
    pub control: PathBuf,
    pub instance: InstanceId,
    /// The host epoch that the worker obeys at the start (DP-8).
    pub host_epoch: u64,
    pub token: [u8; TOKEN_LEN],
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; TOKEN_LEN]> {
    let digits = text.as_bytes();
    if digits.len() != TOKEN_LEN * 2 {
        return None;
    }
    let nibble = |d: u8| match d {
        b'0'..=b'9' => Some(d - b'0'),
        b'a'..=b'f' => Some(d - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; TOKEN_LEN];
    for (byte, pair) in out.iter_mut().zip(digits.chunks(2)) {
        *byte = nibble(pair[0])? * 16 + nibble(pair[1])?;
    }
    Some(out)
}

impl WorkerLaunch {
    /// The arguments after the program name.
    pub fn args(&self) -> Vec<OsString> {
        vec![
            "--role".into(),
            "session".into(),
            "--control".into(),
            self.control.clone().into_os_string(),
            "--instance".into(),
            self.instance.0.clone().into(),
            "--epoch".into(),
            self.host_epoch.to_string().into(),
        ]
    }

    /// The whole environment of the worker: exactly the token (AD-6, A2-1: nothing is inherited).
    pub fn env(&self) -> Vec<(OsString, OsString)> {
        vec![(TOKEN_VAR.into(), hex(&self.token).into())]
    }

    /// Reads a launch from the arguments after the program name and the value of [`TOKEN_VAR`].
    pub fn parse<S: AsRef<OsStr>>(
        args: &[S],
        token: Option<&str>,
    ) -> Result<WorkerLaunch, LaunchError> {
        let mut control = None;
        let mut instance = None;
        let mut epoch = None;
        let mut role_seen = false;
        let mut it = args.iter().map(AsRef::as_ref);
        while let Some(flag) = it.next() {
            let value = it.next().ok_or(LaunchError::MissingValue)?;
            match flag.to_str() {
                Some("--role") => {
                    if value != "session" {
                        return Err(LaunchError::UnknownRole);
                    }
                    role_seen = true;
                }
                Some("--control") => control = Some(PathBuf::from(value)),
                Some("--instance") => instance = value.to_str().map(|s| InstanceId(s.to_string())),
                Some("--epoch") => epoch = value.to_str().and_then(|s| s.parse::<u64>().ok()),
                _ => return Err(LaunchError::UnknownFlag),
            }
        }
        if !role_seen {
            return Err(LaunchError::UnknownRole);
        }
        Ok(WorkerLaunch {
            control: control.ok_or(LaunchError::Missing("--control"))?,
            instance: instance.ok_or(LaunchError::Missing("--instance"))?,
            host_epoch: epoch.ok_or(LaunchError::Missing("--epoch"))?,
            token: token.and_then(unhex).ok_or(LaunchError::BadToken)?,
        })
    }
}

/// Why a worker's command line is not a launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    MissingValue,
    UnknownFlag,
    UnknownRole,
    Missing(&'static str),
    BadToken,
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::MissingValue => f.write_str("a flag has no value"),
            LaunchError::UnknownFlag => f.write_str("an unknown flag"),
            LaunchError::UnknownRole => f.write_str("the role is not `session`"),
            LaunchError::Missing(flag) => write!(f, "{flag} is missing"),
            LaunchError::BadToken => f.write_str("the token is missing or is not 64 hex digits"),
        }
    }
}

impl std::error::Error for LaunchError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch() -> WorkerLaunch {
        WorkerLaunch {
            control: "/data/c".into(),
            instance: InstanceId("7-1".into()),
            host_epoch: 7,
            token: [0xAB; TOKEN_LEN],
        }
    }

    /// AD-6: the command line round-trips, and the token travels in the environment only.
    #[test]
    fn a_launch_round_trips_and_the_token_is_not_an_argument() {
        let l = launch();
        let args = l.args();
        let text = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!text.contains("abab"), "{text}");
        let env = l.env();
        assert_eq!(env.len(), 1);
        let value = env[0].1.to_string_lossy().into_owned();
        assert_eq!(WorkerLaunch::parse(&args, Some(&value)), Ok(l));
    }

    #[test]
    fn a_launch_with_a_missing_part_is_refused() {
        let l = launch();
        let args = l.args();
        let token = hex(&l.token);
        assert_eq!(WorkerLaunch::parse(&args, None), Err(LaunchError::BadToken));
        assert_eq!(
            WorkerLaunch::parse(&args, Some("zz")),
            Err(LaunchError::BadToken)
        );
        assert_eq!(
            WorkerLaunch::parse(&args[..6], Some(&token)),
            Err(LaunchError::Missing("--epoch"))
        );
        assert_eq!(
            WorkerLaunch::parse(&args[2..], Some(&token)),
            Err(LaunchError::UnknownRole)
        );
        assert_eq!(
            WorkerLaunch::parse(&["--role", "guardian"], Some(&token)),
            Err(LaunchError::UnknownRole)
        );
        assert_eq!(
            WorkerLaunch::parse(&["--x", "1"], Some(&token)),
            Err(LaunchError::UnknownFlag)
        );
        assert_eq!(
            WorkerLaunch::parse(&["--role"], Some(&token)),
            Err(LaunchError::MissingValue)
        );
    }

    /// AD-6: a token of every hex digit decodes to its bytes, and a non-digit or a short text does not.
    #[test]
    fn unhex_reads_every_digit() {
        let text: String = (0..TOKEN_LEN)
            .map(|i| format!("{:02x}", (i * 7 + 9) % 256))
            .collect();
        let bytes = unhex(&text).expect("a token");
        for (i, b) in bytes.iter().enumerate() {
            assert_eq!(usize::from(*b), (i * 7 + 9) % 256);
        }
        assert_eq!(unhex(&"09".repeat(TOKEN_LEN)).unwrap(), [9u8; TOKEN_LEN]);
        assert_eq!(unhex(&"90".repeat(TOKEN_LEN)).unwrap(), [0x90u8; TOKEN_LEN]);
        assert_eq!(unhex(&"af".repeat(TOKEN_LEN)).unwrap(), [0xAFu8; TOKEN_LEN]);
        assert!(unhex(&"0g".repeat(TOKEN_LEN)).is_none());
        assert!(unhex("00").is_none());
    }

    /// Every launch error has its own words.
    #[test]
    fn launch_errors_say_what_is_wrong() {
        assert_eq!(LaunchError::MissingValue.to_string(), "a flag has no value");
        assert_eq!(LaunchError::UnknownFlag.to_string(), "an unknown flag");
        assert_eq!(
            LaunchError::UnknownRole.to_string(),
            "the role is not `session`"
        );
        assert_eq!(
            LaunchError::Missing("--epoch").to_string(),
            "--epoch is missing"
        );
        assert_eq!(
            LaunchError::BadToken.to_string(),
            "the token is missing or is not 64 hex digits"
        );
    }
}
