//! The xtask commands against a small repo that each test builds under the target directory.

use botster_test_support::tempdir::TempRoot;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const CONTRACT: &str = "# Doc\n\n## 1. Section\n\n| EN-1 | text | `conf::en_1_a` |\n| EN-2 | text\n- more\n| `conf::en_2_b` |\n";
const TRANSCRIPT: &str =
    r#"{"id":"conf::en_1_a","contract":"hc","clause":"EN-1","subject":"hub","steps":[]}"#;

struct Repo {
    root: PathBuf,
    /// Owns the root: a unique canonical directory per instance, removed on drop (BUILD.md Testing rule 6).
    _tmp: TempRoot,
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

impl Repo {
    fn new() -> Repo {
        let tmp = TempRoot::new().unwrap();
        let repo = Repo {
            root: tmp.path().to_path_buf(),
            _tmp: tmp,
        };
        write(&repo.root, "contracts.toml", "dir = \"frozen/final6\"\n[[contract]]\nprefix = \"hc\"\nfiles = [\"hc.md\"]\n[superseded]\nfiles = []\n");
        write(&repo.root, "frozen/final6/hc.md", CONTRACT);
        write(&repo.root, "docs/taint-denylist.txt", "# c\nbanned_name\n");
        repo.ok(&["ledger"]);
        write(&repo.root, "conformance/pending.txt", "conf::en_2_b\n");
        write(&repo.root, "conformance/hc/conf::en_1_a.json", TRANSCRIPT);
        repo
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Runs the command in-process (BUILD.md Testing rule 7). The error is its stderr text, as the binary prints it.
    fn xtask(&self, args: &[&str]) -> Result<(), String> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        xtask::run(&self.root, &args).map_err(|e| xtask::render_error(&e))
    }

    /// Makes the tree a git repo, if it is not one yet, and stages every file.
    fn stage(&self) {
        if !self.root.join(".git").exists() {
            self.git(&["init", "-q"]);
        }
        self.git(&["add", "-A"]);
    }

    fn ok(&self, args: &[&str]) {
        if let Err(err) = self.xtask(args) {
            panic!("{args:?} failed: {err}");
        }
    }

    /// Runs the command, expects a failure, and returns stderr.
    fn fails(&self, args: &[&str]) -> String {
        match self.xtask(args) {
            Ok(()) => panic!("{args:?} should fail"),
            Err(err) => err,
        }
    }
}

#[test]
fn ledger_writes_every_id_with_its_clause_and_checks_clean() {
    let repo = Repo::new();
    let text = std::fs::read_to_string(repo.root.join("conformance/ledger.json")).unwrap();
    let ledger: serde_json::Value = serde_json::from_str(&text).unwrap();
    let ids = ledger["ids"].as_array().unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0]["id"], "conf::en_1_a");
    assert_eq!(ids[0]["clause"], "EN-1");
    assert_eq!(ids[1]["clause"], "EN-2");
    assert_eq!(ids[1]["section"], "1. Section");
    repo.ok(&["ledger", "--check"]);
}

#[test]
fn ledger_is_deterministic() {
    let repo = Repo::new();
    let first = std::fs::read(repo.root.join("conformance/ledger.json")).unwrap();
    repo.ok(&["ledger"]);
    assert_eq!(
        first,
        std::fs::read(repo.root.join("conformance/ledger.json")).unwrap()
    );
}

#[test]
fn ledger_check_fails_on_a_stale_ledger() {
    let repo = Repo::new();
    write(
        &repo.root,
        "frozen/final6/hc.md",
        &format!("{CONTRACT}| EN-3 | text | `conf::en_3_c` |\n"),
    );
    let err = repo.fails(&["ledger", "--check"]);
    assert!(err.contains("out of date"), "{err}");
}

#[test]
fn ledger_check_fails_on_an_id_with_no_transcript_and_not_pending() {
    let repo = Repo::new();
    write(&repo.root, "conformance/pending.txt", "");
    let err = repo.fails(&["ledger", "--check"]);
    assert!(err.contains("conf::en_2_b: no transcript"), "{err}");
}

