//! The one way to signal a process group or a process (the pattern rule). Every signal target comes from a record (a
//! registry row, a spawn, a guard), and a record can be wrong: a corrupted row (A10-2) or a defect can give any number.
//!
//! - `kill(-1, …)` signals every process that the user may signal, and `kill(1, …)` signals init. A group of 1 is the same
//!   `kill(-1, …)`. A target of 0 is our own group.
//! - So [`signal_group`] and [`signal_process`] refuse a target of 0 or 1, a target above the pid range, and our own group
//!   or our own process, with a typed error. Only an allowed target reaches the operating system.
//! - A guard that ends its own group on purpose uses [`signal_own_group`]. It takes no target: no record can give one. It
//!   still refuses when our own group is 0 or 1, because then the whole system's first group is ours.
//!
//! `clippy.toml` bans `rustix::process::kill_process_group`, `rustix::process::kill_process` and
//! `rustix::process::kill_current_process_group` in every crate except here.

pub use rustix::process::Signal;
use rustix::process::{getpgrp, Pid};
use std::io;

/// Why a signal was not sent.
#[derive(Debug)]
pub enum SignalError {
    /// The target is 0, 1 or above the pid range: no record names such a process.
    NotATarget(u32),
    /// The target is our own process group, or our own process.
    Own(u32),
    /// The operating system did not send the signal. `ESRCH`: no such process is left.
    Os(io::Error),
}

/// An operating-system error stays itself; a refusal is `InvalidInput`.
impl From<SignalError> for io::Error {
    fn from(error: SignalError) -> io::Error {
        match error {
            SignalError::Os(error) => error,
            refused => io::Error::new(io::ErrorKind::InvalidInput, format!("{refused:?}")),
        }
    }
}

/// The pid of `raw`, when a signal may go to it. `own` is our own group id or our own pid.
///
/// # Errors
/// [`SignalError::NotATarget`] or [`SignalError::Own`].
pub fn target(raw: u32, own: u32) -> Result<Pid, SignalError> {
    if raw == own {
        return Err(SignalError::Own(raw));
    }
    i32::try_from(raw)
        .ok()
        .filter(|&pid| pid > 1)
        .and_then(Pid::from_raw)
        .ok_or(SignalError::NotATarget(raw))
}

/// Sends `signal` to the process group `pgid`, unless [`target`] refuses it.
///
/// # Errors
/// The refusal of [`target`] against our own group, or the operating-system error.
pub fn signal_group(pgid: u32, signal: Signal) -> Result<(), SignalError> {
    let pgid = target(pgid, getpgrp().as_raw_nonzero().get().unsigned_abs())?;
    #[allow(clippy::disallowed_methods)]
    // The one group signal: the target passed the refusal above.
    rustix::process::kill_process_group(pgid, signal).map_err(|error| SignalError::Os(error.into()))
}

/// Sends `signal` to the process `pid` alone, unless [`target`] refuses it.
///
/// # Errors
/// The refusal of [`target`] against our own pid, or the operating-system error.
pub fn signal_process(pid: u32, signal: Signal) -> Result<(), SignalError> {
    let pid = target(pid, std::process::id())?;
    #[allow(clippy::disallowed_methods)]
    // The one process signal: the target passed the refusal above.
    rustix::process::kill_process(pid, signal).map_err(|error| SignalError::Os(error.into()))
}

/// Our own process group, when [`signal_own_group`] may signal it: never group 0 or 1.
///
/// # Errors
/// [`SignalError::NotATarget`].
pub fn own_group(own: u32) -> Result<(), SignalError> {
    if own <= 1 {
        return Err(SignalError::NotATarget(own));
    }
    Ok(())
}

