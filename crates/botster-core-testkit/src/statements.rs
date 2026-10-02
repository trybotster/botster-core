//! The statement steps of the suite runner that the testkit can answer without a Core (`CoreHarness::statement`).
//!
//! A statement is run by the harness, which reports; the driver checks the report against the statement. A statement that the
//! testkit cannot answer yet is `Unsupported`, which is never a pass.

use botster_core_conformance::ControlError;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The workspace root of this build: the testkit is `crates/botster-core-testkit` in it.
pub fn workspace_root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// `check_crates {crate, not_in, no_dependency_of}` (Core A5-1): the crate is in no facade, and no listed crate depends on it.
///
/// The report is `{found_in, depended_on_by}`. `found_in` lists the facade modules of `not_in` when the facade exposes the crate:
/// the checked-in public API snapshot of the facade (`api/botster-core.txt`) names it, or a facade source file re-exports it with
/// `pub use`. `depended_on_by` lists the crates of `no_dependency_of` that depend on it, directly or through other crates of the
/// workspace, in any dependency table.
pub fn check_crates(root: &Path, spec: &Value) -> Result<Value, ControlError> {
    let bad = |why: &str| ControlError::Bad(why.to_string());
    let krate = spec
        .get("crate")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("needs 'crate'"))?;
    let names = |key: &str| -> Result<Vec<String>, ControlError> {
        spec.get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| bad(&format!("needs '{key}'")))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| bad("a name is a string"))
            })
            .collect()
    };
    let (not_in, no_dependency_of) = (names("not_in")?, names("no_dependency_of")?);
    let ident = krate.replace('-', "_");

    let exposed = facade_exposes(root, &ident);
    let found_in: Vec<&String> = if exposed {
        not_in.iter().collect()
    } else {
        Vec::new()
    };

    let graph = workspace_dependencies(root).map_err(ControlError::Bad)?;
    let depended_on_by: Vec<&String> = no_dependency_of
        .iter()
        .filter(|listed| reaches(&graph, listed, krate))
        .collect();
    Ok(json!({"found_in": found_in, "depended_on_by": depended_on_by}))
}

/// True when the facade's public API snapshot names the crate, or a facade source file has a `pub use` of it.
fn facade_exposes(root: &Path, ident: &str) -> bool {
    let snapshot = std::fs::read_to_string(root.join("api/botster-core.txt")).unwrap_or_default();
    if snapshot.contains(ident) {
        return true;
    }
    let mut files = Vec::new();
    collect_rust_files(&root.join("crates/botster-core/src"), &mut files);
    files.iter().any(|path| {
        std::fs::read_to_string(path).is_ok_and(|text| {
            text.lines().any(|line| {
                let line = line.trim_start();
                (line.starts_with("pub use") || line.starts_with("pub extern crate"))
                    && line.contains(ident)
            })
        })
    })
}

fn collect_rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The dependencies of every crate of the workspace, by package name, from all of its dependency tables.
fn workspace_dependencies(root: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut graph = BTreeMap::new();
    let crates =
        std::fs::read_dir(root.join("crates")).map_err(|e| format!("read crates/: {e}"))?;
    for entry in crates.flatten() {
        let manifest = entry.path().join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let table: toml::Table = text
            .parse()
            .map_err(|e| format!("{}: {e}", manifest.display()))?;
        let Some(name) = table
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
        else {
            continue;
        };
        let mut deps = BTreeSet::new();
        for section in ["dependencies", "build-dependencies", "dev-dependencies"] {
            if let Some(entries) = table.get(section).and_then(toml::Value::as_table) {
                deps.extend(entries.keys().cloned());
            }
        }
        graph.insert(name.to_string(), deps);
    }
    Ok(graph)
}

