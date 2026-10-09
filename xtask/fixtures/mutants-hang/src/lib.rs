//! The fixture of the mutation step. Three mutants park a test thread for ever: two of them (`ready(0)` false) also fail
//! another test at once, and the first failure ends the run; the third (`delete !` in `wait`) fails no test, so only the
//! timeout of cargo-mutants ends it. Every other mutant fails a test at once.

/// Whether the work of `n` is ready.
pub fn ready(n: u32) -> bool {
    n == 0
}

/// Waits until the work of `n` is ready, then returns its result.
pub fn wait(n: u32) -> u32 {
    while !ready(n) {
        std::thread::park();
    }
    n + 7
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_work_of_zero_is_ready_at_once() {
        assert_eq!(super::wait(0), 7);
        assert!(!super::ready(1));
    }

    #[test]
    fn a_second_check_of_ready() {
        assert!(super::ready(0));
    }
}
