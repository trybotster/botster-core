//! The fixture's conformance binary: a selected slow test, and an ignored test that passes the check only while its id
//! is pending-real.
#![cfg(feature = "slow")]

#[test]
fn selected() {
    assert_eq!(botster_core::one(), 1);
}

mod conf {
    #[test]
    #[ignore]
    fn pending_id() {}
}
