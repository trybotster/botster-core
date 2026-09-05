# Registry identity: smallest safe transition

Date: 2026-09-04, amended after coordinator constraints. Author: Fable. Read-only preparation. No implementation, no tests started. Source: Core `crates/botster-core-daemon/src/registry.rs` at `93acae3` (unchanged through `5923bf1`).

## Defect

`record_filename` maps every character outside `[A-Za-z0-9_-]` to `_` (`registry.rs:243-255`). Distinct ids share a file. `load`, `load_skip_malformed`, and `remove` trust the path and never compare the stored `session_id` with the requested id. `load_all` trusts each file's content. Worker socket names already use a SHA-256 digest (`worker_process.rs:2159-2168`) and stay unchanged.

## Format

- Filename: `hex(SHA-256(session_id bytes)).json`. SHA-256 is collision-resistant, not injective, so the filename selects a candidate and the stored id decides. Every operation verifies.
- Encoding stays inside `registry.rs`. Add `sha2` to `botster-core-daemon` directly. No public digest helper in Core, and `record_path` stays private. Hosts never learn filenames.
- Record gains `format: u8` with value 2, serialized always. A file without `format`, or with a different value, is unsupported. This is the format discriminator. Stem length is not, because a 64-hex legacy id sanitizes to a 64-hex stem.
- Keep the `.json.tmp` rename in `save`.

## Verification rule

Every operation parses the file it touches and compares `record.session_id` with the identity that selected the file.

- `save`: if the target exists, parse it first. A different stored id, or an unsupported format, refuses the write with `IdentityMismatch`. Only a matching id or an absent file is overwritten. Malformed content at the target is also refused, not overwritten, so corruption is visible.
- `load`: mismatch or unsupported format returns `SessionRegistryError::IdentityMismatch`. Fail closed, never `Ok(None)`.
- `load_skip_malformed`: mismatch or unsupported format returns `Ok(None)`. The file stays.
- `load_all`: skips records whose stored id does not hash to their filename, and skips unsupported formats. Returns records plus counts of skipped foreign, skipped malformed, and unsupported legacy files. Nothing is deleted.
- `remove`: load and verify first. Unlink only on a matching id and supported format. Mismatch or unsupported returns `IdentityMismatch` and leaves the file. Missing file stays `Ok(())`.
- Error payload: `IdentityMismatch` carries the requested id only, which the caller already holds. It never carries the stored foreign id or the path, so public error mapping stays safe. Hosts classify it as a constant string.

## Legacy records

Cold cut: no migration machinery by default. Legacy files stay on disk unread. Explicit rejection happens in two places:

- Exact lookup: `load` and `load_skip_malformed` first check the hashed path. If absent, they check the legacy sanitized path for the same id without a scan. A present legacy file returns `SessionRegistryError::UnsupportedLegacyRecord` from `load` and `Ok(None)` from `load_skip_malformed`. The daemon surfaces the rejection once per id in its adoption scan as a terminal, not adoptable, state. The legacy check is one `exists` call, so exact reads still do not scan the collection.
- Scan: `load_all` reports the legacy count. Callers see that unsupported records exist.

If the user later wants old sessions back, a separate explicit per-id migration can be added. It is not part of this cut.

## Hub caller

Hub `src/update.rs:868-889` `durable_worker_identity` reads `sessions/<session_id>.json` by raw id, and the caller at `:699` skips waiting for worker exit when it gets `None`. Hub already depends on `botster-core-daemon`, and `SessionRegistry` with `load` is public (`botster-core-daemon/src/lib.rs:50`). Replace the direct file read with `SessionRegistry::new(data_directory).load(&SessionId(session_id))` and read `recovery_identity` from the verified record. No `CoreDaemon` is constructed, no filename is learned, and identity is verified. Land this before Hub's Core pin moves, or the update path silently degrades.

## Core tests to move

Tests that build raw `<id>.json` paths or walk `sessions/`: `daemon_integration_test.rs:4797, :6564, :7413, :7845, :657, :760, :885` and `terminal_wake_test.rs:2958, :2991`. They can keep walking the directory when they need file counts. When they need one record's path, they must go through registry behavior, for example by writing a foreign record with `save` under a different id, rather than computing a filename.

## Test set

1. `audit:a` and `audit_a` save to distinct files, both load, `load_all` returns two. Red test for the filename change.
2. Foreign identity at Y's path: `save(Y)` refuses and the file is unchanged; `load(Y)` is `IdentityMismatch`; `load_skip_malformed(Y)` is `None`; `load_all` skips and counts one; `remove(Y)` errors and the file remains. Negative control: revert the check in `remove` and show the foreign file is deleted.
3. Legacy sanitized file present and no hashed file: `load` is `UnsupportedLegacyRecord`, `load_skip_malformed` is `None`, `load_all` counts one legacy, no deletion, and `test_load_all_calls` stays zero for the exact lookups.
4. A legacy record whose id is 64 hex characters is still rejected by the `format` field, not by stem length.
5. Unicode, punctuation, path separators, and a 1,000-character id round-trip through save, load, and remove with a fixed-length filename.
6. Leftover `.json.tmp` is not loaded by `load_all`.
7. `IdentityMismatch` display contains the requested id only and no path.
8. Daemon: spawn with a punctuation id, restart, adopt, remove. Worker socket path unchanged. Adoption scan reports a legacy file as terminal.
9. Hub: `durable_worker_identity` through `SessionRegistry::load`, with one test that a colliding sibling id cannot supply another session's worker identity.

## Ordering

Core registry change with tests 1 to 8 first. Hub switch and test 9 before the Hub pin update.
