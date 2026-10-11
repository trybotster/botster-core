//! The known-flaky list `ci/known-flaky.toml` (flake guard PR 2; lead, 2026-10-11): each test that fails at random on v1
//! with a diagnosed cause and an open fix. The mutation stages do not run a listed test, so the test cannot catch a mutant
//! by its flake. The slow step and the stage-2 baseline still run it: the list hides no failure of an unmutated run.
//!
//! Each entry names the test (`binary`, the nextest binary id, and `test`, its exact name), the `tier` whose mutation
//! stage builds its binary (`default`: stage 1; `slow`: stage 2), the `diagnosis`, the `owner` of the fix, and the pull
//! request that removes the entry (`removed_by`, `#<number>`). nextest refuses a `binary_id` that matches no binary of the
//! run, so an entry with the wrong tier fails its stage. The `lists` step refuses an entry with a missing, empty or unknown
//! field, a duplicate, and a name with a character outside `[A-Za-z0-9_:/-]`, which a filterset needs no escape for. The
//! Stage 1 exit needs the list empty.

use std::collections::BTreeSet;

/// The list file, from the repository root.
pub const FILE: &str = "ci/known-flaky.toml";

/// The tier whose mutation stage builds a listed test's binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Stage 1: the default features.
    Default,
    /// Stage 2: the slow features.
    Slow,
}

/// One known-flaky test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub binary: String,
    pub test: String,
    pub tier: Tier,
    pub diagnosis: String,
    pub owner: String,
    pub removed_by: String,
}

const FIELDS: [&str; 6] = ["binary", "test", "tier", "diagnosis", "owner", "removed_by"];

/// A name that a filterset `=` matcher takes as it is: not empty, only `[A-Za-z0-9_:/-]`.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '/' | '-'))
}

/// The entries of the list file.
///
/// # Errors
/// The text is not TOML, it has a key other than `flaky`, or an entry breaks a rule of the module documentation. The error
/// names the entry by its position (from 1).
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("{FILE} is not TOML: {e}"))?;
    if let Some(key) = table.keys().find(|key| *key != "flaky") {
        return Err(format!("{FILE}: unknown key `{key}`"));
    }
    let Some(entries) = table.get("flaky") else {
        return Ok(Vec::new());
    };
    let entries = entries
        .as_array()
        .ok_or_else(|| format!("{FILE}: `flaky` is not an array of tables"))?;
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let at = format!("{FILE}: entry {}", index + 1);
        let entry = entry
            .as_table()
            .ok_or_else(|| format!("{at} is not a table"))?;
        if let Some(key) = entry.keys().find(|key| !FIELDS.contains(&key.as_str())) {
            return Err(format!("{at}: unknown field `{key}`"));
        }
        let field = |name: &str| -> Result<String, String> {
            let value = entry
                .get(name)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{at}: no `{name}` string"))?;
            if value.trim().is_empty() {
                return Err(format!("{at}: `{name}` is empty"));
            }
            Ok(value.to_string())
        };
        let binary = field("binary")?;
        let test = field("test")?;
        for (name, value) in [("binary", &binary), ("test", &test)] {
            if !plain_name(value) {
                return Err(format!(
                    "{at}: `{name}` {value:?} has a character outside [A-Za-z0-9_:/-]"
                ));
            }
        }
        let tier = match field("tier")?.as_str() {
            "default" => Tier::Default,
            "slow" => Tier::Slow,
            other => {
                return Err(format!(
                    "{at}: `tier` {other:?} is not \"default\" or \"slow\""
                ))
            }
        };
        let removed_by = field("removed_by")?;
        let number = removed_by.strip_prefix('#').unwrap_or_default();
        if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "{at}: `removed_by` {removed_by:?} is not a pull request `#<number>`"
            ));
        }
        if !seen.insert((binary.clone(), test.clone())) {
            return Err(format!("{at}: {binary} {test} is listed twice"));
        }
        parsed.push(Entry {
            binary,
            test,
            tier,
            diagnosis: field("diagnosis")?,
            owner: field("owner")?,
            removed_by,
        });
    }
    Ok(parsed)
}

/// The filterset of the listed tests of `tier`, or `None` when the tier has none.
pub fn tests_of(entries: &[Entry], tier: Tier) -> Option<String> {
    let terms: Vec<String> = entries
        .iter()
        .filter(|entry| entry.tier == tier)
        .map(|entry| format!("(binary_id(={}) & test(={}))", entry.binary, entry.test))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" | "))
}

/// The filter of a mutation stage of `tier`: `filter` (or every test) without the listed tests of the tier.
pub fn stage_filter(entries: &[Entry], tier: Tier, filter: Option<&str>) -> Option<String> {
    match (filter, tests_of(entries, tier)) {
        (filter, None) => filter.map(str::to_string),
        (None, Some(listed)) => Some(format!("all() - ({listed})")),
        (Some(filter), Some(listed)) => Some(format!("({filter}) - ({listed})")),
    }
}

