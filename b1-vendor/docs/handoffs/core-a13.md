# Handoff: core-a13 (paused 2026-10-02)

## Goal and clause list

- Feature: core-a13. Core Amendment 13 (manifest final30): clipboard writes and the OSC 5522 acknowledgement. Stage 0 transcripts for 11 ids, plus four fixes to existing P3 transcripts (steward ruling R-28).
- Pin: `~/botster-sessions/pins/foundation-core-a13.0e1b131d.md` (supersedes 63bffd46).
- Text: `frozen/current/core-contract-v1.17-amendment-13-candidate5.md` (A13-1, A13-1b, A13-2).
- Rulings in play: R-5 and R-7 (terminal bytes come from the libghostty model, never from a transcript), R-12 (structural transcripts), R-28 (the four P3 fixes).

The 11 ids (all were in `conformance/pending.txt` on main):

- `a13_1_all_representations_one_event_atomic`
- `a13_1_empty_contents_is_clear_distinct_from_empty_value`
- `a13_1_every_recognized_clipboard_write_is_surfaced`
- `a13_1_selection_from_osc_52_string_or_location`
- `a13_1_too_large_counts_all_representations`
- `a13_1b_acknowledgement_is_contiguous_under_input_pressure`
- `a13_1b_one_acknowledgement_with_a_refusing_client`
- `a13_1b_one_acknowledgement_with_an_answering_client`
- `a13_1b_one_acknowledgement_with_no_client`
- `a13_1b_over_bound_is_io_error`
- `a13_1b_success_with_no_host_and_a_full_event_queue`

## Branch and head

- Branch: `foundation/core-a13`, from main `a8db5c9` (contains await-recheck).
- Head: `8bfc5b7ce82bfa26dbe3dfd1a8558572318fc229`, pushed to origin.
- No gate has run on this head. No READY was sent. No reviewer review of the implementation took place.

## Done (in the head)

1. The four R-28 fixes. Each changed step cites `Core R-28` in its `because`.
   - `conf::in_9_kitty_unknown_flag_bits_ignored`: the program writes `CSI = 63 ; 1 u`. The transcript no longer expects 63. It asserts no `ModesChanged` (`await_quiet`, then `expect_quiet_until` with an `UpdateMetadata` barrier), `terminal_state` modes `kitty_flags: 0`, and legacy encoding for the three following keys (the oracle `modes` are `kitty_flags: 0`).
   - `conf::in_9_kitty_all_32_flag_combinations_pinned`, step 4: the wait for `ModesChanged` is replaced by the same quiet check and a `terminal_state` assertion of flags 0.
   - `conf::in_9_kitty_each_flag`, release with flag 1 (old steps 23 to 28): now `NotWritten(NotReported)`, zero bytes, and `pty_input` equal to the earlier log (`$log2`).
   - `conf::a2_2_unsupported_what_values`, step 18: now `NotWritten(NotReported)` instead of `Unsupported{release_event}`. No replacement-map note named `release_event` as proven (checked with grep).
