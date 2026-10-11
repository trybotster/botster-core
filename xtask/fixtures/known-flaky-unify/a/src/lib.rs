//! A library whose only test exists without the feature `x`: the workspace build has `x` (from `b`), so the workspace's
//! test list lacks the test; cargo-mutants tests the mutated package alone, without `x`, so the test runs and catches.

/// Three times `n`.
pub fn triple(n: u32) -> u32 {
    n * 3
}

#[cfg(all(test, not(feature = "x")))]
mod tests {
    #[test]
    fn guarded() {
        assert_eq!(super::triple(2), 6);
    }
}
