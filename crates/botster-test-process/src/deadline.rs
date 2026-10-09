//! The only timer of this crate: a deadline that a wait blocks against (BUILD.md testing rule 5). Every wait of the crate
//! blocks on a real event (an exit, a byte, an end of file) for at most the time that is left, and fails with the limit
//! that it was derived from. Nothing sleeps and nothing polls.

use std::time::{Duration, Instant};

/// The cleanup bound of the guards: the time that an owner gives a group, a child or a reader to end before it reports a
/// failure. It is the existing `CLEANUP` of the shared test guards (`botster-core-sys/tests/common/guard_cleanup.rs`), moved
/// here unchanged. Not a contract value.
pub const CLEANUP: Duration = Duration::from_secs(10);

/// A point in time that a wait must not pass, with the limit that it was derived from (for the failure message).
#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    at: Instant,
    limit: Duration,
}

impl Deadline {
    /// The deadline `limit` from now.
    pub fn after(limit: Duration) -> Self {
        // timer: deadline — the one start of every bounded wait of the crate.
        let at = Instant::now() + limit;
        Self { at, limit }
    }

    /// The deadline of the cleanup bound, from now.
    pub fn cleanup() -> Self {
        Self::after(CLEANUP)
    }

    /// The time that is left; zero once the deadline has passed.
    pub fn remaining(&self) -> Duration {
        self.at.saturating_duration_since(Instant::now())
    }

    /// Whether the deadline has passed.
    pub fn expired(&self) -> bool {
        self.remaining().is_zero()
    }

    /// The limit that the deadline was derived from.
    pub fn limit(&self) -> Duration {
        self.limit
    }

    /// The time that is left, as a timeout for `poll`.
    pub(crate) fn timespec(&self) -> rustix::event::Timespec {
        let left = self.remaining();
        rustix::event::Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: left.subsec_nanos().into(),
        }
    }
}

/// The result of `wait`, a wait that a signal can interrupt (EINTR) before its timeout. `wait` is repeated after an
/// interruption only while `expired` is false; an interruption after the deadline gives `Ok(None)`, which the caller reads
/// as its timeout. Linux `poll` returns EINTR for a pending signal even with no time left, so a steady stream of signals
/// would otherwise keep the wait going past its deadline (#171 R2).
///
/// # Errors
/// The wait failed for another reason.
pub(crate) fn retry_interrupted<T>(
    mut wait: impl FnMut() -> rustix::io::Result<T>,
    expired: impl Fn() -> bool,
) -> rustix::io::Result<Option<T>> {
    loop {
        match wait() {
            Err(rustix::io::Errno::INTR) if expired() => return Ok(None),
            Err(rustix::io::Errno::INTR) => {}
            other => return other.map(Some),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #171 R2: an interrupted wait is repeated before the deadline; an interruption after it ends the wait as at its
    /// timeout, whatever the next wait would return; any other result is returned as it is.
    #[test]
    fn an_interrupted_wait_is_repeated_until_the_deadline() {
        use rustix::io::Errno;
        let run = |results: Vec<rustix::io::Result<usize>>, expired_after: usize| {
            let mut results = results.into_iter();
            let waits = std::cell::Cell::new(0);
            let checks = std::cell::Cell::new(0);
            let waited = retry_interrupted(
                || {
                    waits.set(waits.get() + 1);
                    results.next().expect("no wait after the outcome")
                },
                || {
                    checks.set(checks.get() + 1);
                    checks.get() > expired_after
                },
            );
            (waited, waits.get())
        };
        assert_eq!(
            run(vec![Err(Errno::INTR), Err(Errno::INTR), Ok(1)], 5),
            (Ok(Some(1)), 3)
        );
        assert_eq!(
            run(
                vec![Err(Errno::INTR), Err(Errno::INTR), Err(Errno::INTR), Ok(1)],
                2
            ),
            (Ok(None), 3)
        );
        assert_eq!(run(vec![Ok(0)], 0), (Ok(Some(0)), 1));
        assert_eq!(run(vec![Err(Errno::BADF)], 5), (Err(Errno::BADF), 1));
    }

    #[test]
    fn a_zero_limit_has_expired_and_keeps_its_limit_for_the_message() {
        let deadline = Deadline::after(Duration::ZERO);
        assert!(deadline.expired());
        assert_eq!(deadline.remaining(), Duration::ZERO);
        assert_eq!(deadline.limit(), Duration::ZERO);
        let ts = deadline.timespec();
        assert_eq!((ts.tv_sec, ts.tv_nsec), (0, 0));
    }

    #[test]
    fn the_cleanup_deadline_is_not_expired_and_has_time_left_within_its_limit() {
        let deadline = Deadline::cleanup();
        assert!(!deadline.expired());
        assert_eq!(deadline.limit(), CLEANUP);
        assert!(deadline.remaining() > Duration::ZERO && deadline.remaining() <= CLEANUP);
        let ts = deadline.timespec();
        assert!(ts.tv_sec > 0 && ts.tv_sec <= 10);
    }
}
