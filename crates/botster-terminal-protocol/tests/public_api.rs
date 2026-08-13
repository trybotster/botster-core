#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use botster_terminal_protocol::PUBLIC_API_ALLOWLIST;

#[test]
fn public_source_items_match_allowlist() {
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(&src_dir).expect("src dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read rust source");
        collect_module_public_items(&source, &mut found);
    }

    let allowed: BTreeSet<&str> = PUBLIC_API_ALLOWLIST.iter().copied().collect();
    let found_names: BTreeSet<&str> = found.iter().map(String::as_str).collect();
    assert_eq!(
        found_names,
        allowed,
        "public API drifted.\nonly in source: {:?}\nonly in allowlist: {:?}",
        found_names.difference(&allowed).collect::<Vec<_>>(),
        allowed.difference(&found_names).collect::<Vec<_>>()
    );
}

#[test]
fn terminal_frame_source_has_no_semantic_accessors() {
    let frame = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/frame.rs"))
        .expect("frame source");
    for forbidden in [
        "fn phase",
        "fn state",
        "fn history",
        "fn payload",
        "fn snapshot",
        "pub phase",
        "pub state",
        "pub history",
        "pub payload",
    ] {
        assert!(
            !frame.contains(forbidden),
            "TerminalFrame source must not expose `{forbidden}`"
        );
    }
}

#[test]
fn crate_tree_excludes_runtime_and_hub_dependencies() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "botster-terminal-protocol",
            "--prefix",
            "none",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("cargo tree");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    for crate_name in [
        "botster-core ",
        "botster-core-daemon",
        "botster-hub ",
        "botster-hub-client",
    ] {
        assert!(
            !tree.contains(crate_name),
            "forbidden dependency {crate_name} in tree:\n{tree}"
        );
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn collect_module_public_items(source: &str, found: &mut BTreeSet<String>) {
    let mut pending_use_group = false;
    for line in source.lines() {
        if pending_use_group {
            insert_use_names(line, found);
            if line.contains('}') {
                pending_use_group = false;
            }
            continue;
        }
        if !line.starts_with("pub ") {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        match tokens.as_slice() {
            ["pub", "const", name, ..]
            | ["pub", "struct", name, ..]
            | ["pub", "enum", name, ..] => {
                found.insert(clean_item_name(name));
            }
            ["pub", "fn", name, ..] => {
                found.insert(clean_item_name(name));
            }
            ["pub", "use", rest @ ..] => {
                let joined = rest.join(" ");
                insert_use_names(&joined, found);
                if joined.contains('{') && !joined.contains('}') {
                    pending_use_group = true;
                }
            }
            _ => {}
        }
    }
}

fn clean_item_name(name: &str) -> String {
    name.trim_end_matches('{')
        .trim_end_matches('<')
        .trim_end_matches(':')
        .split('(')
        .next()
        .unwrap_or(name)
        .to_string()
}

fn insert_use_names(fragment: &str, found: &mut BTreeSet<String>) {
    let start = fragment.find('{').map(|index| index + 1).unwrap_or(0);
    let end = fragment.find('}').unwrap_or(fragment.len());
    if start > end {
        return;
    }
    for name in fragment[start..end].split(',') {
        let name = name.trim().trim_end_matches(';');
        if !name.is_empty() && name != "{" {
            found.insert(name.to_string());
        }
    }
}
