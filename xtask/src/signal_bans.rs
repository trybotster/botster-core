//! The pattern rule in every `clippy.toml`: a crate with its own file does not inherit the root list, so each file must ban
//! the raw signal calls itself. `botster_core_sys::signal` is the one way to signal (it refuses a target of 0 or 1 and our
//! own).

use anyhow::Result;

/// The calls that only `botster_core_sys::signal` may make.
pub const SIGNAL_CALLS: [&str; 3] = [
    "rustix::process::kill_process_group",
    "rustix::process::kill_process",
    "rustix::process::kill_current_process_group",
];

/// The signal calls that the `disallowed-methods` list of a `clippy.toml` text does not ban.
pub fn missing(text: &str) -> Result<Vec<&'static str>> {
    let config: toml::Table = text.parse()?;
    let banned: Vec<&str> = config
        .get("disallowed-methods")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("path").and_then(toml::Value::as_str))
        .collect();
    Ok(SIGNAL_CALLS
        .into_iter()
        .filter(|call| !banned.contains(call))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_list_without_a_signal_call_names_it() {
        assert_eq!(missing("# exempt\n").unwrap(), SIGNAL_CALLS);
        let one =
            r#"disallowed-methods = [{ path = "rustix::process::kill_process", reason = "r" }]"#;
        assert_eq!(
            missing(one).unwrap(),
            [
                "rustix::process::kill_process_group",
                "rustix::process::kill_current_process_group"
            ]
        );
        let all = r#"disallowed-methods = [
            "std::thread::spawn",
            { path = "rustix::process::kill_process_group", reason = "r" },
            { path = "rustix::process::kill_process", reason = "r" },
            { path = "rustix::process::kill_current_process_group", reason = "r" },
        ]"#;
        assert!(missing(all).unwrap().is_empty());
        assert!(missing("disallowed-methods = [").is_err());
    }

    #[test]
    fn every_clippy_toml_of_the_repo_bans_the_raw_signal_calls() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let files: Vec<String> = crate::fsutil::walk_files(&root)
            .into_iter()
            .filter(|f| f == "clippy.toml" || f.ends_with("/clippy.toml"))
            .collect();
        assert!(files.len() >= 6, "{files:?}");
        for file in files {
            let text = std::fs::read_to_string(root.join(&file)).unwrap();
            assert!(missing(&text).unwrap().is_empty(), "{file}");
        }
    }
}
