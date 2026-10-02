//! `cargo xtask public-api [--update]`: the public API of the facade `botster-core` against the checked-in snapshot
//! `api/botster-core.txt` (plan section 8, step 4).
//!
//! The listing is `cargo public-api -s` (blanket impls omitted; auto-trait impls kept, so `impl !Sync for Core` is in the
//! snapshot, Core TH-1). It runs on the pinned nightly, because rustdoc JSON needs one.

use crate::tools::{cargo_nightly, ensure_nightly, require_cargo_tool, NIGHTLY};
use anyhow::{bail, Context, Result};
use std::path::Path;

const SNAPSHOT: &str = "api/botster-core.txt";
const INSTALL: &str = "cargo install cargo-public-api --locked";

/// The first line that differs, as `(line number, snapshot, current)`, or `None` when the texts are equal.
fn first_difference<'a>(snapshot: &'a str, current: &'a str) -> Option<(usize, &'a str, &'a str)> {
    let mut a = snapshot.lines();
    let mut b = current.lines();
    let mut n = 0;
    loop {
        n += 1;
        match (a.next(), b.next()) {
            (None, None) => return None,
            (x, y) if x == y => {}
            (x, y) => {
                return Some((
                    n,
                    x.unwrap_or("<end of snapshot>"),
                    y.unwrap_or("<end of listing>"),
                ))
            }
        }
    }
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    crate::caps::require()?;
    let update = match args {
        [] => false,
        [flag] if flag == "--update" => true,
        _ => bail!("usage: cargo xtask public-api [--update]"),
    };
    ensure_nightly()?;
    require_cargo_tool(root, &["public-api", "--version"], INSTALL)?;
    let out = cargo_nightly(root)
        .args(["public-api", "-s", "-p", "botster-core", "--color", "never"])
        .output()
        .context("run cargo public-api")?;
    if !out.status.success() {
        bail!(
            "cargo public-api failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let current = String::from_utf8(out.stdout)?;
    let path = root.join(SNAPSHOT);
    if update {
        std::fs::create_dir_all(path.parent().context("snapshot directory")?)?;
        std::fs::write(&path, &current)?;
        println!(
            "public-api: wrote {SNAPSHOT} ({} lines, {NIGHTLY})",
            current.lines().count()
        );
        return Ok(());
    }
    let snapshot = std::fs::read_to_string(&path).with_context(|| {
        format!("read {SNAPSHOT}; create it with `cargo xtask public-api --update`")
    })?;
    match first_difference(&snapshot, &current) {
        None => {
            println!("public-api: the facade matches {SNAPSHOT}");
            Ok(())
        }
        Some((line, old, new)) => bail!(
            "the public API of botster-core changed (line {line}: snapshot `{old}`, now `{new}`); review the change, then run `cargo xtask public-api --update`"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_texts_have_no_difference() {
        assert_eq!(first_difference("a\nb\n", "a\nb\n"), None);
    }

    #[test]
    fn the_first_changed_line_is_reported() {
        assert_eq!(
            first_difference("a\nb\nc\n", "a\nx\nc\n"),
            Some((2, "b", "x"))
        );
    }

    #[test]
    fn an_added_or_removed_tail_is_a_difference() {
        assert_eq!(
            first_difference("a\n", "a\nb\n"),
            Some((2, "<end of snapshot>", "b"))
        );
        assert_eq!(
            first_difference("a\nb\n", "a\n"),
            Some((2, "b", "<end of listing>"))
        );
    }
}
