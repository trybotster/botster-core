//! Errors: the `Error` object, the closed code list with its categories (HC section 11), and the registry that `schemas/hub-client/errors.json` is made from.

use crate::wire::{open_enum, opt_no_null, OpenObject};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

open_enum! {
    /// The error code. The list is closed per version and additive: a client maps an unknown code to its `category` (HC section 11, EN-6).
    ///
    /// Clause: HC section 11.
    pub enum ErrorCode {
        InvalidArguments = "invalid_arguments",
        NotFound = "not_found",
        Unsupported = "unsupported",
        Conflict = "conflict",
        WrongState = "wrong_state",
        PermissionDenied = "permission_denied",
        TooManyRequests = "too_many_requests",
        Busy = "busy",
        ResponseTooLarge = "response_too_large",
        PluginFailed = "plugin_failed",
        RemoteHubUnsupported = "remote_hub_unsupported",
        HubShutdown = "hub_shutdown",
        ExpiredCursor = "expired_cursor",
        StaleDescriptor = "stale_descriptor",
        Cancelled = "cancelled",
        Internal = "internal",
    }
}

open_enum! {
    /// The category of an error. A client maps an unknown category to `fault` (HC section 11).
    ///
    /// Clause: HC section 11.
    pub enum Category {
        Caller = "caller",
        State = "state",
        Auth = "auth",
        Capacity = "capacity",
        Fault = "fault",
    }
}

open_enum! {
    /// The `effect` of an error that answers a mutating request: certainly none, unknown, or partial (HC section 11).
    ///
    /// Clause: HC section 11, EN-3, EN-3b.
    pub enum ErrorEffect {
        None = "none",
        Unknown = "unknown",
        Partial = "partial",
    }
}

impl Category {
    /// A client reads an unknown category as `fault`.
    ///
    /// Clause: HC section 11.
    pub fn or_fault(self) -> Category {
        if self == Category::Other {
            Category::Fault
        } else {
            self
        }
    }
}

/// One entry of `field_errors`: a JSON Pointer, an optional stable field id, a code and a message.
///
/// Clause: HC section 11, SF-2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FieldError {
    pub pointer: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub field_id: Option<String>,
    pub code: String,
    pub message: String,
}

/// The error object of a `res`, a tool result and an operation.
///
/// Clause: HC section 11, A.1b (`detail`), EN-3b.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Error {
    pub code: ErrorCode,
    pub category: Category,
    pub retryable: bool,
    /// Present on every error that answers a mutating request.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub effect: Option<ErrorEffect>,
    pub message: String,
    /// A kind-specific sub-code, listed in the kind's `describe`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub reason: Option<String>,
    /// The data that the contract names next to a code (`sessions`, `candidates`, `interface`, `range`, `key`).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub detail: Option<OpenObject>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub field_errors: Option<Vec<FieldError>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub retry_after_ms: Option<u64>,
}

open_enum! {
    /// Whether a code is retryable (HC section 11). `retryable` on an `Error` is authoritative.
    ///
    /// Clause: HC section 11.
    pub enum Retryable {
        No = "no",
        Yes = "yes",
        /// Decided by `reason` (`wrong_state`).
        DependsOnReason = "depends_on_reason",
    }
}

/// One row of the error registry.
///
/// Clause: HC section 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ErrorRow {
    pub code: ErrorCode,
    pub category: Category,
    pub retryable: Retryable,
    /// The error must carry `retry_after_ms` (`busy`).
    pub retry_after_ms: bool,
    /// The clause that defines the code.
    pub clause: &'static str,
}

const fn row(
    code: ErrorCode,
    category: Category,
    retryable: Retryable,
    retry_after_ms: bool,
    clause: &'static str,
) -> ErrorRow {
    ErrorRow {
        code,
        category,
        retryable,
        retry_after_ms,
        clause,
    }
}

/// The reasons that any kind's request may carry, as `<code>:<reason>`, in the form of the per-kind `reasons` lists. They come from
/// the request fields, not from a kind: a reused `operation_id` (EN-7b) and a reused idempotency key (EN-8).
///
/// Clause: HC EN-7b, EN-8, A4 EN-8a.
pub const COMMON_REASONS: &[&str] = &[
    "conflict:operation_id_in_use",
    "conflict:idempotency_key_reused",
];

/// The error registry, one row per code in the order of `ErrorCode::ALL`.
///
/// Clause: HC section 11.
pub const ERROR_TABLE: &[ErrorRow] = &[
    row(
        ErrorCode::InvalidArguments,
        Category::Caller,
        Retryable::No,
        false,
        "section 11",
    ),
    row(
        ErrorCode::NotFound,
        Category::Caller,
        Retryable::No,
        false,
        "section 11",
    ),
    row(
        ErrorCode::Unsupported,
        Category::Caller,
        Retryable::No,
        false,
        "HS-4, EN-6, VB-7",
    ),
    row(
        ErrorCode::Conflict,
        Category::State,
        Retryable::No,
        false,
        "EN-8, VB-4",
    ),
    row(
        ErrorCode::WrongState,
        Category::State,
        Retryable::DependsOnReason,
        false,
        "VB-5",
    ),
    row(
        ErrorCode::PermissionDenied,
        Category::Auth,
        Retryable::No,
        false,
        "DS-5",
    ),
    row(
        ErrorCode::TooManyRequests,
        Category::Capacity,
        Retryable::Yes,
        false,
        "EN-4",
    ),
    row(
        ErrorCode::Busy,
        Category::Capacity,
        Retryable::Yes,
        true,
        "BD-1",
    ),
    row(
        ErrorCode::ResponseTooLarge,
        Category::Capacity,
        Retryable::No,
        false,
        "EN-5, BD-1",
    ),
    row(
        ErrorCode::PluginFailed,
        Category::Fault,
        Retryable::No,
        false,
        "DS-7",
    ),
    row(
        ErrorCode::RemoteHubUnsupported,
        Category::Caller,
        Retryable::No,
        false,
        "P5",
    ),
    row(
        ErrorCode::HubShutdown,
        Category::State,
        Retryable::Yes,
        false,
        "EN-3",
    ),
    row(
        ErrorCode::ExpiredCursor,
        Category::State,
        Retryable::Yes,
        false,
        "VB-1",
    ),
    row(
        ErrorCode::StaleDescriptor,
        Category::State,
        Retryable::Yes,
        false,
        "DS-6",
    ),
    row(
        ErrorCode::Cancelled,
        Category::State,
        Retryable::Yes,
        false,
        "EN-3b",
    ),
    row(
        ErrorCode::Internal,
        Category::Fault,
        Retryable::No,
        false,
        "section 11",
    ),
];

impl ErrorCode {
    /// The registry row of a known code. `Other` has none.
    ///
    /// Clause: HC section 11.
    pub fn row(self) -> Option<&'static ErrorRow> {
        ERROR_TABLE.iter().find(|r| r.code == self)
    }
}

/// The text of `schemas/hub-client/errors.json`: the codes, categories and retryability.
///
/// Clause: HC section 11.
pub fn errors_json() -> String {
    let rows: Vec<_> = ERROR_TABLE
        .iter()
        .map(|r| {
            serde_json::json!({
                "code": r.code.as_str(),
                "category": r.category.as_str(),
                "retryable": r.retryable.as_str(),
                "retry_after_ms": r.retry_after_ms,
                "clause": r.clause,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "categories": Category::NAMES,
        "unknown_category_reads_as": "fault",
        "effects": ErrorEffect::NAMES,
        "codes": rows,
    });
    let mut text = serde_json::to_string_pretty(&doc).expect("a registry serializes");
    text.push('\n');
    text
}
