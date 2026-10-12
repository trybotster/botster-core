# Prior art: codec fixes, test tooling and adopted tools (feature `prior-art-tooling`)

Owner: the prior-art-tooling pair. Date: 2026-10-01. BUILD.md "Starting fresh" rule 0 requires this note. It answers design section 12b ("Changes this pass makes") and `docs/tooling.md`.

## Sources read

Pinned, read-only:
- botster-core @ 72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c: `crates/botster-terminal-protocol/src/frame.rs` (an immutable frame in a shared buffer) and `tests/public_api.rs` (an allowlist of public items).
- botster-hub @ 9bc6cd7d4963ae5cccd642931619b9cac5c8daac: `crates/botster-hub-test-support/src/isolated_hub.rs` (process census and group teardown).
- The vault: "argv marker censuses cannot see zombie survivors".
- This repo: `crates/botster-plugin-contract/src/props.rs` (the proptest, test-strategy and bolero pattern), `xtask/src/test_budget.rs` (the leftover census before this feature).

## Reused

| What | Where | Trailers |
|---|---|---|
| The shared immutable frame idea, as `ToClient::encode_shared` over `bytes::Bytes` | commit ef21fdb | `Stolen-From: botster-core@72b2e33…:crates/botster-terminal-protocol/src/frame.rs`, `For-Clause: BP-2, BP-3`. The idea only: no code and no scheme-2 header is copied. |
| The allowlist-test idea | `crates/botster-route-codec/tests/public_api.rs` | None. The scanner is new; the old test is not ported. |
| The property and fuzz pattern | `crates/botster-route-codec/src/props.rs` | None. Same repo, same tools (docs/tooling.md). |

## Rejected, and why

- **Copying the old frame type.** Its scheme-2 header, `Arc<[u8]>` body and kind table do not fit rev36 (no header, JSON bodies, per-type bounds, the ignorable bit). `bytes::Bytes` gives the same sharing and O(1) slices, so a payload decoded from a shared buffer is a slice of it.
- **`Arc<[u8]>` for payloads.** It cannot slice. A decode would copy, which is what this feature removes.
- **Porting old tests, or the Hub harness `isolated_hub.rs`.** It sleep-polls and has test switches, which BUILD.md Testing rules 5 and 8 forbid. Nothing is ported; the census idea is rewritten from what `ps` reports.
- **A derive framing crate (deku, binrw), cargo-fuzz alone, insta.** Rejected in docs/tooling.md.
- **A second inflate in the receiver.** The decoder keeps what it inflated to validate an `output_deflate` frame (`ToClient::inflated`), so no consumer inflates again.

## Hand-rolled, with reason

- **The process census and `Driver` (`crates/botster-test-support`).** They call `ps` and `kill` through `std::process`. The workspace forbids `unsafe`, and `libc` would add a dependency for signals that `kill(1)` sends. No maintained crate gives a "processes of one test run, including a `setsid` escapee and zombies" census. The rules are in `census::update_owned`, and `tests/slow.rs` holds the real-process controls.
- **The temp root.** `tempfile` returns `$TMPDIR`-based paths (about 57 bytes on macOS after `/var` is canonicalized), which leave too little room for a Unix socket (limit 103 bytes). The root is `/tmp/bt<pid>-<n>`, canonicalized.
- **The public-API scanner (`tests/public_api.rs`).** `cargo public-api` needs nightly rustdoc JSON, and the default tier is stable. The scan reads rustfmt's layout; a negative control proves that an unlisted root item, root re-export, method and prelude name are found.
- **Typed `DecodeDetail`.** `thiserror` would only generate `Display`. The enum keeps one code per variant in one `match`, which the property tests check.

## Adopted (no hand-rolling)

proptest 1.11 and test-strategy 0.4.5 (generators derived from the types, validated against the checked-in schemas with `jsonschema`), bolero 0.13.6 (one harness for the route frame decoder), cargo-mutants 27.1.0 (landing `--in-diff`, weekly on fakes and codecs). Commands are in the README "Building".

## Findings that the tools made

A full `cargo mutants -p botster-route-codec` run on 2026-10-01 had 168 survivors in code this feature does not change (base64 and hex helpers, `file_component` (TS-5f), `parse_op`, `OutputDropped::fit`, `Input::check` branches, `unknown_value`, strict-JSON bounds, schema helpers). The weekly workflow reports them. Killing them is a separate change; no `exclude_re` entry was added.
