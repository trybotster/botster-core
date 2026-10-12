//! The bolero harness of the host's cursor ordinal counter row (Core ST-7; steward ruling R-101 Q6; BUILD.md "Adopted
//! tools"; plan section 8 step 9). `Start` decodes the row's bytes from the registry.
//!
//! `ordinal_decoder` runs as a property test on stable (`cargo test`) and as a fuzzer at landing:
//! `cargo +<nightly> bolero test -p botster-core-host --test props ordinal_decoder -T 60s`.
//! A crash input is committed under `__fuzz__/` as a regression case.

use botster_core_host::session::next_ordinal;

/// Cases of the default tier. The seed is pinned by `cargo xtask` through `BOLERO_RANDOM_SEED` (bolero has no API for
/// it).
const CASES: usize = 256;

/// For arbitrary row bytes: the decoder never panics; a row decodes exactly when it is a decimal `u64`, and then the
/// next ordinal is one more, at most `u64::MAX`; the row that `Start` writes back (the ordinal in decimal) decodes to
/// the ordinal after it, so the counter never repeats an ordinal below `u64::MAX`.
#[test]
fn ordinal_decoder() {
    bolero::check!()
        .with_iterations(CASES)
        .for_each(|bytes: &[u8]| {
            let parsed = std::str::from_utf8(bytes)
                .ok()
                .and_then(|text| text.parse::<u64>().ok());
            let next = next_ordinal(Some(bytes));
            assert_eq!(next, parsed.map(|last| last.saturating_add(1)));
            if let Some(next) = next {
                assert!(next >= 1);
                let row = next.to_string();
                assert_eq!(
                    next_ordinal(Some(row.as_bytes())),
                    Some(next.saturating_add(1))
                );
            }
        });
}

/// No row is the first ordinal, 1.
#[test]
fn no_row_is_the_first_ordinal() {
    assert_eq!(next_ordinal(None), Some(1));
}

// This test binary was built in the worktree where it runs (bug catalog B13).
botster_test_process::built_in_test!();
