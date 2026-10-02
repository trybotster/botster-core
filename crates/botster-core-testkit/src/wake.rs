//! The in-memory `Wake` edge (plan 2.3): a level flag. A spurious wake is a choice point of the scheduler.
//!
//! The shuttle variant of A5-4 is not part of this edge yet.

use crate::scheduler::SchedulerHandle;
use botster_core_edges::Wake;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The "runnable work exists" flag. Clones share the flag, so the `WakeHandle` side and the engine side see one level.
///
/// Clause: Core TM-6, Core TH-2.
#[derive(Debug, Clone, Default)]
pub struct SimWake {
    flag: Arc<AtomicBool>,
}

impl SimWake {
    pub fn new() -> SimWake {
        SimWake::default()
    }

    /// True while the flag is set. A waiter returns when this is true.
    pub fn is_signaled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Sets the flag with no work behind it when the scheduler chooses a spurious wake. Returns whether it did.
    ///
    /// Clause: Core A5-2 ("spurious wakes"), Core OU-6, Core TH-2.
    pub fn maybe_wake_spuriously(&self, scheduler: &SchedulerHandle) -> bool {
        let spurious = scheduler.with(|s| s.spurious_wake());
        if spurious {
            self.signal();
        }
        spurious
    }
}

impl Wake for SimWake {
    fn signal(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    fn drain(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_is_a_level_that_clones_share() {
        let wake = SimWake::new();
        let other = wake.clone();
        assert!(!wake.is_signaled());
        other.signal();
        other.signal();
        assert!(wake.is_signaled());
        wake.drain();
        assert!(!other.is_signaled());
    }

    /// A5-2: a seed decides each spurious wake. Both outcomes occur, and the same seed repeats them.
    #[test]
    fn a_seed_decides_the_spurious_wakes() {
        let run = |seed| {
            let scheduler = SchedulerHandle::with_seed(seed);
            let wake = SimWake::new();
            (0..32)
                .map(|_| {
                    wake.drain();
                    let spurious = wake.maybe_wake_spuriously(&scheduler);
                    assert_eq!(wake.is_signaled(), spurious);
                    spurious
                })
                .collect::<Vec<_>>()
        };
        let first = run(2);
        assert!(first.contains(&true) && first.contains(&false));
        assert_eq!(first, run(2));
    }
}
