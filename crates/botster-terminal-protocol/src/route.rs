//! Route identity and the routed terminal envelope.
//!
//! A route is one subscription on one connection. Core validates the id once
//! at attach and shares it as an `Arc<str>` on every routed frame. Hub reads
//! route and generation from this envelope, never from the body.

use std::fmt;
use std::sync::Arc;

use crate::frame::TerminalFrame;

/// Maximum route id length in UTF-8 bytes.
pub const MAX_ROUTE_ID_BYTES: usize = 1024;

/// Validated subscription route id.
///
/// UTF-8, 1 through 1024 bytes, no control characters. Cloning clones the
/// `Arc`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RouteId(Arc<str>);

impl RouteId {
    /// Validate and intern a route id.
    pub fn new(id: &str) -> Result<Self, RouteIdError> {
        if id.is_empty() {
            return Err(RouteIdError::Empty);
        }
        if id.len() > MAX_ROUTE_ID_BYTES {
            return Err(RouteIdError::TooLong {
                max: MAX_ROUTE_ID_BYTES,
                actual: id.len(),
            });
        }
        if let Some(control) = id.chars().find(|character| character.is_control()) {
            return Err(RouteIdError::ControlCharacter { control });
        }
        Ok(Self(Arc::from(id)))
    }

    /// Route id text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Shared handle to the route id text.
    #[must_use]
    pub fn shared(&self) -> &Arc<str> {
        &self.0
    }

    /// Route id length in UTF-8 bytes. Always 1 through 1024.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Never true for a validated route id.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for RouteId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Route id validation failure. Attach is rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RouteIdError {
    /// Route id has zero bytes.
    #[error("route id is empty")]
    Empty,
    /// Route id exceeds the byte ceiling.
    #[error("route id too long: max {max} bytes, actual {actual}")]
    TooLong {
        /// Maximum length in bytes.
        max: usize,
        /// Observed length in bytes.
        actual: usize,
    },
    /// Route id contains a Unicode control character.
    #[error("route id contains control character {control:?}")]
    ControlCharacter {
        /// First control character found.
        control: char,
    },
}

/// One terminal frame addressed to one route.
///
/// `generation` is the route stream generation. It starts equal to the Core
/// attach generation and increases by one on every `ROUTE_RESYNC` for that
/// route. The resync frame and every later frame carry the new value, so a
/// frame queued before the overflow is fenced by its lower generation. Hub
/// copies this value into its container verbatim. A client adopts a higher
/// generation only from `ATTACH_STATE` or `ROUTE_RESYNC` and discards any
/// frame whose generation differs from the adopted value. Input operation ids
/// are per attach and do not restart on resync. Cloning clones two `Arc`s and
/// copies the generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedTerminalFrame {
    /// Destination route.
    pub route: RouteId,
    /// Attach generation of the destination route.
    pub generation: u64,
    /// Shared or personalized frame body.
    pub frame: TerminalFrame,
}

impl RoutedTerminalFrame {
    /// Address one frame to one route.
    #[must_use]
    pub const fn new(route: RouteId, generation: u64, frame: TerminalFrame) -> Self {
        Self {
            route,
            generation,
            frame,
        }
    }
}
