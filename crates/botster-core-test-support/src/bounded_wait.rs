//! One shared bounded wait for tests.
//!
//! It holds the only test timer: a deadline whose expiry fails the test. Each
//! step blocks on a real event — a wake source, a channel, a condvar, a
//! process exit — for at most the time that is left. Nothing sleeps or polls.

use std::time::{Duration, Instant};

/// Call `step` with the time left until it returns `Some`.
///
/// `step` must block on an event for at most the duration it receives, and
/// return `None` when that wait ended without the awaited result. When the
/// deadline passes first, this panics naming `what`.
pub fn wait_for<T>(what: &str, bound: Duration, mut step: impl FnMut(Duration) -> Option<T>) -> T {
    // timer: deadline — the awaited event must arrive; expiry fails the test
    let deadline = Instant::now() + bound;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if let Some(value) = step(remaining) {
            return value;
        }
        assert!(
            !remaining.is_zero(),
            "{what}: the event did not arrive within {bound:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::wait_for;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn returns_the_value_once_the_event_arrives() {
        let (sender, receiver) = mpsc::channel();
        let producer = std::thread::spawn(move || sender.send(7).expect("send"));
        let value = wait_for("channel value", Duration::from_secs(5), |remaining| {
            receiver.recv_timeout(remaining).ok()
        });
        assert_eq!(value, 7);
        producer.join().expect("producer");
    }

    #[test]
    #[should_panic(expected = "never: the event did not arrive")]
    fn fails_at_the_deadline_without_the_event() {
        let (_sender, receiver) = mpsc::channel::<()>();
        wait_for("never", Duration::from_millis(10), |remaining| {
            receiver.recv_timeout(remaining).ok()
        });
    }
}
