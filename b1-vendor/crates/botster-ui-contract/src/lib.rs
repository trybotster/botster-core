//! The UI contract: the view document, `UiNode`, admission and the reference `realize`.
//!
//! Clause: ui-contract 1.0 (all sections), with the clauses of 1.1, 1.2, 1.3 and 1.4 (see `frozen/final7`).
//!
//! This crate holds types and one piece of product logic, the reference `realize` (UI-RZ-3). The vectors in
//! `vectors/` are the conformance transcripts of the `realizer` subject (UI-RZ-3b).

#[cfg(feature = "unstable")]
pub mod admission;
#[cfg(not(feature = "unstable"))]
mod admission;
#[cfg(feature = "unstable")]
pub mod doc;
#[cfg(not(feature = "unstable"))]
mod doc;
#[cfg(feature = "unstable")]
pub mod ids;
#[cfg(not(feature = "unstable"))]
mod ids;
#[cfg(feature = "unstable")]
pub mod input;
#[cfg(not(feature = "unstable"))]
mod input;
#[cfg(feature = "unstable")]
pub mod links;
#[cfg(not(feature = "unstable"))]
mod links;
#[cfg(feature = "unstable")]
pub mod navigation;
#[cfg(not(feature = "unstable"))]
mod navigation;
#[cfg(feature = "unstable")]
pub mod node;
#[cfg(not(feature = "unstable"))]
mod node;
#[cfg(feature = "unstable")]
pub mod realize;
#[cfg(not(feature = "unstable"))]
mod realize;
#[cfg(feature = "unstable")]
pub mod value;
#[cfg(not(feature = "unstable"))]
mod value;
#[cfg(feature = "unstable")]
pub mod vocabulary;
#[cfg(not(feature = "unstable"))]
mod vocabulary;

#[cfg(feature = "unstable")]
pub mod schemas;
#[cfg(not(feature = "unstable"))]
mod schemas;
#[cfg(feature = "unstable")]
pub mod vectors;
#[cfg(not(feature = "unstable"))]
mod vectors;

/// The public facade.
pub mod prelude {
    pub use crate::admission::{
        admit, admit_badge, admit_text, check_stale, AdmissionCode, AdmissionContext,
        AdmissionFailure, HandlerDescriptor, KindDescriptor,
    };
    pub use crate::doc::*;
    pub use crate::ids::{
        child_id, draft_key, field_error_target, is_author_id, is_role_word, row_key, split_id,
        ROLE_WORDS,
    };
    pub use crate::input::*;
    pub use crate::links::{
        is_view_route, parse_route, percent_decode, percent_encode, route_path, web_url,
    };
    pub use crate::navigation::{
        home_layout, navigation_badge, HomeArea, HomeEntry, RealizedBadge,
    };
    pub use crate::node::{
        CopyKey, Diagnostic, LocalChanges, MappingRow, RealizedTree, StampedError, Text, UiNode,
        UiNodeKind, ABSENT_REASONS, CLAUSE_PROPS, COPY_KEYS, COPY_KEY_ARGS, DIAGNOSTIC_CODES,
        LOCAL_ERROR_CODES, MAPPING,
    };
    pub use crate::realize::{diagnostic_tree, realize};
    /// The (file name, text) pairs of the generated schemas under `schemas/ui/` (for `cargo xtask codegen`).
    pub use crate::schemas::all as schema_files;
    /// The embedded vector files and the runner of one file (UI-RZ-3, UI-RZ-3b).
    pub use crate::vectors::{all as vector_files, run_file as run_vector_file};
    pub use crate::vocabulary::{node_schema, UiNodeSchema};
}
