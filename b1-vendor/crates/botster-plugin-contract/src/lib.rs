//! Hub-to-plugin contract types: frames, results, limits, the call list and the manifest (HP J4e with A1 and A2; ledger prefix `hp`).
//!
//! Clause: HP section 19 (the contract crate), Appendix A, Appendix B, Appendix C, A1 PK-1 to PK-9, A2.
//!
//! This crate holds types, static tables and the framing codec. It holds no Hub or host behavior. Every wire type derives `JsonSchema`, and
//! `cargo xtask codegen` writes the artifacts of `artifacts` to `schemas/plugin/`. The UI view document comes from `botster-ui-contract`.
//! Doc comments cite clause ids; the contract text is in `frozen/final7`.

// cargo-mutants finds a source file only through a plain `mod` item, and it does not expand macros. So the private declarations
// are plain items, one for each file, and the `unstable` declarations come from a macro. A plain item in both branches would
// make cargo-mutants list each mutant twice.
#[cfg(feature = "unstable")]
macro_rules! unstable_modules {
    ($($name:ident),* $(,)?) => {
        $(
            pub mod $name;
        )*
    };
}

#[cfg(feature = "unstable")]
unstable_modules!(
    artifacts, call_types, calls, common, entities, errors, events, files, frames, globals,
    handlers, limits, manifest, operations, result, rows, schema, wire,
);
#[cfg(not(feature = "unstable"))]
mod artifacts;
#[cfg(not(feature = "unstable"))]
mod call_types;
#[cfg(not(feature = "unstable"))]
mod calls;
#[cfg(not(feature = "unstable"))]
mod common;
#[cfg(not(feature = "unstable"))]
mod entities;
#[cfg(not(feature = "unstable"))]
mod errors;
#[cfg(not(feature = "unstable"))]
mod events;
#[cfg(not(feature = "unstable"))]
mod files;
#[cfg(not(feature = "unstable"))]
mod frames;
#[cfg(not(feature = "unstable"))]
mod globals;
#[cfg(not(feature = "unstable"))]
mod handlers;
#[cfg(not(feature = "unstable"))]
mod limits;
#[cfg(not(feature = "unstable"))]
mod manifest;
#[cfg(not(feature = "unstable"))]
mod operations;
#[cfg(not(feature = "unstable"))]
mod result;
#[cfg(not(feature = "unstable"))]
mod rows;
#[cfg(not(feature = "unstable"))]
mod schema;
#[cfg(not(feature = "unstable"))]
mod wire;

#[cfg(test)]
mod props;

/// The public facade.
pub mod prelude {
    pub use crate::artifacts::{all as artifact_files, artifact_id};
    pub use crate::call_types::*;
    pub use crate::calls::{
        allowed_in, find as find_call, CallSpec, Ctx, Failure, CALLS, COMMON_FAILURES,
        LUA_LOCAL_NAMES, LUA_LOG_SUGAR, LUA_REGISTRATION_NAMES,
    };
    pub use crate::common::{
        ConstTrue, Effect, Epoch, OpenObject, ResourceRef, SessionRef, WireId, WireU64,
        MAX_WIRE_INT,
    };
    pub use crate::entities::*;
    pub use crate::errors::{
        registry_json as error_registry, CodeSpec, CODES, FAULT_CODES, INGRESS_STATUSES,
        KIND_MEANINGS, LOAD_FAILURE_CODES, NOT_WRITTEN_REASONS, WRITE_RESULT_REASONS,
        WRONG_STATE_REASONS,
    };
    pub use crate::events::*;
    pub use crate::files::*;
    pub use crate::frames::*;
    pub use crate::globals::{
        artifact_json as lua_host_globals, botster_names, globals as lua_globals,
        UNAVAILABLE as LUA_UNAVAILABLE,
    };
    pub use crate::handlers::{check_descriptor, HandlerSpec, HANDLERS};
    pub use crate::limits::{
        KeyMode, LaneBound, LaneLimits, LimitSpec, LimitViolation, PluginLimits, Range,
        LIMIT_SPECS, TRUSTED_HOSTS_KEY,
    };
    pub use crate::manifest::*;
    pub use crate::operations::{
        OperationFields, OperationRecord, Residue, StateTable, Truncated, OPERATION_KINDS,
        REQUIRED_FIELDS, SAFE_WRITE_ABANDONED_REASONS, SAFE_WRITE_CANCELLED_REASONS,
        SAFE_WRITE_FAILED_REASONS, SAFE_WRITE_PROBE, SAFE_WRITE_UNKNOWN_REASONS,
        SPAWN_FAILED_REASONS, STATE_TABLES,
    };
    pub use crate::result::{
        Detail, ErrorBody, ErrorKind, FoundProvider, PluginResult, ResolutionFailure,
    };
    pub use crate::rows::{SessionRow, SessionTypeRow, SpawnTargetRow, WorktreeRow};
    pub use crate::wire::{
        decode, encode, encode_raw, Decoded, Direction, Disposition, EncodeError, Frame,
        FrameClass, FrameSpec, FrameType, Lane, WireError, BULK_CALL, FRAMES,
    };
}
