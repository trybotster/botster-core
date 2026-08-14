//! Opaque negotiated terminal capability tokens.
//!
//! Hub constructs this set from advertised feature tokens. Empty sets are
//! valid. Core stores the set on a bound subscription and does not interpret
//! host grants.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::TerminalCompatibility;

/// Immutable set of negotiated terminal feature tokens.
///
/// Empty sets are valid. Unknown tokens fail construction against the
/// advertised feature inventory. This type does not store host grants,
/// protocol version, or conformance revision.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TerminalCapabilitySet {
    tokens: BTreeSet<String>,
}

/// Construction failure for [`TerminalCapabilitySet`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalCapabilitySetError {
    /// Token is not in the advertised terminal feature inventory.
    #[error("unknown terminal capability token: {token}")]
    UnknownToken {
        /// Rejected token.
        token: String,
    },
}

impl TerminalCapabilitySet {
    /// Empty negotiated set. Baseline live output still encodes after bind.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            tokens: BTreeSet::new(),
        }
    }

    /// Build a set from unique advertised tokens.
    ///
    /// An empty iterator succeeds. Unknown tokens fail here, not at Core bind.
    pub fn from_tokens<I, S>(tokens: I) -> Result<Self, TerminalCapabilitySetError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let advertised = TerminalCompatibility::current();
        let mut set = BTreeSet::new();
        for token in tokens {
            let token = token.as_ref();
            if !advertised.supports_feature(token) {
                return Err(TerminalCapabilitySetError::UnknownToken {
                    token: token.to_string(),
                });
            }
            set.insert(token.to_string());
        }
        Ok(Self { tokens: set })
    }

    /// Return whether `token` is present.
    #[must_use]
    pub fn contains(&self, token: &str) -> bool {
        self.tokens.contains(token)
    }

    /// Return whether the set has no tokens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Iterate advertised tokens in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.tokens.iter().map(String::as_str)
    }
}
