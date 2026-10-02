//! Botster Core: the facade. It re-exports the contract and holds the `Core` handle.
//!
//! The other crates of the workspace are published with the same version, but they are not part of the API: they carry no
//! compatibility promise (plan 2.6). A host imports this crate only.

use std::cell::Cell;
use std::marker::PhantomData;

/// The Core contract crate, whole.
pub use botster_core_contract as contract;

/// The names that a user of Core needs: the prelude of the contract.
pub mod prelude {
    pub use botster_core_contract::prelude::*;
}

/// The Core handle. It is `Send` and not `Sync`: one thread at a time owns it, and it can move between threads.
///
/// P0 holds the shell only. P1 gives it `open` and `impl CoreApi`.
///
/// Clause: Core TH-1.
pub struct Core {
    /// `Cell` is `Send` and not `Sync`, so the handle has the same two properties.
    _not_sync: PhantomData<Cell<()>>,
}
