//! Core TH-1: the handle is `Send` and not `Sync`. The check is made at compile time.

use botster_core::Core;
use static_assertions::{assert_impl_all, assert_not_impl_any};

#[test]
fn core_is_send_and_not_sync() {
    assert_impl_all!(Core: Send);
    assert_not_impl_any!(Core: Sync);
}
