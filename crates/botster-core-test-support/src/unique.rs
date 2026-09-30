//! Names that stay unique between tests that run in parallel threads.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A suffix for a temporary path: the clock in nanoseconds, then a counter
/// that no other call in this process shares. The clock alone is not enough:
/// on macOS it ticks in microseconds, so two test threads can read the same
/// value and then share one directory and one socket path.
#[must_use]
pub fn stamp() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is after the unix epoch")
        .as_nanos();
    format!("{nanos}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::stamp;
    use std::collections::HashSet;

    /// Threads that ask in the same instant still receive different names.
    #[test]
    fn stamps_from_parallel_threads_never_repeat() {
        let threads: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..2_000).map(|_| stamp()).collect::<Vec<_>>()))
            .collect();
        let mut seen = HashSet::new();
        for thread in threads {
            for name in thread.join().expect("a stamp thread") {
                assert!(seen.insert(name.clone()), "stamp {name} was issued twice");
            }
        }
    }
}
