//! Runs every vector file against the reference `realize` and admission (UI-RZ-3, UI-RZ-3b).
//! The runner makes one pass per file and runs each case twice to check determinism (UI-RZ-1).

use botster_ui_contract::prelude::{run_vector_file, vector_files};

#[test]
fn every_vector_passes() {
    let files = vector_files();
    assert!(!files.is_empty(), "no vectors found");
    let mut failures = vec![];
    for (name, file) in &files {
        let stem = name.trim_end_matches(".json");
        if file["id"] != stem {
            failures.push(format!("{name}: the id must equal the file name"));
        }
        if let Err(f) = run_vector_file(file) {
            failures.extend(f);
        }
    }
    assert!(
        failures.is_empty(),
        "{} failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
