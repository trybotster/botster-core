//! The fixture of the mutation step: the mutants that make `ready(0)` false park the test thread for ever; every other
//! mutant fails the test at once.

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
}
