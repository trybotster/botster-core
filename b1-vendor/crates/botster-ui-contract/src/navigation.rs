//! Navigation badges and home placement (UI-NB-1, UI-PL-1, UI-CF-5).
//!
//! Clause: UI-NB-1 (a badge reads one row and one field), UI-PL-1 (home order and size), UI-CF-5(b) and (c)
//! (the fallbacks). Both are pure functions over the same inputs as `realize` (UI-RZ-1).

use crate::doc::{Badge, OrUnknown, Placement};
use crate::ids::row_key;
use crate::input::{Capabilities, KindSnapshot};
use crate::realize::FEATURE_DISPLAY_BADGE;
use crate::value::{es_number, truthy};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A navigation badge that is shown (UI-NB-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RealizedBadge {
    /// `dot` or `count`.
    pub display: String,
    /// The integer of a `count` badge, as the row gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    /// UI-CF-5(b): the suffix on the entry label of a renderer without a badge widget: `(n)` or `●`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
}

/// The badge of a navigation entry (UI-NB-1). `None` hides it: a missing row, an absent field, `0`, `false` or a type
/// mismatch.
pub fn navigation_badge(
    badge: &Badge,
    snapshots: &BTreeMap<String, KindSnapshot>,
    caps: &Capabilities,
) -> Option<RealizedBadge> {
    let snap = snapshots.get(&badge.source.kind)?;
    let row = snap
        .rows
        .iter()
        .find(|r| row_key(r).as_deref() == Some(badge.source.id.as_str()))?;
    let field = row.pointer(&badge.source.field);
    let value = match badge.display.as_str() {
        // UI-NB-1: `count` shows the field when it is an integer greater than 0.
        "count" => match field {
            // An integer is a number without a fraction, whatever its JSON spelling: `1`, `1.0` or a value above
            // `i64::MAX` (UI-NB-1 fixes no bound).
            Some(Value::Number(n)) => n
                .as_f64()
                .filter(|f| f.is_finite() && *f > 0.0 && f.fract() == 0.0)
                .map(|_| Some(n.clone()))?,
            _ => return None,
        },
        // UI-NB-1: `dot` shows a dot when the field is truthy (UI-BD-1).
        "dot" => {
            if !truthy(field) {
                return None;
            }
            None
        }
        _ => return None,
    };
    let tone = match &badge.tone {
        Some(OrUnknown::Known(t)) => Some(t.as_str().to_string()),
        _ => None,
    };
    // UI-CF-5(b): without a badge widget, a suffix on the entry label.
    let suffix = (!caps.has_feature(FEATURE_DISPLAY_BADGE)).then(|| match &value {
        Some(n) => format!("({})", es_number(n.as_f64().unwrap_or(0.0))),
        None => "●".to_string(),
    });
    Some(RealizedBadge {
        display: badge.display.clone(),
        value,
        tone,
        suffix,
    })
}

/// The home area of a client (UI-PL-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HomeArea {
    /// The sizes that the client's grid has. `small` is the smallest.
    pub sizes: Vec<String>,
}

/// One home-placed view as the client shows it (UI-PL-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HomeEntry {
    pub view: String,
    /// The size after the mapping.
    pub size: String,
}

/// The views that the home area shows, in ascending `order` and then by view id (UI-PL-1). A client with fewer sizes
/// maps `full` to `wide` and `wide` to `medium`. A client with no home area (`None`) shows the views in their normal
/// place only (UI-CF-5(c)), so the list is empty.
pub fn home_layout(views: &[(String, Placement)], home: Option<&HomeArea>) -> Vec<HomeEntry> {
    let Some(home) = home else {
        return vec![];
    };
    let mut sorted: Vec<&(String, Placement)> = views.iter().collect();
    sorted.sort_by(|a, b| a.1.order.cmp(&b.1.order).then_with(|| a.0.cmp(&b.0)));
    sorted
        .into_iter()
        .map(|(id, p)| HomeEntry {
            view: id.clone(),
            size: map_size(&p.size, &home.sizes),
        })
        .collect()
}

fn map_size(size: &str, sizes: &[String]) -> String {
    let mut size = size;
    while !sizes.iter().any(|s| s == size) {
        size = match size {
            "full" => "wide",
            "wide" => "medium",
            // `small` is the smallest.
            _ => return "small".to_string(),
        };
    }
    size.to_string()
}
