//! Keyed transactional store mechanism.
//!
//! Core owns the trait, the size limits, and the typed errors. Hub owns
//! namespaces, grants, and quota policy over this trait. The production
//! backend is `redb`, one table per namespace.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum key length in bytes.
pub const MAX_KEY_BYTES: usize = 512;
/// Maximum value length in bytes.
pub const MAX_VALUE_BYTES: usize = 1 << 20;
/// Maximum operations in one atomic batch.
pub const MAX_BATCH_OPS: usize = 256;
/// Maximum items returned by one range page.
pub const MAX_RANGE_ITEMS: usize = 1_000;
/// Maximum key plus value bytes returned by one range page.
pub const MAX_RANGE_BYTES: usize = 4 << 20;
/// Maximum namespace length in bytes.
pub const MAX_NAMESPACE_BYTES: usize = 64;

/// Validated store namespace: `^[a-z0-9_.-]{1,64}$`. One table per namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Namespace(String);

impl Namespace {
    /// Validate a namespace string.
    pub fn new(name: &str) -> Result<Self, NamespaceError> {
        if name.is_empty() || name.len() > MAX_NAMESPACE_BYTES {
            return Err(NamespaceError::Length {
                max: MAX_NAMESPACE_BYTES,
                actual: name.len(),
            });
        }
        if let Some(character) = name
            .chars()
            .find(|character| !matches!(character, 'a'..='z' | '0'..='9' | '_' | '.' | '-'))
        {
            return Err(NamespaceError::Character { character });
        }
        Ok(Self(name.to_owned()))
    }

    /// Namespace text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<String> for Namespace {
    type Error = NamespaceError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<Namespace> for String {
    fn from(namespace: Namespace) -> Self {
        namespace.0
    }
}

/// Namespace validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NamespaceError {
    /// Namespace is empty or longer than [`MAX_NAMESPACE_BYTES`].
    #[error("namespace length must be 1 through {max} bytes, got {actual}")]
    Length {
        /// Maximum length.
        max: usize,
        /// Observed length.
        actual: usize,
    },
    /// Namespace contains a character outside `[a-z0-9_.-]`.
    #[error("namespace contains invalid character {character:?}")]
    Character {
        /// First invalid character.
        character: char,
    },
}

/// One write in an atomic batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreOp {
    /// Insert or replace `key` with `value`.
    Put {
        /// Key, at most [`MAX_KEY_BYTES`].
        key: Vec<u8>,
        /// Value, at most [`MAX_VALUE_BYTES`].
        value: Vec<u8>,
    },
    /// Remove `key`. Removing an absent key succeeds.
    Delete {
        /// Key to remove.
        key: Vec<u8>,
    },
}

impl StoreOp {
    /// Key this operation touches.
    #[must_use]
    pub fn key(&self) -> &[u8] {
        match self {
            Self::Put { key, .. } | Self::Delete { key } => key,
        }
    }
}

/// One key and value returned by a range read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeItem {
    /// Key.
    pub key: Vec<u8>,
    /// Value.
    pub value: Vec<u8>,
}

/// One bounded page of a prefix range in ascending key order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RangePage {
    /// Items in ascending key order.
    pub items: Vec<RangeItem>,
    /// Whether more items exist after the last returned key.
    pub has_more: bool,
}

impl RangePage {
    /// Key to pass as `after` for the next page, when [`Self::has_more`].
    #[must_use]
    pub fn next_after(&self) -> Option<&[u8]> {
        if self.has_more {
            self.items.last().map(|item| item.key.as_slice())
        } else {
            None
        }
    }
}

/// Typed store failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoreError {
    /// Key exceeds [`MAX_KEY_BYTES`] or is empty.
    #[error("store key length must be 1 through {max} bytes, got {actual}")]
    KeyLength {
        /// Maximum length.
        max: usize,
        /// Observed length.
        actual: usize,
    },
    /// Value exceeds [`MAX_VALUE_BYTES`].
    #[error("store value too large: max {max} bytes, got {actual}")]
    ValueTooLarge {
        /// Maximum length.
        max: usize,
        /// Observed length.
        actual: usize,
    },
    /// Batch has zero operations or more than [`MAX_BATCH_OPS`].
    #[error("store batch must contain 1 through {max} operations, got {actual}")]
    BatchSize {
        /// Maximum operations.
        max: usize,
        /// Observed operations.
        actual: usize,
    },
    /// Range request exceeds [`MAX_RANGE_ITEMS`] or [`MAX_RANGE_BYTES`].
    #[error("store range budget too large: max_items {max_items}, max_bytes {max_bytes}")]
    RangeBudget {
        /// Requested item limit.
        max_items: usize,
        /// Requested byte limit.
        max_bytes: usize,
    },
    /// Backend I/O or transaction failure. The batch did not apply.
    #[error("store backend failed during {operation}: {message}")]
    Backend {
        /// Operation name.
        operation: &'static str,
        /// Backend detail.
        message: String,
    },
}

/// Keyed transactional store over namespaced tables.
///
/// `batch` is atomic: every operation applies or none does. `range` returns
/// keys with `prefix`, strictly after `after` when given, in ascending order,
/// bounded by `max_items` and `max_bytes`, and never fewer than one item when
/// any item exists within the budget. Implementations validate every limit in
/// this module before touching the backend.
pub trait KeyedStore: Send + Sync {
    /// Read one value.
    fn get(&self, namespace: &Namespace, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;

    /// Read one bounded page of a prefix range.
    fn range(
        &self,
        namespace: &Namespace,
        prefix: &[u8],
        after: Option<&[u8]>,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<RangePage, StoreError>;

    /// Apply every operation atomically.
    fn batch(&self, namespace: &Namespace, ops: &[StoreOp]) -> Result<(), StoreError>;
}

/// Validate a key against [`MAX_KEY_BYTES`].
pub fn check_key(key: &[u8]) -> Result<(), StoreError> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(StoreError::KeyLength {
            max: MAX_KEY_BYTES,
            actual: key.len(),
        });
    }
    Ok(())
}

/// Validate a value against [`MAX_VALUE_BYTES`].
pub fn check_value(value: &[u8]) -> Result<(), StoreError> {
    if value.len() > MAX_VALUE_BYTES {
        return Err(StoreError::ValueTooLarge {
            max: MAX_VALUE_BYTES,
            actual: value.len(),
        });
    }
    Ok(())
}

/// Validate a batch: size and every key and value.
pub fn check_batch(ops: &[StoreOp]) -> Result<(), StoreError> {
    if ops.is_empty() || ops.len() > MAX_BATCH_OPS {
        return Err(StoreError::BatchSize {
            max: MAX_BATCH_OPS,
            actual: ops.len(),
        });
    }
    for op in ops {
        check_key(op.key())?;
        if let StoreOp::Put { value, .. } = op {
            check_value(value)?;
        }
    }
    Ok(())
}

/// Validate a range budget against [`MAX_RANGE_ITEMS`] and [`MAX_RANGE_BYTES`].
pub fn check_range_budget(max_items: usize, max_bytes: usize) -> Result<(), StoreError> {
    if max_items == 0
        || max_items > MAX_RANGE_ITEMS
        || max_bytes == 0
        || max_bytes > MAX_RANGE_BYTES
    {
        return Err(StoreError::RangeBudget {
            max_items,
            max_bytes,
        });
    }
    Ok(())
}