#[test]
fn ledger_check_fails_on_a_transcript_with_an_unknown_id() {
    let repo = Repo::new();
    write(
        &repo.root,
        "conformance/hc/conf::zz.json",
        &TRANSCRIPT.replace("en_1_a", "zz"),
    );
    let err = repo.fails(&["ledger", "--check"]);
    assert!(err.contains("'conf::zz' is not in the ledger"), "{err}");
}

#[test]
fn ledger_check_fails_when_the_file_name_differs_from_the_id() {
    let repo = Repo::new();
    std::fs::rename(
        repo.root.join("conformance/hc/conf::en_1_a.json"),
        repo.root.join("conformance/hc/other.json"),
    )
    .unwrap();
    let err = repo.fails(&["ledger", "--check"]);
    assert!(err.contains("the file name must equal the id"), "{err}");
}

#[test]
fn ledger_check_fails_when_pending_lists_an_id_that_has_a_transcript() {
    let repo = Repo::new();
    write(
        &repo.root,
        "conformance/pending.txt",
        "conf::en_1_a\nconf::en_2_b\n",
    );
    let err = repo.fails(&["ledger", "--check"]);
    assert!(err.contains("'conf::en_1_a' has a transcript"), "{err}");
}

#[test]
fn ledger_check_skips_engine_vectors_and_fixtures() {
    let repo = Repo::new();
    write(&repo.root, "conformance/engine-vectors/m.json", "{}");
    write(&repo.root, "conformance/hc/fixtures/base.json", "{}");
    repo.ok(&["ledger", "--check"]);
}

#[test]
fn the_replacement_map_is_not_a_transcript() {
    let repo = Repo::new();
    write(
        &repo.root,
        "conformance/replacement-map.json",
        r#"{"version":1,"ids":[]}"#,
    );
    write(
        &repo.root,
        "README.md",
        "<!-- real-only:begin (generated by `cargo xtask real-only`; do not edit) -->\n<!-- real-only:end -->\n",
    );
    repo.ok(&["ledger", "--check"]);
    repo.ok(&["real-only"]);
}

#[test]
fn ledger_check_reads_ui_vectors_as_transcripts() {
    let repo = Repo::new();
    write(
        &repo.root,
        "crates/botster-ui-contract/vectors/conf::zz.json",
        r#"{"id":"conf::zz"}"#,
    );
    let err = repo.fails(&["ledger", "--check"]);
    assert!(err.contains("'conf::zz' is not in the ledger"), "{err}");
}

#[test]
fn ledger_check_fails_when_pending_grows_against_a_base() {
    let repo = Repo::new();
    commit(&repo);
    repo.ok(&["ledger", "--check", "--base", "HEAD"]);
    std::fs::remove_file(repo.root.join("conformance/hc/conf::en_1_a.json")).unwrap();
    write(
        &repo.root,
        "conformance/pending.txt",
        "conf::en_1_a\nconf::en_2_b\n",
    );
    let err = repo.fails(&["ledger", "--check", "--base", "HEAD"]);
    assert!(err.contains("may only shrink"), "{err}");
}

#[test]
fn ledger_check_allows_pending_to_gain_an_id_the_ledger_gained_since_the_base() {
    let repo = Repo::new();
    commit(&repo);
    // An accepted amendment adds a clause with a new id: the ledger gains it, and so may pending.
    write(
        &repo.root,
        "frozen/final6/hc.md",
        &format!("{CONTRACT}| EN-3 | text | `conf::en_3_c` |\n"),
    );
    repo.ok(&["ledger"]);
    write(
        &repo.root,
        "conformance/pending.txt",
        "conf::en_2_b\nconf::en_3_c\n",
    );
    repo.ok(&["ledger", "--check", "--base", "HEAD"]);
    // An id that the base ledger already had may still never come back.
    std::fs::remove_file(repo.root.join("conformance/hc/conf::en_1_a.json")).unwrap();
    write(
        &repo.root,
        "conformance/pending.txt",
        "conf::en_1_a\nconf::en_2_b\nconf::en_3_c\n",
    );
    let err = repo.fails(&["ledger", "--check", "--base", "HEAD"]);
    assert!(err.contains("'conf::en_1_a' is new since HEAD"), "{err}");
    assert!(!err.contains("conf::en_3_c"), "{err}");
}

