//! Generates one `#[test]` per toy transcript, with the same generator that runner crates use (design 6.1).

#[path = "src/testgen.rs"]
mod testgen;

use std::{env, fs, path::PathBuf};

fn main() {
    let dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("tests/fixtures/transcripts");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut ids: Vec<String> = fs::read_dir(&dir)
        .expect("fixture directory exists")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();
    ids.sort();
    let source = testgen::render_test_macro(
        "toy_conformance_tests",
        "botster-conformance",
        "&$crate::TOY_TRANSCRIPTS",
        &ids,
    );
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("toy_tests.rs");
    fs::write(out, source).unwrap();
}
