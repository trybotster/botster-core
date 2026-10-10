//! What the host and the worker of a route agree on without a message (Core OU-1).

use botster_core_contract::prelude::SnapshotFormat;

/// The `terminal_format` name of a snapshot format (OU-1, Codec TS-1): the lowercase format name, its version, and `+raw`
/// for raw `output` bytes, as in the codec's example `ghostsnp/2+raw`. The host refuses an attach with no common name, and
/// the worker announces the name that it picked, so both use this one function.
pub fn terminal_format(format: &SnapshotFormat) -> String {
    format!(
        "{}/{}+raw",
        format.name.to_ascii_lowercase(),
        format.version
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_name_is_lowercase_with_its_version_and_raw_output() {
        let format = SnapshotFormat {
            name: "GHOSTSNP".into(),
            version: 2,
        };
        assert_eq!(terminal_format(&format), "ghostsnp/2+raw");
    }
}
