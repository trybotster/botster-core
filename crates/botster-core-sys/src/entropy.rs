//! The real `Entropy` edge: the OS CSPRNG and nothing else (plan 2.3a).
//!
//! Clause: Core AD-6 (the per-worker token), Core SV-1 (the service secret).

use botster_core_edges::Entropy;

/// Random bytes from the operating system. A failure of the OS generator is fatal: a token that cannot be drawn must never
/// be replaced by a weaker one.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsEntropy;

impl Entropy for OsEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        getrandom::fill(buf).expect("the OS random generator failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_draws_differ_and_fill_the_buffer() {
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        OsEntropy.fill(&mut a);
        OsEntropy.fill(&mut b);
        assert_ne!(a, b);
        assert_ne!(a, [0u8; 32]);
    }
}