#[test]
fn ledger_check_refuses_an_old_id_when_the_base_has_no_pending_file() {
    let repo = Repo::new();
    // A base after Phase 0: every id has a transcript and pending.txt is gone.
    write(
        &repo.root,
        "conformance/hc/conf::en_2_b.json",
        &TRANSCRIPT
            .replace("en_1_a", "en_2_b")
            .replace("EN-1", "EN-2"),
    );
    std::fs::remove_file(repo.root.join("conformance/pending.txt")).unwrap();
    commit(&repo);
    std::fs::remove_file(repo.root.join("conformance/hc/conf::en_1_a.json")).unwrap();
    write(&repo.root, "conformance/pending.txt", "conf::en_1_a\n");
    let err = repo.fails(&["ledger", "--check", "--base", "HEAD"]);
    assert!(err.contains("'conf::en_1_a' is new since HEAD"), "{err}");
}

// ---- deferred.txt and withdrawn.txt (Core A6-2, A9-2)

const DEFERRED: &str = "conf::en_2_b  waits  (HC EN-2)  until: the release that ends it\n";

/// Commits the whole tree as the base: one init, one add, one commit.
fn commit(repo: &Repo) {
    repo.stage();
    repo.git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        "base",
    ]);
}

/// The test repo with `conf::en_2_b` moved from pending to deferred.
fn deferred_repo() -> Repo {
    let repo = Repo::new();
    write(&repo.root, "conformance/pending.txt", "");
    write(&repo.root, "conformance/deferred.txt", DEFERRED);
    repo
}

#[test]
fn a_deferred_id_is_a_valid_state_and_a_missing_file_is_empty() {
    deferred_repo().ok(&["ledger", "--check"]);
}

#[test]
fn ledger_check_fails_when_an_id_is_in_two_states() {
    let repo = deferred_repo();
    write(&repo.root, "conformance/pending.txt", "conf::en_2_b\n");
    let err = repo.fails(&["ledger", "--check"]);
    assert!(
        err.contains("conf::en_2_b: is in more than one state: pending.txt, deferred.txt"),
        "{err}"
    );
    let repo = deferred_repo();
    write(
        &repo.root,
        "conformance/withdrawn.txt",
        "conf::en_1_a  withdrawn by HC EN-1  replaced by conf::en_2_b\n",
    );
    let err = repo.fails(&["ledger", "--check"]);
    assert!(
        err.contains("conf::en_1_a: is in more than one state: a transcript, withdrawn.txt"),
        "{err}"
    );
}

#[test]
fn ledger_check_fails_on_a_deferred_or_withdrawn_id_that_is_not_in_the_ledger() {
    let repo = deferred_repo();
    write(
        &repo.root,
        "conformance/deferred.txt",
        &format!("{DEFERRED}conf::zz  waits  (HC EN-2)  until: x\n"),
    );
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("deferred.txt: 'conf::zz' is not in the ledger"));
    write(&repo.root, "conformance/deferred.txt", DEFERRED);
    write(
        &repo.root,
        "conformance/withdrawn.txt",
        "conf::zz  withdrawn by HC EN-1  replaced by none\n",
    );
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("withdrawn.txt: 'conf::zz' is not in the ledger"));
}

