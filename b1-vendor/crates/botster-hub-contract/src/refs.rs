//! References that carry the hub (HC P5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A session identity: `{hub_id, session_id}`.
///
/// Clause: HC P5.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SessionRef {
    pub hub_id: String,
    pub session_id: String,
}

/// Any other resource: `{hub_id, kind, id}`. The session `{hub_id, session_id}` is `{hub_id, kind: "session", id: session_id}`.
///
/// Clause: HC P5.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ResourceRef {
    pub hub_id: String,
    pub kind: String,
    pub id: String,
}

impl From<SessionRef> for ResourceRef {
    /// The one conversion of P5.
    ///
    /// Clause: HC P5.
    fn from(s: SessionRef) -> Self {
        ResourceRef {
            hub_id: s.hub_id,
            kind: "session".into(),
            id: s.session_id,
        }
    }
}