2. The 11 transcripts, all `fake: structural`, `ruling: R-12`. They were produced by `~/botster-sessions/shared/foundation/handoffs/core-a13-gen.py` (a generator; the JSON files are the truth, and the generator is only an aid). Design:
   - Stimulus: `pty_output` controls with OSC 52, OSC 1337 `Copy=:<b64>` and OSC 5522 write transactions (`type=write`, `type=wdata:mime=<b64>;<b64>`, `type=wdata` commit). The formats were checked against the fork source in `~/botster-sessions/ghostty-p2-vt-core-stage1` (`src/terminal/stream_terminal.zig`, `src/terminal/kitty/clipboard*.zig`, `osc/parsers/iterm2.zig`).
   - Expected `selection`, `contents` and `total_bytes` are the values A13-1 fixes for the stimulus. A `mime` that the model chooses (OSC 52 and 1337) is `{"$type": "string"}`. Multi-representation `contents` use `$set` (the model's order is not fixed). `contents: none` for TooLarge uses `{"$one_of": [{"$absent": true}, null]}`.
   - Acknowledgement ids: the expected acknowledgement bytes come from the new control `oracle_clipboard_ack {session, status}` (`success` is DONE, `io_error` is EIO), bound to `$ack` with `bind_fields`. The assertion is that `pty_input` equals `$ack` exactly (one acknowledgement, the right status). The client variants (answering: route with `answers_queries: true`; refusing: route with `answers_queries: false`) read the raw sequence in Output with `route_expect_output` and check that the PTY log is still exactly one acknowledgement.
   - Contiguity id: a 40-byte paste in flight (`pty_accept` 10), then the OSC 5522 commit, then `pty_accept` 1000. The assertions are that the PTY log has length `len(paste) + len(ack)`, starts with the paste, and ends with `$ack`.
   - Full-queue id: `event_queue: 16`, 20 bells with no poll, the write, then the PTY log equals the acknowledgement before any poll, then `EventsLost`.
   - Over-bound id: `clipboard_bytes: 4`, an 11-byte write. The acknowledgement is `io_error`. The event is `TooLarge`, `total_bytes: 11`, `contents` none.
3. New control `oracle_clipboard_ack` (row in `docs/core-testkit-controls.md`; `CONTROL_ARGS` and `CONTROLS` in `crates/botster-core-conformance/tests/vocabulary.rs`). It is real-only (R-7). The oracle is fed the session's output and replies to its clipboard-write callback with the status. The packet layout, the echoed `id` and the terminator are the model's. Stage 1 must implement it.
4. `conformance/pending.txt` (the 11 ids removed), the README real-only list and `docs/fake-replacement-map.md` were regenerated (`cargo xtask real-only`, `cargo xtask replacement-map`). `cargo xtask ledger --check --base origin/main` passes.

## Left

1. **Lead decision 2026-10-02: option (a): add the A13 ClipboardWrite shape (ClipboardContent{mime, bytes}, contents: Option<Vec<ClipboardContent>>, None for TooLarge) to botster-core-contract in this feature, regenerate schemas with cargo xtask codegen, add a crate test, move ev_3_clipboard_write_surfaced and ev_3_oversize_write_is_reported_not_dropped to `contents` (A13-2); no fake change.**
   - State of the vocabulary test: `cargo nextest run -p botster-core-conformance --features fake` gives 708 passed and 1 failed. The failing test is `vocabulary every_core_transcript_uses_the_vocabulary_of_the_driver`, with 42 problems of one kind: "the event ClipboardWrite has no field 'contents'" and "ClipboardWrite: the contract has no field 'contents' here" (21 steps, two messages each). The cause is that `crates/botster-core-contract/src/event.rs` (variant `ClipboardWrite`, around line 194) and `schemas/core/event.schema.json` still have `bytes: Option<HexBytes>`. Nothing else fails.
   - What to do: in `event.rs`, replace `bytes` with `contents: Option<Vec<ClipboardContent>>` (new type `ClipboardContent {mime: String, bytes: HexBytes}`, with a clause comment `Core A13-1`). `None` is TooLarge. `Some(vec![])` is "clear the destination", distinct from one entry with empty `bytes`, so keep `skip_serializing_if = "Option::is_none"` only on the option. Then run `cargo xtask codegen` (and `--check`), add a crate test (serde round trip of the three shapes: clear, empty value, TooLarge; check how `crates/botster-core-contract/src/tests.rs` tests other events), and fix any other use of the old field (grep says only `event.rs` mentions it).
   - The two old EV-3 transcripts (`conf::ev_3_clipboard_write_surfaced`, `conf::ev_3_oversize_write_is_reported_not_dropped`) still match the old `bytes` field. Move them to `contents` (`[{mime, bytes}]` and none). A13-2 item 2 says EV-3's two tests stand. Their `print_after_input` stimulus is unchanged. The OSC 52 `mime` is the model's, so use `{"$type": "string"}`.
2. Run the gate, then READY. Before every READY: `git fetch origin && git merge origin/main`; resolve generated-file conflicts (pending.txt: union minus ids that have a transcript; the README real-only list: `cargo xtask real-only`; the replacement map: `cargo xtask replacement-map`); `cargo xtask ledger --check --base origin/main`; commit; `botster-gate <worktree>`; check `git merge-base --is-ancestor origin/main HEAD`.
3. Reviewer's stated expectations (pin review, no findings): every expectation must cite the A13 sentence; recognition and acknowledgement bytes need the real oracle (R-5, R-7); a scripted client or a PTY byte count alone cannot prove one acknowledgement, its status, or transaction continuity; an id stays pending if its transcript lacks the boundary proof; the four fixes cite R-28.
4. Points the reviewer may challenge (decide, or move the id to pending with the gap in `docs/core-a13-notes.md`):
   - The "answering client" and "refusing client" ids differ only in the route attach option `answers_queries` (true or false). Client behaviour beyond that has no Core-visible form. The proof is the exact PTY log (one oracle acknowledgement before and after the client reads Output). If the reviewer rejects this, those two ids stay pending, with the gap "a client's side effect and platform refusal are outside Core's boundary".
   - Expected `selection` and `contents` literals are derived from the stimulus bytes (base64 decode, selection letters), as the existing EV-3 transcripts do. Contents could also be taken from an oracle control if the reviewer wants that.
   - The contiguity id leaves no order open: the single acknowledgement must come after the whole paste.
5. Write `docs/core-a13-notes.md` (prior art, premises, the control, the gaps above), as the earlier features did (`docs/core-p4a-2-notes.md` is a model). Nothing was written yet.

## Last review verdict and open findings

- Reviewer: pin review only, no findings. No implementation head was reviewed. Open findings: none.
- Previous features of this pair: core-p4a-2 (CLEAN at 3591b0db), conformance-speed (CLEAN at c4b6cdc0), await-recheck (CLEAN at a8db5c9a, merged as contracts-v0.1.13). The lead confirmed the merges.

## Gotchas

- Main moves every few minutes, so the head must contain `origin/main` at READY. Check `git merge-base --is-ancestor origin/main HEAD` right after the gate, and again right before READY.
- `conformance/pending.txt` conflicts on merges. The `xtask ledger --check` rule is: the file may only shrink against the base, and an id with a transcript must not be in it.
- `xtask::commands` tests have failed the 2 s test limit twice on the Linux gate machine under load (the cause is not verified; a rerun passed).
- Never pipe a gate in a `&&` chain. Run the gate unpiped and read its summary. `botster-gate` needs a clean tracked tree.
- Do not use `--help` with `cargo xtask`. The list of commands is in `xtask/README.md`.
- `post_message` takes `session_uuid` and `payload` (not `to` and `message`).
- Core transcripts never hand-write expected terminal output (R-5, R-7). The `oracle_*` controls are the way to get such values.

## Exact next step for a fresh implementer

1. `git fetch origin && git switch -c foundation/core-a13 --track origin/foundation/core-a13` (head `8bfc5b7`). Read this file and the pin.
2. Do "Left" item 1: the contract crate change, `cargo xtask codegen`, the crate test, the two EV-3 transcripts. Run `cargo nextest run -p botster-core-conformance --features fake` until it is all green.
3. Write `docs/core-a13-notes.md`. Merge main, regenerate the generated files, run the gate, and send READY to the reviewer.
