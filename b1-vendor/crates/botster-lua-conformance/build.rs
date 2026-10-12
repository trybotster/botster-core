//! Writes one `#[test]` per Lua binding transcript (design 6.1), with the generator that every runner crate uses.

#[path = "../botster-conformance/src/testgen.rs"]
mod testgen;

use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../conformance/lb");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut ids = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            let v: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(e.path()).unwrap_or_default())
                    .unwrap_or_default();
            if v["subject"] == "lua_host" {
                ids.push(id.to_string());
            }
        }
    }
    let text = testgen::render_test_macro(
        "lua_host_conformance_tests",
        "botster-lua-conformance",
        "&$crate::LB_TRANSCRIPTS",
        &ids,
    );
    // The generated test calls this crate's runner, which wraps the subject for the HP driver and shares the HP frame templates.
    let text = text
        .replace("::botster_conformance::run_one_test", "$crate::run_test")
        .replace(
            "lua_host_conformance_tests!($make,",
            "$crate::lua_host_conformance_tests!($make,",
        );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("lb_tests.rs"),
        text,
    )
    .unwrap();
}