/// True when `from` depends on `target`, directly or through the crates of the workspace.
fn reaches(graph: &BTreeMap<String, BTreeSet<String>>, from: &str, target: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack = vec![from.to_string()];
    while let Some(name) = stack.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        for dep in graph.get(&name).into_iter().flatten() {
            if dep == target {
                return true;
            }
            stack.push(dep.clone());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(crates: &[(&str, &str)], facade_src: &str, snapshot: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (name, deps) in crates {
            let krate = root.join("crates").join(name);
            std::fs::create_dir_all(krate.join("src")).unwrap();
            std::fs::write(
                krate.join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\n\n[dependencies]\n{deps}\n"),
            )
            .unwrap();
        }
        std::fs::write(root.join("crates/botster-core/src/lib.rs"), facade_src).unwrap();
        std::fs::create_dir_all(root.join("api")).unwrap();
        std::fs::write(root.join("api/botster-core.txt"), snapshot).unwrap();
        dir
    }

    fn spec() -> Value {
        json!({"crate": "botster-core-testkit", "not_in": ["botster_core::prelude", "botster_core::contract"], "no_dependency_of": ["botster-core-ffi"]})
    }

    fn run(dir: &tempfile::TempDir) -> Value {
        check_crates(dir.path(), &spec()).unwrap()
    }

    /// The report that the statement of A5-1 wants: the crate is in no facade and no listed crate depends on it.
    #[test]
    fn a_clean_workspace_reports_nothing() {
        let dir = workspace(
            &[
                ("botster-core", "botster-core-host = \"1\""),
                ("botster-core-ffi", "botster-core = \"1\""),
                ("botster-core-testkit", ""),
                ("botster-core-host", ""),
            ],
            "pub use botster_core_contract::prelude;\n",
            "pub mod botster_core\n",
        );
        assert_eq!(run(&dir), json!({"found_in": [], "depended_on_by": []}));
    }

    #[test]
    fn a_facade_re_export_or_a_snapshot_entry_is_found_in_the_facade() {
        let reexport = workspace(
            &[("botster-core", ""), ("botster-core-ffi", "")],
            "pub use botster_core_testkit::Sim;\n",
            "",
        );
        assert_eq!(
            run(&reexport)["found_in"],
            json!(["botster_core::prelude", "botster_core::contract"])
        );
        let snapshot = workspace(
            &[("botster-core", ""), ("botster-core-ffi", "")],
            "",
            "pub use botster_core::botster_core_testkit::Sim\n",
        );
        assert_eq!(run(&snapshot)["found_in"].as_array().map(Vec::len), Some(2));
        // A use in a comment or a private `use` is not the facade.
        let private = workspace(
            &[("botster-core", ""), ("botster-core-ffi", "")],
            "// pub use botster_core_testkit::Sim;\nuse botster_core_testkit::Sim;\n",
            "",
        );
        assert_eq!(run(&private)["found_in"], json!([]));
    }

    /// A listed crate that depends on the testkit, directly or through another crate, is reported.
    #[test]
    fn a_direct_or_transitive_dependency_of_a_listed_crate_is_reported() {
        let direct = workspace(
            &[
                ("botster-core", ""),
                ("botster-core-ffi", "botster-core-testkit = \"1\""),
                ("botster-core-testkit", ""),
            ],
            "",
            "",
        );
        assert_eq!(run(&direct)["depended_on_by"], json!(["botster-core-ffi"]));
        let transitive = workspace(
            &[
                ("botster-core", ""),
                ("botster-core-ffi", "botster-core-mid = \"1\""),
                (
                    "botster-core-mid",
                    "botster-core-testkit = { path = \"../x\" }",
                ),
                ("botster-core-testkit", ""),
            ],
            "",
            "",
        );
        assert_eq!(
            run(&transitive)["depended_on_by"],
            json!(["botster-core-ffi"])
        );
        // A cycle does not loop.
        let cycle = workspace(
            &[
                ("botster-core", ""),
                ("botster-core-ffi", "botster-core-mid = \"1\""),
                ("botster-core-mid", "botster-core-ffi = \"1\""),
            ],
            "",
            "",
        );
        assert_eq!(run(&cycle)["depended_on_by"], json!([]));
    }

    #[test]
    fn a_statement_with_missing_arguments_is_bad() {
        let dir = workspace(&[("botster-core", "")], "", "");
        for spec in [
            json!({}),
            json!({"crate": "x"}),
            json!({"crate": "x", "not_in": []}),
            json!({"crate": "x", "not_in": [1], "no_dependency_of": []}),
        ] {
            assert!(
                matches!(check_crates(dir.path(), &spec), Err(ControlError::Bad(_))),
                "{spec}"
            );
        }
    }

    /// The real workspace: the testkit is in no facade, and the one crate that lists it is a dev-dependency only.
    #[test]
    fn this_workspace_keeps_the_testkit_out_of_the_facade() {
        let report = check_crates(workspace_root(), &spec()).unwrap();
        assert_eq!(report, json!({"found_in": [], "depended_on_by": []}));
    }
}