#[test]
fn ledger_check_fails_when_an_authority_has_no_row_of_its_contract() {
    // The clause exists (HC EN-2), but only in the HC contract: another contract's clause of the same name cannot authorize the record.
    let repo = deferred_repo();
    write(
        &repo.root,
        "conformance/deferred.txt",
        &DEFERRED.replace("(HC EN-2)", "(Core EN-2)"),
    );
    let err = repo.fails(&["ledger", "--check"]);
    assert!(
        err.contains("cites the authority 'Core EN-2', which is no clause of that contract"),
        "{err}"
    );
    write(
        &repo.root,
        "conformance/deferred.txt",
        &DEFERRED.replace("(HC EN-2)", "(HC EN-99)"),
    );
    assert!(repo.fails(&["ledger", "--check"]).contains("'HC EN-99'"));
    let repo = Repo::new();
    write(
        &repo.root,
        "conformance/withdrawn.txt",
        "conf::en_2_b  withdrawn by Core EN-2  replaced by none\n",
    );
    write(&repo.root, "conformance/pending.txt", "");
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("withdrawn.txt: 'conf::en_2_b' cites the authority 'Core EN-2'"));
}

#[test]
fn ledger_check_fails_on_a_withdrawn_replacement_that_is_not_a_ledger_id_or_is_itself() {
    let repo = Repo::new();
    write(&repo.root, "conformance/pending.txt", "");
    write(
        &repo.root,
        "conformance/withdrawn.txt",
        "conf::en_2_b  withdrawn by HC EN-2  replaced by conf::nope\n",
    );
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("the replacement 'conf::nope' of 'conf::en_2_b' is not in the ledger"));
    write(
        &repo.root,
        "conformance/withdrawn.txt",
        "conf::en_2_b  withdrawn by HC EN-2  replaced by conf::en_2_b\n",
    );
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("'conf::en_2_b' cannot replace itself"));
    write(
        &repo.root,
        "conformance/withdrawn.txt",
        "conf::en_2_b  withdrawn by HC EN-2  replaced by none\n",
    );
    repo.ok(&["ledger", "--check"]);
    write(&repo.root, "conformance/withdrawn.txt", "conf::en_2_b  withdrawn by HC EN-2  replaced by none\nconf::en_1_a  withdrawn by HC EN-1  replaced by conf::en_2_b\n");
    write(
        &repo.root,
        "conformance/hc/conf::en_1_a.json",
        "{\"id\":\"conf::en_1_a\"}",
    );
    std::fs::remove_file(repo.root.join("conformance/hc/conf::en_1_a.json")).unwrap();
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("is itself withdrawn"));
}

#[test]
fn ledger_check_fails_on_a_malformed_record() {
    let repo = deferred_repo();
    for (file, line, what) in [
        (
            "deferred.txt",
            "conf::en_2_b  waits  until: x\n",
            "not a deferred record",
        ),
        (
            "deferred.txt",
            "conf::en_2_b  waits  (HC EN-2)  until:\n",
            "not a deferred record",
        ),
        (
            "deferred.txt",
            "not-applicable conf::en_1_a  case  (HC EN-1)\n",
            "not a not-applicable record",
        ),
        (
            "withdrawn.txt",
            "conf::en_2_b  withdrawn by HC EN-2\n",
            "not a withdrawn record",
        ),
        (
            "withdrawn.txt",
            "conf::en_2_b  by HC EN-2  replaced by none\n",
            "not a withdrawn record",
        ),
    ] {
        write(
            &repo.root,
            &format!("conformance/{file}"),
            &format!(
                "{}{line}",
                if file == "deferred.txt" { DEFERRED } else { "" }
            ),
        );
        let err = repo.fails(&["ledger", "--check"]);
        assert!(err.contains(what), "{line}: {err}");
        write(
            &repo.root,
            &format!("conformance/{file}"),
            if file == "deferred.txt" { DEFERRED } else { "" },
        );
    }
}

#[test]
fn a_not_applicable_case_leaves_its_id_active() {
    let repo = deferred_repo();
    let case = "not-applicable conf::en_1_a  n-minus-1  (HC EN-1)  because: it has no subject\n";
    write(
        &repo.root,
        "conformance/deferred.txt",
        &format!("{DEFERRED}{case}"),
    );
    repo.ok(&["ledger", "--check"]);
    // The case of an id that is also deferred is a conflict: the id is no longer active.
    write(
        &repo.root,
        "conformance/deferred.txt",
        &format!("{DEFERRED}{}", case.replace("en_1_a", "en_2_b")),
    );
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("stays active and is not deferred or withdrawn"));
    // The case record is not an id state: an id with a case and no transcript, pending or deferred line is still stateless.
    write(&repo.root, "conformance/deferred.txt", case);
    assert!(repo
        .fails(&["ledger", "--check"])
        .contains("conf::en_2_b: no transcript"));
}

