# Review: registry identity source candidate b767d7d

Date: 2026-09-04. Reviewer: Fable. Source review only, no builds or tests. Candidate `b767d7df4b952325344e95be92c1515962e34858` on `foundation/registry-identity`, base `55d2b53` (Core resize `5923bf1`, which this review leaves independent). Worktree HEAD matches and is clean. Hub consumer patch: working tree at `/private/tmp/botster-registry-consumer.tuGcwH/hub`, base `11facec`, only `src/update.rs` differs.

## Verdict

**Approved as source, with one actionable correction (Finding 1) to land before the test window, and one operational consequence to record (Finding 2).** The Hub consumer patch is approved as source.

## Properties checked against the diff

- Private versioned full-digest filenames. `record_filename` is `v1.` plus 64 lowercase hex of SHA-256 over the id bytes plus `.json` (`registry.rs:311-316`). `record_path`, `record_filename`, `legacy_record_filename`, and the format types are private. `load_entry` is `pub(crate)` for the daemon's paged scan only. The `v1.` prefix contains a dot, which the old sanitizer maps to `_`, so no legacy stem can masquerade; the 64-hex legacy id test proves it.
- Identity before return, overwrite, delete. `load` verifies format then `record.session_id == requested` (`:196-206`). `save` calls `load` first, then verifies any existing temp file (`:177-192`). `remove` calls `load` and unlinks only on `Some` (`:283-288`). `load_all` and `load_entry` verify that the stored id hashes to the entry's filename (`:266-273`).
- Preserved unsupported files. Foreign, unsupported, legacy, and malformed files are never written or unlinked by a refused operation. The unit tests read the bytes back after each refusal.
- Exact operations without scans. Exact paths use one read of the digest path and, on a miss, one `symlink_metadata` on the legacy path (`:290-303`). `test_load_all_calls` stays zero in the legacy, long-id, and unrelated-file tests.
- Paged baseline. `advance_baseline_index` takes the id from `load_entry`'s verified record instead of the filename stem (`daemon.rs:2980-2984`). Malformed entries skip, foreign or unsupported entries become `SourceChanged`. The one-item budget test proves both colliding ids complete within bounded pages with one snapshot identity. Cost note: each index entry now costs one read and parse, and row materialization reads the record again through `load_skip_malformed`. Bounded by the existing item accounting; acceptable.
- Cold cut. No migration, no fallback read. Legacy files are rejected with `UnsupportedFormat` on exact and scan paths, and adoption returns that error without inventing a terminal state (integration test).
- Error safety. `IdentityMismatch` and `UnsupportedFormat` are constant messages with no id or path.
- Serialization. The private envelope adds `format` beside flattened public fields. The reader checks `format` first, then deserializes the public record from the same bytes. The test proves public JSON has no `format` field and that the public type still reads stored bytes.

## Finding 1 (correct before the test window): a malformed temp file blocks the identity forever

`save` reads the temp path and propagates a JSON error (`registry.rs:184-186`). A crash during `fs::write` of `v1.<hash>.json.tmp` leaves partial bytes. Every later `save` for that session then fails with `Json`, so spawn, resize persistence, and lifecycle marks for that id stay broken until manual cleanup. Before this change the temp file was overwritten. The temp path is derived from this identity, so malformed temp bytes cannot be another identity's committed record.

Correction: in `save`, treat `Err(Json(_))` from the temp read as overwritable. Keep refusing `IdentityMismatch` and `UnsupportedFormat` from the temp read. Add one unit test: truncated temp bytes, `save` succeeds, `load` returns the record, temp file gone. The existing temporary-file test keeps its unsupported-format case.

The same shape exists for a malformed current file: `save` and `remove` refuse with `Json` (`malformed_current_files_are_skipped_only_by_tolerant_reads`). That is a deliberate choice, because the identity of malformed bytes cannot be verified. It is acceptable, but see Finding 2.

## Finding 2 (record as a decision): rejection is all-or-nothing and errors name no file

One legacy, foreign, or unsupported `.json` under `sessions/` makes `load_all` fail, so `list` and `adoption_scan` fail entirely, and the paged baseline reports `SourceChanged` on every page. A legacy file for an id blocks `save` for that id, so spawning that id fails. `remove` cannot clear legacy or malformed files. This is the explicit rejection the coordinator directed and is within the cold-cut authorization. The consequence is that an existing install with any old record cannot list sessions until an operator clears the directory, and the errors carry no path by design.

Recommendation, separate from this commit: a daemon-level operator diagnostic that lists offending filenames (filenames, not ids), or a documented instruction to clear the sessions directory on upgrade. Not blocking.

## Tests

The ten unit tests are load-bearing for the properties above; each refusal asserts the bytes on disk afterward. Negative controls are not yet run; the report says so. Two suggested controls in addition to the report's three: restore the old sanitizer only and require `colliding_sanitizer_ids_keep_distinct_records` to fail; remove the temp-file identity check and require the temp-file test to fail.

Integration: `registry_fixture_path` finds fixtures by parsing every `.json` as the public record and comparing equality, which works because the private envelope is a superset. The real-worker collision test bounds its cleanup at five seconds and re-adopts before shutdown on failure. `adoption_rejects_legacy_registry_without_inventing_terminal_state` proves the constraint the coordinator set.

## Hub consumer patch

`durable_worker_identity` now calls `SessionRegistry::new(data_directory).load(&requested)` and reads `recovery_identity` from the verified record (`update.rs:869-883`). No filename knowledge, no `CoreDaemon`. Approved as source.

- `.ok()??` maps every registry error to `None`, so the updater skips waiting for a worker whose record is legacy, foreign, or malformed. That matches the previous behavior on any read failure. Record it.
- The `record.session_id != requested` check is redundant with `load`'s verification. Harmless.
- `Component` and `fs` imports remain used elsewhere. No unused-import risk.
- The tree pins Core `93acae3`, where `SessionRegistry::load` has the same signature, so the patch compiles against both Cores. `durable_worker_identity_never_selects_a_colliding_sibling` should fail against the pinned old Core and pass against the candidate. Run it both ways; that is a free negative control for the Hub side.
