//! The fixture of `slow_coverage::slow_tests`: a library test that only the `slow` feature builds.

pub fn one() -> u8 {
    1
}

#[test]
fn one_is_one() {
    assert_eq!(one(), 1);
}

// The test renames this module `slow_tests`: then the slow filter selects its test.
#[cfg(feature = "slow")]
mod tests {
    #[test]
    fn runs_only_under_slow() {
        assert_eq!(super::one(), 1);
    }
}