// The rules for a line, old or new (the lead's ruling): every line cites a current clause of its own contract; a line added or changed since
// the base cites it too, whether its file is new or already existed; edits and removals of existing lines are allowed under that; a base
// without the files counts as empty, with no exemption; one state per id.

#[test]
fn rule_1_every_line_cites_a_current_clause_even_one_that_the_base_already_had() {
    let repo = deferred_repo();
    commit(&repo);
    // The line is in the base, and it still cites a clause that the current manifest no longer has.
    write(
        &repo.root,
        "conformance/deferred.txt",
        &DEFERRED.replace("(HC EN-2)", "(HC EN-99)"),
    );
    repo.stage();
    repo.git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        "stale authority",
    ]);
    let err = repo.fails(&["ledger", "--check", "--base", "HEAD"]);
    assert!(err.contains("cites the authority 'HC EN-99'"), "{err}");
}

#[test]
fn rule_2_a_line_added_since_the_base_cites_a_clause_whether_its_file_is_new_or_old() {
    let repo = Repo::new();
    commit(&repo);
    // The authority's file (hc.md) already existed at the base: the line is accepted, because the clause exists in the current manifest.
    write(&repo.root, "conformance/pending.txt", "");
    write(&repo.root, "conformance/deferred.txt", DEFERRED);
    repo.ok(&["ledger", "--check", "--base", "HEAD"]);
    // The same line with a clause that no contract has is refused, whatever the age of the files.
    write(
        &repo.root,
        "conformance/deferred.txt",
        &DEFERRED.replace("(HC EN-2)", "(HC EN-99)"),
    );
    assert!(repo
        .fails(&["ledger", "--check", "--base", "HEAD"])
        .contains("cites the authority 'HC EN-99'"));
    // A line whose clause is in a file added since the base is accepted too.
    write(
        &repo.root,
        "frozen/final6/hc2.md",
        "# Doc 2\n\n## 1. Section\n\n| EN-3 | text | `conf::en_3_c` |\n",
    );
    write(&repo.root, "contracts.toml", "dir = \"frozen/final6\"\n[[contract]]\nprefix = \"hc\"\nfiles = [\"hc.md\", \"hc2.md\"]\n[superseded]\nfiles = []\n");
    repo.ok(&["ledger"]);
    write(&repo.root, "conformance/pending.txt", "conf::en_3_c\n");
    write(
        &repo.root,
        "conformance/deferred.txt",
        "conf::en_2_b  waits  (HC EN-3)  until: the release that ends it\n",
    );
    repo.ok(&["ledger", "--check", "--base", "HEAD"]);
}

#[test]
fn rule_3_edits_and_removals_of_existing_lines_are_allowed() {
    let repo = deferred_repo();
    commit(&repo);
    // An edit of the line that the base has.
    write(
        &repo.root,
        "conformance/deferred.txt",
        &DEFERRED.replace("waits", "waits longer"),
    );
    repo.ok(&["ledger", "--check", "--base", "HEAD"]);
    // A removal: the deferral ends and the id has a transcript (it cannot return to pending.txt).
    std::fs::remove_file(repo.root.join("conformance/deferred.txt")).unwrap();
    write(
        &repo.root,
        "conformance/hc/conf::en_2_b.json",
        &TRANSCRIPT
            .replace("en_1_a", "en_2_b")
            .replace("EN-1", "EN-2"),
    );
    repo.ok(&["ledger", "--check", "--base", "HEAD"]);
    // A withdrawn line removed from the file is allowed under the same rules (its id then needs a state).
    let repo = Repo::new();
    write(&repo.root, "conformance/pending.txt", "");
    write(
        &repo.root,
        "conformance/withdrawn.txt",
        "conf::en_2_b  withdrawn by HC EN-2  replaced by none\n",
    );
    commit(&repo);
    std::fs::remove_file(repo.root.join("conformance/withdrawn.txt")).unwrap();
    write(&repo.root, "conformance/pending.txt", "");
    assert!(repo
        .fails(&["ledger", "--check", "--base", "HEAD"])
        .contains("conf::en_2_b: no transcript"));
}