/// The lines that a mutation stage of `tier` prints for its listed tests: each one with its diagnosis, owner and removing
/// pull request.
pub fn report(entries: &[Entry], tier: Tier) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| entry.tier == tier)
        .map(|entry| {
            format!(
                "known flaky, not run: {} {} ({}; owner {}; removed by {})",
                entry.binary, entry.test, entry.diagnosis, entry.owner, entry.removed_by
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = r##"
[[flaky]]
binary = "botster-core::slow_conformance"
test = "conf::ev_5_x"
tier = "slow"
diagnosis = "R-63"
owner = "P3"
removed_by = "#243"
"##;

    fn entry(binary: &str, test: &str, tier: Tier) -> Entry {
        Entry {
            binary: binary.into(),
            test: test.into(),
            tier,
            diagnosis: "d".into(),
            owner: "o".into(),
            removed_by: "#1".into(),
        }
    }

    #[test]
    fn an_entry_is_read_with_each_field_and_an_empty_file_has_none() {
        assert_eq!(
            parse(ENTRY),
            Ok(vec![Entry {
                binary: "botster-core::slow_conformance".into(),
                test: "conf::ev_5_x".into(),
                tier: Tier::Slow,
                diagnosis: "R-63".into(),
                owner: "P3".into(),
                removed_by: "#243".into(),
            }])
        );
        assert_eq!(parse("# none\n"), Ok(vec![]));
        assert_eq!(
            parse(&ENTRY.replace("\"slow\"", "\"default\"")).unwrap()[0].tier,
            Tier::Default
        );
    }

    /// Each field must be there and not empty; each rule of the module documentation refuses its entry.
    #[test]
    fn an_entry_that_breaks_a_rule_is_refused() {
        for field in FIELDS {
            let without: String = ENTRY
                .lines()
                .filter(|line| !line.starts_with(&format!("{field} =")))
                .map(|line| format!("{line}\n"))
                .collect();
            assert_eq!(
                parse(&without),
                Err(format!("{FILE}: entry 1: no `{field}` string")),
                "{field}"
            );
            let line = ENTRY
                .lines()
                .find(|line| line.starts_with(&format!("{field} =")))
                .unwrap();
            let empty = ENTRY.replace(line, &format!("{field} = \" \""));
            assert_eq!(
                parse(&empty),
                Err(format!("{FILE}: entry 1: `{field}` is empty")),
                "{field}"
            );
        }
        let cases = [
            (
                ENTRY.replace("removed_by", "fixed_by"),
                "unknown field `fixed_by`",
            ),
            (format!("{ENTRY}\n[other]\n"), "unknown key `other`"),
            (ENTRY.replace("\"slow\"", "\"Slow\""), "`tier` \"Slow\""),
            (ENTRY.replace("\"#243\"", "\"243\""), "`removed_by` \"243\""),
            (ENTRY.replace("\"#243\"", "\"#\""), "`removed_by` \"#\""),
            (
                ENTRY.replace("\"#243\"", "\"#24a\""),
                "`removed_by` \"#24a\"",
            ),
            (
                ENTRY.replace("conf::ev_5_x", "conf::ev_5 x"),
                "`test` \"conf::ev_5 x\" has a character",
            ),
            (
                ENTRY.replace("conf::ev_5_x", "conf::ev_5_x)"),
                "`test` \"conf::ev_5_x)\" has a character",
            ),
            (
                ENTRY.replace("botster-core::slow_conformance", "a,b"),
                "`binary` \"a,b\" has a character",
            ),
            (
                format!("{ENTRY}{ENTRY}"),
                "entry 2: botster-core::slow_conformance conf::ev_5_x is listed twice",
            ),
            (
                "flaky = 1\n".to_string(),
                "`flaky` is not an array of tables",
            ),
            ("flaky = [1]\n".to_string(), "entry 1 is not a table"),
            ("[[flaky]\n".to_string(), "is not TOML"),
        ];
        for (text, error) in cases {
            match parse(&text) {
                Err(found) => assert!(found.contains(error), "{error}: {found}"),
                Ok(entries) => panic!("{error}: {entries:?}"),
            }
        }
    }

    /// A stage removes only its own tier's entries from its filter; with none, its filter is unchanged.
    #[test]
    fn a_stage_filter_removes_exactly_the_listed_tests_of_its_tier() {
        let entries = [
            entry("a::slow_x", "conf::one", Tier::Slow),
            entry("a", "tests::two", Tier::Default),
            entry("b", "three", Tier::Slow),
        ];
        assert_eq!(
            stage_filter(&entries, Tier::Default, None),
            Some("all() - ((binary_id(=a) & test(=tests::two)))".to_string())
        );
        assert_eq!(
            stage_filter(&entries, Tier::Slow, Some("binary(/^slow/)")),
            Some(
                "(binary(/^slow/)) - ((binary_id(=a::slow_x) & test(=conf::one)) | (binary_id(=b) & test(=three)))"
                    .to_string()
            )
        );
        assert_eq!(stage_filter(&entries[..1], Tier::Default, None), None);
        assert_eq!(
            stage_filter(&entries[1..2], Tier::Slow, Some("f")),
            Some("f".to_string())
        );
    }

    #[test]
    fn a_stage_reports_each_listed_test_of_its_tier() {
        let entries = [
            entry("a::slow_x", "conf::one", Tier::Slow),
            entry("a", "tests::two", Tier::Default),
        ];
        assert_eq!(
            report(&entries, Tier::Slow),
            ["known flaky, not run: a::slow_x conf::one (d; owner o; removed by #1)"]
        );
        assert_eq!(
            report(&entries, Tier::Default),
            ["known flaky, not run: a tests::two (d; owner o; removed by #1)"]
        );
    }

    /// The repository's list parses, and it holds only the diagnosed ev_5 flake (lead, 2026-10-11).
    #[test]
    fn the_repository_list_holds_only_the_ev_5_flake() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let entries = parse(&std::fs::read_to_string(root.join(FILE)).unwrap()).unwrap();
        let names: Vec<(&str, &str, Tier)> = entries
            .iter()
            .map(|e| (e.binary.as_str(), e.test.as_str(), e.tier))
            .collect();
        assert_eq!(
            names,
            [(
                "botster-core::slow_conformance",
                "conf::ev_5_repeated_attach_detach_without_polling_stays_bounded",
                Tier::Slow
            )]
        );
    }
}
