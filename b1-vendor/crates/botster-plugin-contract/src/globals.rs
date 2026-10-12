//! The Lua host globals (the source of `lua-host-globals.json`).
//!
//! Clause: HP LH-4, section 17, Appendix B; Lua binding section D (the global set). `Ready.host.globals` is the published source and the host wins on a difference.

use crate::calls::{CALLS, LUA_LOCAL_NAMES, LUA_LOG_SUGAR, LUA_REGISTRATION_NAMES};
use serde_json::{json, Value};

/// Base functions available to plugin code (Lua binding section D). `setmetatable` refuses `__gc`.
pub const BASE_FUNCTIONS: &[&str] = &[
    "assert",
    "error",
    "getmetatable",
    "ipairs",
    "next",
    "pairs",
    "pcall",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "select",
    "setmetatable",
    "tonumber",
    "tostring",
    "type",
    "xpcall",
];

/// Libraries available to plugin code (Lua binding section D).
pub const LIBRARIES: &[&str] = &["string", "table", "math", "utf8"];

/// Other globals: `require` of staged modules and the `botster` table (Lua binding section D).
pub const OTHER_GLOBALS: &[&str] = &["require", "botster"];

/// Names that are not available to plugin code (Lua binding section D).
pub const UNAVAILABLE: &[&str] = &[
    "os",
    "io",
    "package",
    "debug",
    "coroutine",
    "load",
    "loadfile",
    "dofile",
    "collectgarbage",
    "print",
];

/// The exact global names the Lua host exposes, in a stable order (HP LH-4).
pub fn globals() -> Vec<&'static str> {
    BASE_FUNCTIONS
        .iter()
        .chain(LIBRARIES)
        .chain(OTHER_GLOBALS)
        .copied()
        .collect()
}

/// The names under the `botster` table: every host call (HP LN-1, LN-2), the level sugar, the host-local names and the registration helpers.
pub fn botster_names() -> Vec<String> {
    let mut v: Vec<String> = CALLS.iter().map(|c| c.name.to_string()).collect();
    v.extend(LUA_LOG_SUGAR.iter().map(|s| s.to_string()));
    v.extend(LUA_LOCAL_NAMES.iter().map(|s| s.to_string()));
    v.extend(LUA_REGISTRATION_NAMES.iter().map(|s| s.to_string()));
    v.sort();
    v
}

/// The text of `lua-host-globals.json` as a JSON value (HP Appendix B).
pub fn artifact_json() -> Value {
    json!({
        "description": "HP Appendix B, LH-4: the exact global names the Lua host exposes. Ready.host.globals is the published source; the host wins on a difference.",
        "base_functions": BASE_FUNCTIONS,
        "libraries": LIBRARIES,
        "other": OTHER_GLOBALS,
        "globals": globals(),
        "unavailable": UNAVAILABLE,
        "botster": botster_names(),
    })
}
