//! The fixture of the flake guard (BUILD.md Testing rule 9; lead, 2026-10-11). A default test catches each mutant of
//! `triple`. The only test of `half` fails once under a mutant of `half` and passes on its retry (a marker file), so in
//! the `mutants` profile each mutant of `half` is FLAKY in that test and counts as MISSED, not as a catch.

/// Three times `n`.
pub fn triple(n: u32) -> u32 {
    n * 3
}

/// Half of `n`.
pub fn half(n: u32) -> u32 {
    n / 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn two_tripled_is_six() {
        assert_eq!(super::triple(2), 6);
    }

    /// Unmutated, it passes at once. Under a mutant of `half`, the first attempt writes the marker and fails; the retry
    /// finds the marker, removes it, and passes, so the next mutant starts without it.
    #[test]
    fn half_fails_once_under_a_mutant() {
        if super::half(4) == 2 {
            return;
        }
        let marker = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("flaky-marker");
        if marker.exists() {
            std::fs::remove_file(&marker).unwrap();
        } else {
            std::fs::write(&marker, "").unwrap();
            panic!("half(4) is not 2");
        }
    }
}
