//! The fixture of the two-stage mutation verdict (plan 23n). `double` is compiled always, and a default test catches each
//! of its mutants. `triple` is compiled only with the `slow` feature: the default stage lists its mutants and misses each
//! one; the slow stage builds the feature, and the slow test `tests/slow_triple.rs` catches them. `wide` has a struct-field
//! deletion mutant, which cargo-mutants 27.1.0 does not filter by name (`-F`): a default test catches it, so stage 2 must
//! not test it.

/// Two times `n`.
pub fn double(n: u32) -> u32 {
    n * 2
}

/// A size.
#[derive(Debug, Default, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// A size `width` wide, with the default height.
pub fn wide(width: u32) -> Size {
    Size {
        width,
        ..Default::default()
    }
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

    #[test]
    fn wide_keeps_its_width() {
        assert_eq!(
            super::wide(3),
            super::Size {
                width: 3,
                height: 0
            }
        );
    }
}
