//! The fixture of the two-stage mutation verdict (plan 23n). `double` is compiled always, and a default test catches each
//! of its mutants. `triple` is compiled only with the `slow` feature: the default stage lists its mutants and misses each
//! one; the slow stage builds the feature, and the slow test `tests/slow_triple.rs` catches them.

/// Two times `n`.
pub fn double(n: u32) -> u32 {
    n * 2
}

/// Three times `n`, with the `slow` feature only.
#[cfg(feature = "slow")]
pub fn triple(n: u32) -> u32 {
    n * 3
}

#[cfg(test)]
mod tests {
    #[test]
    fn three_doubled_is_six() {
        assert_eq!(super::double(3), 6);
    }
}
