//! The fixture of the platform-only exclusions. `double` is compiled on every OS, and its test catches each of its
//! mutants. `triple` is compiled only on Windows, where no gate runs: cargo-mutants still lists its mutants, and each one
//! is MISSED unless the run excludes the code that its OS does not compile.

/// Two times `n`.
pub fn double(n: u32) -> u32 {
    n * 2
}

/// Three times `n`, on Windows only.
#[cfg(windows)]
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
