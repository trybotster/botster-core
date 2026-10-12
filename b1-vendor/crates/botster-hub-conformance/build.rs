//! Writes one `#[test]` per transcript (design 6.1), with the generator that every runner crate uses: the codec suite and the HC suite.

#[path = "../botster-conformance/src/testgen.rs"]
mod testgen;

use std::{env, fs, path::PathBuf};

/// `subject` selects the transcripts of one subject (`hub` or `client`) by their `subject` member; `None` takes every id.
fn write_tests(suite: &str, subject: Option<&str>, macro_name: &str, dir_expr: &str, out: &str) {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../conformance")
        .join(suite);
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut ids: Vec<String> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| {
                let path = e.ok()?.path();
                let name = path
                    .file_name()?
                    .to_str()?
                    .strip_suffix(".json")?
                    .to_string();
                if let Some(want) = subject {
                    let doc: serde_json::Value =
                        serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
                    if doc["subject"] != want {
                        return None;
                    }
                }
                Some(name)
            })
            .collect()
        })
        .unwrap_or_default();
    ids.sort();
    let source = testgen::render_test_macro(macro_name, "botster-hub-conformance", dir_expr, &ids);
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join(out),
        source,
    )
    .unwrap();
}

fn main() {
    // The UI ids whose subject is the Hub run here (design 6.4); the other UI transcripts belong to `botster-ui-conformance`.
    write_tests(
        "ui",
        Some("hub"),
        "ui_hub_conformance_tests",
        "&$crate::UI_TRANSCRIPTS",
        "ui_hub_tests.rs",
    );
    write_tests(
        "codec",
        None,
        "codec_conformance_tests",
        "&$crate::CODEC_TRANSCRIPTS",
        "codec_tests.rs",
    );
    // A Hub repo runs the shared transcripts (subject `hub`); a client repo runs the client-only ones (subject `client`) (HC A5-3).
    write_tests(
        "hc",
        Some("hub"),
        "hc_hub_conformance_tests",
        "&$crate::hc::HC_TRANSCRIPTS",
        "hc_hub_tests.rs",
    );
    write_tests(
        "hc",
        Some("client"),
        "hc_client_conformance_tests",
        "&$crate::hc::HC_TRANSCRIPTS",
        "hc_client_tests.rs",
    );
}
