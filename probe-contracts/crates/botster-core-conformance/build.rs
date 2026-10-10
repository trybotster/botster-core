//! Writes one `#[test]` per Core transcript (design 6.1), with the generator that every runner crate uses.

#[path = "../botster-conformance/src/testgen.rs"]
mod testgen;

use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../conformance/core");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut ids: Vec<String> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| {
                let name = e.ok()?.file_name().into_string().ok()?;
                name.strip_suffix(".json").map(str::to_string)
            })
            .collect()
        })
        .unwrap_or_default();
    ids.sort();
    let source = testgen::render_test_macro(
        "conformance_tests",
        "botster-core-conformance",
        "&$crate::CORE_TRANSCRIPTS",
        &ids,
    );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("core_tests.rs"),
        source,
    )
    .unwrap();
}
