//! `conf::ap_b_artifacts_match_code` (HP Appendix B): the artifacts that the code makes equal the checked-in files, and codegen registers each one.

use botster_plugin_contract::prelude::artifact_files;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn vector() -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vectors/conf::ap_b_artifacts_match_code.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("vector file"))
        .expect("vector parses")
}

#[test]
fn conf_ap_b_artifacts_match_code() {
    let v = vector();
    assert_eq!(v["id"], "conf::ap_b_artifacts_match_code");
    let dir = repo_root().join(v["artifacts_dir"].as_str().unwrap());
    let listed: Vec<&str> = v["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let made = artifact_files();
    // Every artifact the code makes is named by the vector, and the other way round.
    let mut made_names: Vec<&str> = made.iter().map(|(n, _)| *n).collect();
    made_names.sort();
    let mut want = listed.clone();
    want.sort();
    assert_eq!(
        made_names, want,
        "the vector lists exactly the artifacts of the code"
    );
    let codegen =
        std::fs::read_to_string(repo_root().join("xtask/src/codegen.rs")).expect("codegen source");
    let mut problems = vec![];
    for (name, text) in &made {
        match std::fs::read_to_string(dir.join(name)) {
            Ok(have) if have == *text => {}
            Ok(_) => problems.push(format!(
                "{name}: the checked-in file differs from the generated text"
            )),
            Err(_) => problems.push(format!("{name}: missing under schemas/plugin")),
        }
        if !codegen.contains(&format!("plugin!(\"{name}\")")) {
            problems.push(format!("{name}: not registered in cargo xtask codegen"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
