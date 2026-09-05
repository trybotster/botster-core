# Registry identity: smallest safe transition

Date: 2026-09-04. Author: Fable. Read-only preparation for the next audit repair. No implementation, no tests started. Source: Core `crates/botster-core-daemon/src/registry.rs` at `93acae3` (unchanged through `5923bf1`).

## Defect

`record_filename` maps every character outside `[A-Za-z0-9_-]` to `_` (`registry.rs:243-255`). Distinct ids share a file. `load`, `load_skip_malformed`, and `remove` trust the path and never compare the stored `session_id` with the requested id. `load_all` trusts each file's content. Worker socket names already use a SHA-256 digest (`worker_process.rs:2159-2168`) and stay unchanged.

## Format

- Filename: `hex(SHA-256(session_id bytes)).json`, 64 hex characters plus extension. Fixed length, injective, path-safe, and independent of the id's characters. Keep the `.json.tmp` rename in `save`.
- Digest helper: add one `pub fn session_identity_digest(&SessionId) -> [u8; 32]` in `botster-core`, which already depends on `sha2`. Hex-encode in the daemon with `format!("{byte:02x}")`, so `botster-core-daemon` gains no dependency. Do not reuse the worker socket derivation; it truncates to 16 bytes and must not change.
- Make `SessionRegistry::record_path(&SessionId)` public. Tests and any host code must obtain paths from it, never build `<id>.json`.

## Verification rule

Every read parses the file, then compares `record.session_id` with the identity that selected the file.

- `load`: mismatch returns a new `SessionRegistryError::IdentityMismatch { expected, found }`. Fail closed, never `Ok(None)`.
- `load_skip_malformed`: mismatch returns `Ok(None)`, same as malformed. The file stays on disk.
- `load_all`: a record whose stored id does not hash to its own filename is skipped and counted. Expose the count through a `RegistryScanReport { records, skipped_foreign, skipped_malformed, legacy }` return, or a second method, so hosts can see it.
- `remove`: load and verify first. Unlink only when the stored id matches. Mismatch returns `IdentityMismatch` and leaves the file. Missing file stays `Ok(())`.
- `save`: no change beyond the new path. A record can only land at its own hash.

## Old records

Cold cut with explicit rejection, no silent fallback. A file whose stem is not 64 hex characters is legacy. `load` never falls back to the legacy path, because that fallback would reintroduce the collision. `load_all` skips legacy files and lists them in the scan report. Nothing deletes them. Provide one explicit `migrate_legacy_record(&SessionId)`: read the legacy path for that id, require `record.session_id == id`, save at the hashed path, then remove the legacy file. A stored id that differs is rejected with `IdentityMismatch` and the legacy file stays. The daemon may call this during `adoption_scan` only for ids the host names explicitly. Do not migrate on a directory walk.

## Callers that break and must move

- Hub `src/update.rs:868-889` `durable_worker_identity` reads `sessions/<session_id>.json` by raw id. After the cut it returns `None` and the update path stops waiting for worker exit. Hub must not learn the filename. Add a Core API on `CoreDaemon` that returns the recovery identity for one session, and switch Hub to it. This is a required Hub follow-up, not optional.
- Core tests that build raw filenames: `daemon_integration_test.rs:4797, :6564, :7413, :7845`, plus the `sessions_dir` walks at `:657, :760, :885` and `terminal_wake_test.rs:2958, :2991`. Switch them to `record_path`.

## Test set

1. `audit:a` and `audit_a` save to distinct files; both load; `load_all` returns two. This is the audit reproduction and the red test for the filename change.
2. Foreign identity: write a record with id X at the path for Y. `load(Y)` is `IdentityMismatch`; `load_skip_malformed(Y)` is `None`; `load_all` skips it and counts one foreign; `remove(Y)` errors and the file remains. Negative control: revert the check in `remove` and show the foreign file is deleted.
3. Legacy file at `<sanitized>.json`: `load` is `None`, `load_all` lists it as legacy and leaves it, `migrate_legacy_record` moves it when the stored id matches and rejects it when it does not.
4. Unicode, punctuation, path separators, and a 1,000-character id round-trip through save, load, and remove with a fixed-length filename.
5. Leftover `.json.tmp` is not loaded by `load_all` (existing extension check).
6. Exact-session reads still do not scan: keep the `test_load_all_calls` assertions.
7. Daemon: spawn with a punctuation id, restart, adopt, remove. Lifecycle journal and worker socket path unchanged.
8. Hub: `durable_worker_identity` through the new Core API, with one test that a colliding sibling id cannot supply another session's worker identity.

## Ordering

Land the Core registry change with tests 1 to 7 first. Land the Hub API switch and test 8 before updating Hub's Core pin, or the update path silently degrades.