#[test]
fn rule_4_a_base_without_the_status_files_counts_as_empty_with_no_exemption() {
    let repo = Repo::new();
    commit(&repo);
    // The base has neither file. Every line of the new files is checked: one with an unknown contract, and one that another contract's
    // clause cannot authorize.
    write(&repo.root, "conformance/pending.txt", "");
    write(
        &repo.root,
        "conformance/deferred.txt",
        &DEFERRED.replace("(HC EN-2)", "(Core EN-2)"),
    );
    assert!(repo
        .fails(&["ledger", "--check", "--base", "HEAD"])
        .contains("'Core EN-2', which is no clause of that contract"));
    write(&repo.root, "conformance/deferred.txt", DEFERRED);
    repo.ok(&["ledger", "--check", "--base", "HEAD"]);
}

#[test]
fn rule_5_one_state_per_id_holds_against_a_base_too() {
    let repo = deferred_repo();
    commit(&repo);
    write(&repo.root, "conformance/pending.txt", "conf::en_2_b\n");
    let err = repo.fails(&["ledger", "--check", "--base", "HEAD"]);
    assert!(
        err.contains("is in more than one state: pending.txt, deferred.txt"),
        "{err}"
    );
}

#[test]
fn ledger_fails_and_lists_an_id_it_cannot_attribute() {
    let repo = Repo::new();
    write(
        &repo.root,
        "frozen/final6/hc.md",
        "## Section\n\nTests: `conf::loose_id`.\n",
    );
    let err = repo.fails(&["ledger"]);
    assert!(err.contains("conf::loose_id"), "{err}");
}

#[test]
fn a_clause_override_attributes_an_id_and_an_unused_override_fails() {
    let repo = Repo::new();
    write(
        &repo.root,
        "frozen/final6/hc.md",
        "## Section\n\nTests: `conf::loose_id`.\n",
    );
    let toml = std::fs::read_to_string(repo.root.join("contracts.toml")).unwrap();
    write(&repo.root, "contracts.toml", &format!("{toml}[[clause_override]]\nid_prefix = \"conf::loose_\"\nclause = \"X-1\"\nnote = \"n\"\n"));
    repo.ok(&["ledger"]);
    write(&repo.root, "frozen/final6/hc.md", CONTRACT);
    let err = repo.fails(&["ledger"]);
    assert!(err.contains("matches no unattributed id"), "{err}");
}

#[test]
fn an_id_in_two_files_has_one_row_owned_by_the_first_contract() {
    let repo = Repo::new();
    write(&repo.root, "contracts.toml", "dir = \"frozen/final6\"\n[[contract]]\nprefix = \"codec\"\nfiles = [\"codec.md\"]\n[[contract]]\nprefix = \"hc\"\nfiles = [\"hc.md\"]\n[superseded]\nfiles = []\n");
    write(
        &repo.root,
        "frozen/final6/codec.md",
        "| TS-1 | text | `conf::en_1_a` |\n",
    );
    repo.ok(&["ledger"]);
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo.root.join("conformance/ledger.json")).unwrap(),
    )
    .unwrap();
    let row = &ledger["ids"][0];
    assert_eq!(
        (row["contract"].as_str(), row["clause"].as_str()),
        (Some("codec"), Some("TS-1"))
    );
    assert_eq!(row["also_in"], serde_json::json!(["hc.md"]));
}

#[test]
fn an_unclassified_contract_file_fails() {
    let repo = Repo::new();
    write(&repo.root, "frozen/final6/new.md", "x\n");
    let err = repo.fails(&["ledger"]);
    assert!(
        err.contains("new.md is neither current nor superseded"),
        "{err}"
    );
}

