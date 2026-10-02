//! The seeded `Entropy` edge (plan 2.3a). It exists only in the testkit: production reads the OS CSPRNG.

use botster_core_edges::Entropy;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

/// The stream number of the entropy stream. The scheduler uses stream 0, so the two streams of one seed never overlap.
const ENTROPY_STREAM: u64 = 1;

/// Random values from a ChaCha8 stream that is seeded from the test seed and is separate from the scheduler's stream, so a
/// draw of a token never moves a scheduling choice (plan 2.3a).
///
/// Clause: Core AD-6, Core SV-1, Core ID-1.
#[derive(Debug, Clone)]
pub struct SeededEntropy {
    rng: ChaCha8Rng,
}

impl SeededEntropy {
    pub fn with_seed(seed: u64) -> SeededEntropy {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(ENTROPY_STREAM);
        rng.set_word_pos(0);
        SeededEntropy { rng }
    }
}

impl Entropy for SeededEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        self.rng.fill_bytes(buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::SeededScheduler;

    fn bytes(seed: u64) -> [u8; 32] {
        let mut out = [0u8; 32];
        SeededEntropy::with_seed(seed).fill(&mut out);
        out
    }

    #[test]
    fn a_seed_fixes_the_bytes() {
        assert_eq!(bytes(4), bytes(4));
        assert_ne!(bytes(4), bytes(5));
    }

    /// Plan 2.3a: the entropy stream is separate from the scheduler stream. The same seed gives different words, and a draw
    /// from one never changes the other.
    #[test]
    fn the_stream_is_separate_from_the_scheduler() {
        let mut sched = SeededScheduler::with_seed(9);
        let alone: Vec<usize> = (0..16).map(|_| sched.ready_work(1 << 20)).collect();

        let mut sched = SeededScheduler::with_seed(9);
        let mut entropy = SeededEntropy::with_seed(9);
        let mixed: Vec<usize> = (0..16)
            .map(|_| {
                entropy.fill(&mut [0u8; 8]);
                sched.ready_work(1 << 20)
            })
            .collect();
        assert_eq!(alone, mixed);

        let first = u64::from_le_bytes(bytes(9)[..8].try_into().expect("8 bytes"));
        let mut rng = ChaCha8Rng::seed_from_u64(9);
        assert_ne!(first, rng.next_u64());
    }
}