/// Sends `signal` to our own process group, this process included, unless [`own_group`] refuses it. Only a guard that
/// ends its own group uses it.
///
/// # Errors
/// The refusal of [`own_group`], or the operating-system error.
pub fn signal_own_group(signal: Signal) -> Result<(), SignalError> {
    own_group(getpgrp().as_raw_nonzero().get().unsigned_abs())?;
    #[allow(clippy::disallowed_methods)]
    // The one signal to our own group: the group passed the refusal above.
    rustix::process::kill_current_process_group(signal)
        .map_err(|error| SignalError::Os(error.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pattern rule: 0, 1, a number above the pid range and our own group or pid are never a target; every other pid
    /// is, unchanged.
    #[test]
    fn a_target_of_0_1_out_of_range_or_our_own_is_refused() {
        for raw in [0, 1, u32::try_from(i32::MAX).unwrap() + 1, u32::MAX] {
            assert!(
                matches!(target(raw, 500), Err(SignalError::NotATarget(r)) if r == raw),
                "{raw}"
            );
        }
        assert!(matches!(target(500, 500), Err(SignalError::Own(500))));
        assert!(matches!(target(1, 1), Err(SignalError::Own(1))));
        for raw in [2, 499, 501, u32::try_from(i32::MAX).unwrap()] {
            assert_eq!(
                target(raw, 500)
                    .unwrap()
                    .as_raw_nonzero()
                    .get()
                    .unsigned_abs(),
                raw
            );
        }
    }

    /// The pattern rule on the real calls: our own group and our own process are refused before any signal. `SIGCONT` is the
    /// signal, so a defect in the refusal resumes this process and stops nothing.
    #[test]
    fn our_own_group_and_process_are_never_signalled() {
        let own_group = getpgrp().as_raw_nonzero().get().unsigned_abs();
        assert!(matches!(
            signal_group(own_group, Signal::CONT),
            Err(SignalError::Own(g)) if g == own_group
        ));
        let me = std::process::id();
        assert!(matches!(
            signal_process(me, Signal::CONT),
            Err(SignalError::Own(p)) if p == me
        ));
        // An allowed target reaches the operating system: no process or group has the highest pid (`ESRCH`).
        let none = u32::try_from(i32::MAX).unwrap();
        for sent in [
            signal_group(none, Signal::CONT),
            signal_process(none, Signal::CONT),
        ] {
            assert!(
                matches!(&sent, Err(SignalError::Os(e)) if e.raw_os_error() == Some(rustix::io::Errno::SRCH.raw_os_error())),
                "{sent:?}"
            );
        }
    }

    /// The pattern rule for our own group: group 0 or 1 is never signalled; any other group of ours is.
    #[test]
    fn our_own_group_of_0_or_1_is_refused() {
        for own in [0, 1] {
            assert!(matches!(own_group(own), Err(SignalError::NotATarget(g)) if g == own));
        }
        for own in [2, 500, u32::MAX] {
            assert!(own_group(own).is_ok(), "{own}");
        }
    }

    /// The real call reaches our own group, this process included. `SIGURG` is the signal: its default action is to ignore
    /// it, so no other member of the group is affected. The handler of this process writes one byte to a socket, and the
    /// test waits for that byte (the event), not for a flag.
    #[test]
    fn a_signal_to_our_own_group_reaches_this_process() {
        use std::io::Read;
        use std::time::Duration;
        let (mut reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let handler =
            signal_hook::low_level::pipe::register(signal_hook::consts::SIGURG, writer).unwrap();
        // timer: deadline — bounds the wait for the handler's byte; the signal is sent before the wait starts.
        let deadline = Some(Duration::from_secs(10));
        reader.set_read_timeout(deadline).unwrap();
        signal_own_group(Signal::URG).unwrap();
        let mut byte = [0];
        let read = reader.read_exact(&mut byte);
        signal_hook::low_level::unregister(handler);
        read.expect("the signal reached the handler of this process");
    }

    /// A caller of `io::Result` keeps the errno of the operating system and sees a refusal as invalid input.
    #[test]
    fn an_os_error_keeps_its_errno_and_a_refusal_is_invalid_input() {
        let srch = rustix::io::Errno::SRCH.raw_os_error();
        let os = io::Error::from(SignalError::Os(io::Error::from_raw_os_error(srch)));
        assert_eq!(os.raw_os_error(), Some(srch));
        for refused in [SignalError::NotATarget(1), SignalError::Own(7)] {
            let text = format!("{refused:?}");
            let error = io::Error::from(refused);
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert_eq!(error.to_string(), text);
        }
    }
}
