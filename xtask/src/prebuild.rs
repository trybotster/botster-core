//! `cargo xtask prebuild-worker`: builds the binaries that the real-process tier runs, into `target/candidate/`, with a
//! sha256 manifest (plan section 5; the idea of the old `script/prebuild-worker`). A test never builds a binary, which also
//! avoids the macOS first-launch stall inside a test (BUILD.md Testing rule 7).
//!
//! - `botster-worker`: built from this workspace once a package of that name has a binary (P3). Until then nothing is built.
//! - `botster-conformance-probe`: the program of the real-process tier, built from the pinned botster-contracts tag with
//!   `cargo install --git`.

use crate::fsutil::{metadata, Meta};
use crate::tools::{cargo, run};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const PROBE: &str = "botster-conformance-probe";
const WORKER: &str = "botster-worker";

/// The git url and the tag of the contracts dependency in the root `Cargo.toml`.
fn contracts_source(cargo_toml: &str) -> Result<(String, String)> {
    let table: toml::Table = cargo_toml.parse().context("parse Cargo.toml")?;
    let dep = &table["workspace"]["dependencies"]["botster-core-contract"];
    let field = |name: &str| -> Result<String> {
        Ok(dep
            .get(name)
            .and_then(toml::Value::as_str)
            .with_context(|| format!("botster-core-contract has no `{name}`"))?
            .to_string())
    };
    Ok((field("git")?, field("tag")?))
}

fn sha256_hex(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// The manifest text: one entry per binary, sorted by name.
fn manifest_text(entries: &[(String, String)]) -> String {
    let mut sorted: Vec<&(String, String)> = entries.iter().collect();
    sorted.sort();
    let binaries: Vec<serde_json::Value> = sorted
        .iter()
        .map(|(name, sha)| serde_json::json!({ "name": name, "path": name, "sha256": sha }))
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "binaries": binaries })).expect("json") + "\n"
}

/// Puts `built` at `target` as a new file: the bytes go to a fresh file, which is renamed over `target`. The old inode is
/// never rewritten, because macOS can kill a re-signed binary that was written over an inode that already ran (the lesson of
/// the old `script/prebuild-worker`).
fn install_executable(built: &Path, target: &Path) -> Result<()> {
    let name = target
        .file_name()
        .context("a target has a file name")?
        .to_string_lossy();
    let fresh = target.with_file_name(format!(".{name}.new"));
    let _ = std::fs::remove_file(&fresh);
    std::fs::copy(built, &fresh).with_context(|| format!("copy {}", built.display()))?;
    std::fs::rename(&fresh, target).with_context(|| format!("replace {}", target.display()))
}

/// Whether the workspace has a package named `botster-worker` with a binary target.
fn has_worker_binary(members: &[(String, bool)]) -> bool {
    members
        .iter()
        .any(|(name, has_bin)| name == WORKER && *has_bin)
}

fn build_worker(root: &Path, meta: &Meta, candidate: &Path) -> Result<Option<(String, String)>> {
    if !has_worker_binary(&meta.members) {
        println!("prebuild-worker: no `{WORKER}` binary in the workspace yet (P3 adds it); nothing to build");
        return Ok(None);
    }
    let mut build = cargo(root);
    build.args(["build", "-p", WORKER, "--locked"]);
    run(build)?;
    let built = meta.target_dir.join("debug").join(WORKER);
    let target = candidate.join(WORKER);
    install_executable(&built, &target)?;
    Ok(Some((WORKER.to_string(), sha256_hex(&target)?)))
}

fn build_probe(root: &Path, meta: &Meta, candidate: &Path) -> Result<(String, String)> {
    let (git, tag) = contracts_source(&std::fs::read_to_string(root.join("Cargo.toml"))?)?;
    let install_root: PathBuf = meta.target_dir.join("probe-install");
    let mut install = cargo(root);
    install
        .args([
            "install", "--locked", "--force", "--git", &git, "--tag", &tag,
        ])
        .arg("--root")
        .arg(&install_root)
        .arg("--target-dir")
        .arg(meta.target_dir.join("probe-build"))
        .arg(PROBE);
    run(install)?;
    let built = install_root.join("bin").join(PROBE);
    let target = candidate.join(PROBE);
    install_executable(&built, &target)?;
    Ok((PROBE.to_string(), sha256_hex(&target)?))
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    crate::caps::require()?;
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let meta = metadata(root)?;
    let candidate = meta.target_dir.join("candidate");
    std::fs::create_dir_all(&candidate)?;
    // The manifest is written last, after every replacement succeeded: a failed run leaves none.
    let manifest = candidate.join("manifest.json");
    let _ = std::fs::remove_file(&manifest);
    let mut entries = Vec::new();
    entries.extend(build_worker(root, &meta, &candidate)?);
    entries.push(build_probe(root, &meta, &candidate)?);
    std::fs::write(&manifest, manifest_text(&entries))?;
    println!(
        "prebuild-worker: wrote {} ({} binaries)",
        manifest.display(),
        entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_contracts_source_is_read_from_the_dependency() {
        let text = "[workspace.dependencies]\nbotster-core-contract = { git = \"https://x/y\", tag = \"t1\" }\n";
        assert_eq!(
            contracts_source(text).unwrap(),
            ("https://x/y".to_string(), "t1".to_string())
        );
        assert!(contracts_source(
            "[workspace.dependencies]\nbotster-core-contract = { git = \"u\", rev = \"r\" }\n"
        )
        .is_err());
    }

    #[test]
    fn the_worker_is_built_only_when_a_package_of_that_name_has_a_binary() {
        let member = |name: &str, bin: bool| (name.to_string(), bin);
        assert!(has_worker_binary(&[
            member("a", true),
            member("botster-worker", true)
        ]));
        assert!(!has_worker_binary(&[member("botster-worker", false)]));
        assert!(!has_worker_binary(&[member("other", true)]));
        assert!(!has_worker_binary(&[]));
    }

    #[test]
    fn the_manifest_lists_binaries_sorted_with_their_hashes() {
        let text = manifest_text(&[("b".into(), "22".into()), ("a".into(), "11".into())]);
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["binaries"][0]["name"], "a");
        assert_eq!(json["binaries"][0]["sha256"], "11");
        assert_eq!(json["binaries"][1]["name"], "b");
    }

    #[test]
    fn an_installed_executable_is_a_new_file_with_the_new_bytes() {
        use std::os::unix::fs::MetadataExt;
        let root = botster_test_support::tempdir::TempRoot::new().unwrap();
        let built = root.path().join("built");
        let target = root.path().join("candidate");
        std::fs::write(&built, b"new").unwrap();
        std::fs::write(&target, b"old").unwrap();
        let old_inode = std::fs::metadata(&target).unwrap().ino();
        install_executable(&built, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert_ne!(
            std::fs::metadata(&target).unwrap().ino(),
            old_inode,
            "the old inode was rewritten"
        );
        assert!(
            !root.path().join(".candidate.new").exists(),
            "no temporary file is left"
        );
        // A target that does not exist yet is installed too.
        let fresh = root.path().join("second");
        install_executable(&built, &fresh).unwrap();
        assert_eq!(std::fs::read(&fresh).unwrap(), b"new");
    }

    #[test]
    fn a_file_hash_is_the_sha256_of_its_bytes() {
        let root = botster_test_support::tempdir::TempRoot::new().unwrap();
        let file = root.path().join("f");
        std::fs::write(&file, b"abc").unwrap();
        assert_eq!(
            sha256_hex(&file).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
