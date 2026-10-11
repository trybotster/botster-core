//! A member that enables the feature `x` of `a`. Its test keeps the workspace's test list not empty.

pub use a::triple;

#[cfg(test)]
mod tests {
    #[test]
    fn b_runs() {
        assert_eq!(super::triple(1), 3);
    }
}
