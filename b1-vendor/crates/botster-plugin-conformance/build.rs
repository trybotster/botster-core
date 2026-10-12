//! Writes one `#[test]` per transcript (design 6.1), with the generator that every runner crate uses.
//!
//! Two macros come out: `hub_conformance_tests!` for the transcripts whose subject is `plugin_hub`, and `host_conformance_tests!` for `plugin_host`. A Hub repo calls
//! the first and a plugin-host repo calls the second, so neither sees ids that it cannot run (design 6.5).

#[path = "../botster-conformance/src/testgen.rs"]
mod testgen;

use std::{env, fs, path::PathBuf};

fn ids_of(dir: &PathBuf, subject: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return ids;
    };
    for e in rd.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(id) = name.strip_suffix(".json") else {
            continue;
        };
        let text = fs::read_to_string(e.path()).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if v["subject"] == subject {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    ids
}

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../conformance/hp");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut source = String::new();
    for (name, subject) in [
        ("hub_conformance_tests", "plugin_hub"),
        ("host_conformance_tests", "plugin_host"),
    ] {
        let ids = ids_of(&dir, subject);
        let text = testgen::render_test_macro(
            name,
            "botster-plugin-conformance",
            "&$crate::HP_TRANSCRIPTS",
            &ids,
        );
        // The generated test calls this crate's runner, which builds the subject for the transcript and adds the HP schemas.
        // The one-argument form calls the macro again, so it is called by its crate path.
        let text = text
            .replace("::botster_conformance::run_one_test", "$crate::run_test")
            .replace(
                &format!("{name}!($make,"),
                &format!("$crate::{name}!($make,"),
            );
        source.push_str(&text);
    }
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("hp_tests.rs"),
        source,
    )
    .unwrap();
}
