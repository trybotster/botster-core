//! Deep links to a view (the URL form of a navigation target).
//!
//! Clause: UI-SL-6, UI-SL-9 (ui-contract 1.1 and 1.3).

use crate::doc::NavigationTarget;

/// Percent-encodes a path segment or a query value: every byte outside `A-Z a-z 0-9 - . _ ~` becomes `%XX` (UI-SL-9).
pub fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The part of the URL after `/hubs/<hub_id>`: `/v/<package key>/<view>?<slot>=<row key>` (UI-SL-9).
pub fn route_path(package_key: &str, target: &NavigationTarget) -> String {
    let mut out = format!(
        "/v/{}/{}",
        percent_encode(package_key),
        percent_encode(&target.view)
    );
    for (i, (slot, key)) in target.select.iter().enumerate() {
        out.push(if i == 0 { '?' } else { '&' });
        out.push_str(&format!("{}={}", percent_encode(slot), percent_encode(key)));
    }
    out
}

/// The Web URL of a navigation target (UI-SL-9).
pub fn web_url(origin: &str, hub_id: &str, package_key: &str, target: &NavigationTarget) -> String {
    format!(
        "{origin}/hubs/{}{}",
        percent_encode(hub_id),
        route_path(package_key, target)
    )
}

/// Decodes `%XX` escapes. `None` when an escape is malformed or the bytes are not UTF-8 (UI-SL-6).
pub fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// True when a `route_path` names a view: `/v/<package key>/<view>` (UI-SL-9). The paths `/signal` and `/ice` below
/// `/hubs/<hub_id>` are reserved and never a view.
pub fn is_view_route(route: &str) -> bool {
    route.starts_with("/v/")
}

/// Reads a `route_path` back into the package key, the view and the slots (UI-SL-6: the same slots are restored when
/// the URL loads). `None` when it is not a view route.
pub fn parse_route(route: &str) -> Option<(String, NavigationTarget)> {
    let rest = route.strip_prefix("/v/")?;
    let (path, query) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };
    let (package, view) = path.split_once('/')?;
    let mut target = NavigationTarget {
        view: percent_decode(view)?,
        select: Default::default(),
    };
    for pair in query.into_iter().flat_map(|q| q.split('&')) {
        let (slot, key) = pair.split_once('=')?;
        target
            .select
            .insert(percent_decode(slot)?, percent_decode(key)?);
    }
    Some((percent_decode(package)?, target))
}