#[test]
fn manifest_checks_every_listed_hash() {
    let repo = Repo::new();
    let sha: String = Sha256::digest(CONTRACT.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    write(
        &repo.root,
        "frozen/final6/MANIFEST.md",
        &format!("| Name | File | sha |\n|---|---|---|\n| X | `hc.md` | `{sha}` |\n"),
    );
    repo.ok(&["manifest"]);
    write(&repo.root, "frozen/final6/hc.md", "changed\n");
    let err = repo.fails(&["manifest"]);
    assert!(err.contains("hc.md: sha256"), "{err}");
}

#[test]
fn manifest_fails_on_a_missing_file() {
    let repo = Repo::new();
    let sha = "0".repeat(64);
    write(
        &repo.root,
        "frozen/final6/MANIFEST.md",
        &format!("| X | `gone.md` | `{sha}` |\n"),
    );
    let err = repo.fails(&["manifest"]);
    assert!(err.contains("gone.md"), "{err}");
}

#[test]
fn taint_skips_frozen_and_docs_and_flags_other_tracked_files() {
    let repo = Repo::new();
    write(&repo.root, "frozen/final6/x.md", "banned_name here\n");
    write(&repo.root, "docs/note.md", "banned_name here\n");
    write(
        &repo.root,
        "src/ok.rs",
        "// banned_names is a different word\n",
    );
    repo.stage();
    repo.ok(&["taint"]);
    write(&repo.root, "src/bad.rs", "let a = 1;\nuse banned_name;\n");
    repo.stage();
    let err = repo.fails(&["taint"]);
    assert!(
        err.contains("src/bad.rs:2: banned name 'banned_name'"),
        "{err}"
    );
}

#[test]
fn clause_lint_accepts_known_clauses_and_flags_unknown_ones() {
    let repo = Repo::new();
    write(
        &repo.root,
        "crates/botster-fake-hub/src/lib.rs",
        "// HC EN-1\n/// Clause: HC EN-2, HC EN-1\nfn f() {}\n// HC frames are sent\n",
    );
    repo.ok(&["clause-lint"]);
    write(
        &repo.root,
        "crates/botster-fake-hub/src/more.rs",
        "fn g() {}\n// HC EN-99\n// Core EN-1\n",
    );
    let err = repo.fails(&["clause-lint"]);
    assert!(
        err.contains("more.rs:2: no clause 'EN-99' in contract 'hc'"),
        "{err}"
    );
    assert!(
        err.contains("more.rs:3: no clause 'EN-1' in contract 'core'"),
        "{err}"
    );
}

#[test]
fn clause_lint_ignores_crates_that_are_not_fakes() {
    let repo = Repo::new();
    write(
        &repo.root,
        "crates/botster-hub-contract/src/lib.rs",
        "// HC EN-99\n",
    );
    repo.ok(&["clause-lint"]);
}

#[test]
fn codegen_check_fails_when_the_generated_file_differs() {
    // Two runs of the binary: each one generates every schema, and three runs came within 0.8 s of the 2 s budget on a CI runner
    // (2.003 s on 2026-10-01). A check that passes on clean files is the CI `codegen` job; here the report lists exactly the two bad files.
    let repo = Repo::new();
    repo.ok(&["codegen"]);
    write(&repo.root, "schemas/transcript.schema.json", "{}\n");
    std::fs::remove_file(repo.root.join("schemas/client-step.schema.json")).unwrap();
    let err = repo.fails(&["codegen", "--check"]);
    assert!(
        err.contains("schemas/transcript.schema.json: differs from the generated text"),
        "{err}"
    );
    assert!(
        err.contains("schemas/client-step.schema.json: missing"),
        "{err}"
    );
    let reported = err
        .lines()
        .filter(|l| l.contains("differs from the generated text") || l.contains(": missing"))
        .count();
    assert_eq!(
        reported, 2,
        "only the two changed files are reported: {err}"
    );
}
