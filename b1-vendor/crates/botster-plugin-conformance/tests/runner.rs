//! `run_test` and `run_test_with` fail a transcript that does not pass. Without this test, a runner function that did nothing would pass every generated test.

use botster_plugin_conformance::fake;

static BAD: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/bad");

#[test]
#[should_panic(expected = "does not validate against")]
fn run_test_fails_a_bad_stimulus() {
    botster_plugin_conformance::run_test(
        "botster-plugin-conformance",
        &BAD,
        "conf::bad_frame",
        &fake::host,
        &botster_conformance::NoSchemas,
    );
}

#[test]
#[should_panic(expected = "does not validate against")]
fn run_test_with_fails_a_bad_stimulus() {
    botster_plugin_conformance::run_test_with(
        "botster-plugin-conformance",
        &BAD,
        &BAD,
        "conf::bad_frame",
        &fake::host,
        &botster_conformance::NoSchemas,
    );
}
