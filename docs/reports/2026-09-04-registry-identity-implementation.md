# Registry identity implementation

Date: 2026-09-04. Implementer: Codex. Coordinator: root Codex. Reviewer: Fable.
Status: source review completed. Focused verification ran in the released window. See `2026-09-04-registry-identity-validation.md` for results and remaining gates.

## Worktree and authority

- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-registry-identity`.
- Branch: `foundation/registry-identity`.
- Base: `55d2b539e40df99488c8119f8f2e318b623eeef2`.
- Core resize source in that base: approved `5923bf1847979e2897796fadb9863183ffa5e3f1`.

I verified that the branch and worktree path did not exist before creating them with `git worktree add`.
The new worktree was clean at the required base before source edits.
The resize worktree remains frozen at `55d2b53`. Its isolated Hub export remains unchanged.
No new agent or pipeline was created. No merge, push, or dependency pin update occurred.

I read Fable's registry preparation and the coordinator's overriding constraints.
The coordinator approved the private filename and format tag before production changes.
The coordinator also approved the necessary paged-baseline consumer correction in `daemon.rs`.
Repository CI owns the Core Cargo commands; Core has no test wrapper.

## Private storage format

The filename is `v1.<64 lowercase SHA-256 hex characters>.json`.
The digest covers the exact UTF-8 bytes of `SessionId`.
The dot in the prefix cannot occur in an old sanitized filename stem.
SHA-256 is collision-resistant, not injective. The digest selects a candidate file; stored identity still decides whether that candidate matches.

A private serializer adds `"format": "botster.session-registry.v1"` beside the flattened record fields.
The public `RegistryRecord` type and its standalone serialization remain unchanged.
The reader checks the private format before deserializing the full record.
The reader uses Serde's typed field checks, including duplicate-field rejection, instead of reconstructing the public record from a JSON value map.
Filename helpers and format types remain private in `registry.rs`.

The daemon crate adds a direct `sha2 = "0.11.0"` dependency, matching the existing locked Core dependency.
The lockfile change adds only that dependency edge. No package version or source changes.
Worker socket identity and public session ID rules remain unchanged.

## Operation rules

| Operation | Behavior |
| --- | --- |
| `save` | Validate the primary record strictly. Replace malformed temporary JSON, but reject foreign or unsupported temporary records. |
| `load` | Validate the format and exact requested identity before returning a record. |
| `load_skip_malformed` | Skip malformed current-file JSON. Propagate identity, format, and I/O errors. |
| `load_all` | Ignore temporary files and malformed current-file JSON. Validate every returned record against its filename. |
| `remove` | Validate the format and exact requested identity before deleting the file. |

`IdentityMismatch` and `UnsupportedFormat` use constant messages. Neither error includes the foreign identity or file path.
Foreign, unsupported, and malformed primary records are preserved when a write or removal is refused.
Malformed temporary JSON can be replaced during crash recovery. Valid foreign and unsupported temporary records remain unchanged.
Collection rejection is all-or-nothing: any legacy, unsupported, or foreign record makes the scan return an error.
The scan does not return a partial list, delete files, or invent terminal lifecycle state.
The public `load_all` return type remains unchanged.

The existing temporary-file rename remains the persistence mechanism.
This change does not add locks or a transaction protocol for concurrent independent writers.
The validation and subsequent write/remove are separate filesystem operations, as in the existing single-owner daemon model.

## Legacy files and long IDs

Exact operations first inspect the current candidate path.
If that file is absent, they probe only the exact old sanitized path and return `UnsupportedFormat` if it exists.
That probe never reads, migrates, overwrites, or deletes the legacy record.
It is an explicit rejection probe, not a compatibility read fallback.
If a valid current file exists, exact operations use it without inspecting unrelated legacy files. Collection scans still reject legacy `.json` entries.

An overlong old filename cannot exist on the filesystem. The legacy probe treats `io::ErrorKind::InvalidFilename` like an absent legacy file.
It propagates all other I/O errors except `NotFound`.
Installed Rust 1.97 source maps Unix `ENAMETOOLONG` to `InvalidFilename`; the error kind is stable since Rust 1.87.
The focused long-ID test requires that filesystem error before verifying save, load, and removal through the bounded current filename.
Public IDs are not shortened, normalized, or restricted.

## Paged lifecycle baseline

`CoreDaemon::advance_baseline_index` previously treated each filename stem as a session ID.
That assumption fails for digest filenames.
It now calls the crate-private `SessionRegistry::load_entry` reader and uses the verified stored ID.
`load_all` shares that reader.
The daemon retains its existing directory iterator, one-entry accounting, elapsed limits, and `SourceChanged` error behavior.
Exact-session operations still do not scan the directory.

The index now reads one record to obtain its identity instead of deriving identity from a filename.
The existing row materialization and copy-on-write freeze remain unchanged.
No public digest or path API was added.

## Source tests

Eleven focused unit tests are in `registry.rs`:

- `audit:a` and `audit_a` retain independent records through save, update, load, list, and removal.
- A known SHA-256 vector verifies the full digest and private namespace. Public record JSON remains unchanged.
- Empty, Unicode, punctuation, separator, and 1,000-character IDs round-trip with a fixed filename length.
- The explicit overlong legacy probe preserves support for long IDs without a scan.
- Foreign record bytes cause identity errors on exact read, tolerant read, save, removal, and scan.
- Missing, wrong, and non-string format tags cause explicit errors and preserve bytes.
- Legacy files, including a 64-hex-character ID, remain untouched and are not migrated.
- Malformed current files are skipped only by tolerant reads and scans; save and removal refuse them.
- Save recovers from incomplete temporary JSON with either an existing primary record or no primary record.
- Scans ignore temporary files. Save preserves valid foreign, unsupported-version, and unversioned temporary records.
- Exact operations succeed despite an unrelated unsupported file and leave the collection-scan counter at zero.

Four daemon integration tests cover:

- Bounded one-item baseline pages retain both colliding-sanitizer IDs and one stable snapshot identity.
- A foreign record causes `SourceChanged` without a returned row or a file mutation.
- Two real workers with colliding-sanitizer IDs retain metadata, process identity, and socket identity through restart and adoption.
- Legacy adoption returns an explicit error without inventing terminal state.

The worker scenario removes the first session, then requires a PTY echo from the surviving sibling before removing it.
It attempts cleanup after an assertion failure. The initial source checked worker, child, and socket absence within five seconds after daemon drop.
Execution found unreaped terminated workers in this restart simulation. The separate correction checks termination and socket absence.
The validation report records the exact failure, correction, and limits of the cleanup evidence.

Existing fixtures now locate already-verified records for fault injection instead of reproducing filenames.
Malformed fixtures first save a supported record, then corrupt its bytes at the discovered fixture path.
The oversized-metadata fixture uses `SessionRegistry::save` to retain the private format while changing metadata.
Directory-permission tests and temporary-file scans retain their existing directory-based setup.

## Required Hub consumer

Hub `src/update.rs::durable_worker_identity` must use existing `SessionRegistry::load` before reading `recovery_identity`.
It must not construct `sessions/<id>.json` or learn the private filename encoding.
This consumer change is required before a Hub Core pin update.
The coordinator owns the separate consumer worktree `/private/tmp/botster-registry-consumer.tuGcwH/hub`, branch `foundation/registry-identity-consumer`, base `11facec`.
I did not edit any Hub worktree.

The coordinator later authorized a separate architecture correction in commit `b17ae2c`.
That document now describes acknowledged resize completion and registry ownership. The registry source commit did not include that document.

## Verification results and remaining gates

The coordinator released the validation window after the source-only hold.
I prepared Ghostty at the declared revision and used Rust 1.97.0 with at most two Cargo jobs.
The focused registry tests and affected daemon tests passed after the test correction in `3f29a8d`.
All three negative controls failed at their intended assertions. I restored each source file before the next command.
Workspace formatting passed. Strict daemon Clippy passed after the separate authorized test lint correction in `d10e57a`.
The validation report preserves the original cleanup and Clippy failures.

Commit `3f29a8d` changes only the new worker test's termination check to the existing `process_has_exited` helper.
Commit `d10e57a` changes only the existing worker adapter test's nested condition to an equivalent match guard.
Neither commit changes registry production source.
The worker test verifies termination and socket removal. A later snapshot verifies PID absence after test-process exit.
These results do not establish that the runtime reaped the workers within the test process.

The exact commands, counts, revisions, source hashes, and evidence hashes are in `2026-09-04-registry-validation-evidence.json`.
The result explanations and cleanup limits are in `2026-09-04-registry-identity-validation.md`.
The corrected Clippy log and final process-state logs are included in that evidence manifest.

Full workspace tests, workspace Clippy, contract-only tests, doc tests, doc generation, Node smoke, and exact Hub consumer validation remain pending.
The coordinator owns those gates. I released the validation window after the focused and affected checks completed.

## Temporary-file recovery follow-up

Fable approved the registry source with one required correction before executable verification.
A crash can leave a malformed `.json.tmp` file. That incomplete file must not permanently prevent saving the same identity.
The follow-up tolerates only `SessionRegistryError::Json` from the temporary-file read.
The primary `load` remains the first save precondition and still propagates malformed-primary errors.
Identity, unsupported-format, and I/O errors from a temporary file still stop the save.
The focused recovery test covers both first-save and replacement-save recovery.
The temporary-file preservation test now distinguishes valid foreign identity from unsupported version and absent format.
The recovery and preservation tests passed during the released validation window.
No diagnostic framework was added. The branch was not rebased or merged with main.
