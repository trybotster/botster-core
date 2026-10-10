//! The slow test of `triple`: only the slow tier runs it (its binary is named `slow_*`), with the `slow` feature.

#[cfg(feature = "slow")]
#[test]
fn three_tripled_is_nine() {
    assert_eq!(twostage::triple(3), 9);
}
