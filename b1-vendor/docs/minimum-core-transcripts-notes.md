# minimum-core-transcripts: the two minimum-Core ids that had no transcript

Feature `stage0/minimum-core-transcripts`. The approved minimum-Core set (`~/botster-sessions/shared/core-stage1/minimum-core-draft.md`) names two IN ids with no contracts transcript: `conf::dp_3_frame_limit_checked_before_allocation` and `conf::ou_9_attach_is_sync_baseline_in_pump`. This feature adds both transcripts and removes the ids from `conformance/pending.txt`. Both are `fake: structural` with ruling R-12. FakeCore is not changed.

## Prior art

- The withheld drafts of core-p4a-2 (`docs/core-p4a-2-withheld/`) and the reasons in `docs/core-p4a-2-notes.md` ("Still pending").
- DP-3: the draft checked the bytes that the worker consumed. A decoder that allocates the declared body and then closes passes that check, so it does not prove the order of check and allocation.
- OU-9: the draft checked that nothing is written before the first pump. That does not show that `attach` forms no baseline that is written later.

## `conf::ou_9_attach_is_sync_baseline_in_pump`

New controls (`docs/core-testkit-controls.md`): `hold_handoff` and `release_handoff` at the route-transport edge (Core A5-1, as `fail_handoff`), and `oracle_screen_payload` (libghostty's snapshot loader reads a route's `screen` payload, R-7).

The proof:
1. With the hand-over of the stream endpoint held, `attach` of `r1` returns, and `attach` of `r2` with no pump between them is `RouteLimit` (`routes_per_session` 1). The registration is synchronous (OU-1, OU-9).
2. The route's stream has accepted no byte (`route_stream_written` 0, R-14.3).
3. The program writes bytes, and the model takes them while the worker has no transport. The oracle shows that the screen changed (`$t1` differs from `$t0`). The transcript writes no expected terminal bytes: both texts come from libghostty.
4. After the release, the baseline's screen, read by the oracle, is `$t1`. A baseline formed at the return of `attach` would hold `$t0`.
5. The next output on the route is exactly the next program bytes, so the bytes of step 3 are not sent again (no duplicate after `R`).

The transcript uses `await_quiet` only. It asserts no pump count and no baseline bytes.

## `conf::dp_3_frame_limit_checked_before_allocation`

New controls: `alloc_window` and `alloc_peak`, allowed by ruling R-37 and its follow-up (`docs/steward-rulings.md`).

- **Scope (R-37 follow-up):** "checked before any allocation" covers an allocation for the frame, that is, one whose size depends on the declared length. A buffer whose size does not depend on it is outside DP-3.
- **The proof is differential (the preferred form of the follow-up).** The same stimulus runs twice on a route with `max_frame_bytes` 4096: a length prefix of 4097 (`max_frame_bytes + 1`, the smallest length over the limit, because the length counts the type byte plus the body) and then of `0x7FFFFFFF`, each with the type byte `0x83` and no body. Each run expects the typed close `frame_too_large` and the `RouteClosed` `BadFrame` event. The check is `peak_large <= peak_small`. It has no absolute number, and it fails an allocation that depends on the length, also when it is capped (for example `min(L, 64 MiB)`).
- **Stimulus:** the five bytes are codec framing, not terminal bytes (R-7 does not apply to them).
- **Reproducibility:** the peak is process-wide (R-37). The proof needs the testkit to run this transcript with no other work in the process during the two windows. A concurrent allocation in the second window can make a correct worker fail. A concurrent allocation in the first window can raise `peak_small` and hide a wrong worker.
- **What this PR does not prove:** the contracts workspace has no real worker, and it forbids `unsafe` code, so it cannot hold a `#[global_allocator]`. The observer and the native mutations (allocate `L - 1`, `L / 4` and `min(L, 64 MiB)` before the check; each must fail the check) belong to the Core owner in `botster-core-testkit` (R-37 condition 3), against the real worker at its pinned commit. A subject whose route reader runs in another process answers `unsupported_control`, never a pass.
