# core-p4a-2: the Core route ids that core-p4a-dp and core-p4a-ou left pending, and the A7-1 and A9 ids

Feature `core-p4a-2` (Stage 1 package p4a-routes, follow-up). It adds 24 transcripts to `conformance/core/` and removes their ids from `conformance/pending.txt`. Sixteen of the 40 ids stay pending (see the end).

| Group | Transcribed | Still pending |
|---|---|---|
| the 15 ids of the core-p4a-dp follow-up | 7: `dp_1`, `dp_2_stream_handoff`, `dp_3_output_deflate_inflates_to_the_exact_pty_bytes_in_order`, `dp_3_output_deflate_inflated_payload_is_at_most_the_frame_bound_minus_the_type_byte`, `dp_3_output_deflate_frames_share_no_state`, `dp_5_exception_frames_wait_in_the_route_queue_and_full_queue_stops_input_reads`, `dp_5_input_before_live_is_refused_not_ready` | 8 |
| the 9 ids of core-p4a-ou | 4: the two close-reason matrices, `ou_1`, `ou_11` | 5 |
| the 11 `a7_1_*` ids | 11 | 0 |
| the 5 `a9_*` ids | 2: `a9_3` x2 | 3 |

Every new transcript is `fake: structural` with the ruling R-12 (the frozen FakeCore has no model of the behavior, or a step that it cannot run), except `a7_1_query_deadline_at_1ms_and_at_max_attaches`, which also runs against FakeCore. FakeCore is not changed. Each carries the proof kind of `conformance/replacement-map.json` for its id.

## What the follow-up added to the vocabulary

All are documented in `docs/core-testkit-controls.md` with the clause that needs them and their observer. Each is a measurement or an edge injection at the testkit's boundary (steward ruling R-14.3), never a read of worker state.

- Route-stream measurements (step key `route`): `route_stream_frame` (the type and length of frame number `index`, as the stream carried it), `route_stream_consumed` (bytes the worker took from the stream), `route_stream_written` (bytes the stream accepted from the worker), `route_stream_holders` (live handles of the worker end, and the bytes that went through any other handle), `route_stream_closed` (read at once, with no wait). The WebRTC chunk id uses core-p4c's `datachannel_received`.
- Edge injections: `deflate_choice` (Core A9-3, the per-frame compression choice at the scheduler edge), `webrtc_offer` gained `max_message_size`.
- Two Core driver steps for the DP-5 queue case: `route_flood` (refused `text` frames until the route's `input_blocked` says that the stream takes no more; it binds the count; a flood that has not blocked the stream after its maximum is inconclusive) and `route_expect_refusals` (the next `count` refusals are the ops 1 to `count`, in order; the route boundary gives its own readiness, so the step never infers a missing refusal from the host being idle: on an idle read it pumps the handle once, and a second idle read in a row is inconclusive (`route_idle`); the step's deadline is inconclusive too; a route that closes fails the step; a refusal with the wrong op or reason fails the step). Both have tests in `tests/driver_steps.rs`.
- `route_expect_output` takes `kind` (`output` or `output_deflate`), `eligible_from` and `min_deflate_frames`. With `eligible_from` N, a plain frame is right below N bytes and wrong from N, and a compressed frame that inflates to less than N is wrong (Core A9-3: only an eligible payload is compressed, and an ineligible one never is); `min_deflate_frames` makes the step inconclusive when fewer compressed frames than that were seen: a worker may lawfully split the offered bytes into payloads that are all below the floor, so no eligible payload occurred and the premise is open (a plain frame at or above the floor is still a failure). Each compressed frame is inflated alone by the codec decoder, an independent decoder. The tests are in `botster-hub-conformance/tests/deflated_output.rs`, with a probe of the frame kind that does not depend on the check under test.
- The exact duration form `{"ns": n}` (the lead's decision), read by one function at the attach options (`connect_deadline`, `query_deadline`, `stall_deadline`: it reaches `attach` and `AttachWebRtc` as exactly `Duration::from_nanos(n)`) and at the Duration-valued step arguments (`advance_clock.ms`, `real_wait.ms`, `wait_wake.timeout_ms`, the `ms` of `set_silence_threshold`). Tests show that `{"ns": 500000}` arrives as 500 microseconds and that each step argument takes the exact form.
- The let operation `str` (a number as its decimal string) is from core-p4a-dp.

## Prior art (BUILD.md rule 0)

- Vault: `botster v1 terminal routes terminate at session workers`, `backpressure recovery tests must cover empty and failed snapshot branches`.
- This repo: the drafts of core-p4a-ou (`docs/core-p4a-ou-withheld/`) for the two close-reason matrices and the OU ids (the `SnapshotTooLarge` row uses the A8-2 premise of an uncarriable sequence, as `a8_2_baseline_inside_an_uncarriable_sequence_closes_route_snapshot_too_large` does; the `BadPeer` row reads the wire form through core-p4c's `webrtc_apply_answer` and `datachannel_received`). The step idioms come from core-p1, core-p3, core-p4a-dp, core-p4a-ou and core-p4c.
- botster-core has no route data plane to reuse. Nothing was stolen: no `Stolen-From` commit.

## How the premises are established

- **A compressed frame (A9-3).** `deflate_choice` forces the scheduler edge to compress every eligible payload. The step requires at least one compressed frame and checks the eligibility of each frame it meets.
- **Descriptor ownership (DP-1, DP-2, OU-1, OU-11).** `route_stream_holders` counts the live handles of the stream's worker end, and the bytes through any handle that is not the worker's. The real descriptor proof stays `slow:fd-handoff`.
- **Input before `live` (DP-5).** `route_gate` holds the stream, `route_stream_consumed` shows that the worker took the whole input frame (18 bytes: the length, the type byte, the op and five payload bytes), and `route_stream_written` shows that it sent nothing yet.

## Still pending, and what each needs

Their drafts (transcripts that the reviewer rejected, or that rest on a proof that is not yet available) are in `docs/core-p4a-2-withheld/` and `docs/core-p4a-ou-withheld/`, outside `conformance/`.

- `dp_3_frame_limit_checked_before_allocation`: a boundary observer of allocation, or a proof in the shared codec: a decoder that allocates the declared body and then closes passes a consumed-bytes check.
- `dp_3_output_deflate_over_the_frame_bound_is_invalid_input`: corrupt and over-bound compressed streams given to the receiver (the client decoder), which a Core transcript cannot do; it needs the client-subject boundary.
- `dp_7_detach_completes_after_transport_closed`: a record at the route-transport boundary that compares the close of the transport with the posting of `Completed{Detach}`; reading a flag after the event does not fix their order.
- `ou_9_attach_is_sync_baseline_in_pump`: an observation of the baseline cut through the real model and the transport, to show that `attach` forms no baseline that is written later.
- `dp_9_fairness_bound_holds_under_flood`: scheduler holds that make both routes' frames pending before the admission starts (core-p3's `scheduler_hold` holds operations, not route frames).
- `a3_1_deflate_frame_is_charged_at_its_inflated_size_to_route_queue_bytes` and `a3_1_compressed_repeated_bells_never_exceed_the_count_digits_of_route_queue_bytes`: observed compressed-frame premises, and the charge and count relation, at a permitted boundary (a fixed flood does not fix the queued load, because output framing and source backpressure are open).
- `ou_9_oversize_snapshot_closes_route_typed`: the byte bound of the snapshot of that attach, from a real-format oracle; no sentence fixes a minimum payload size.
- `a9_1_frame_cap_below_the_routes_attached_frame_is_invalid_input` and `a9_1_frame_cap_equal_to_the_attached_frame_attaches`: the attempted route's own `attached` frame length, established without assuming that two serializations of the frame have the same length (the codec fixes the JSON values, not whitespace, so a valid encoder can change the length when the cap changes). They need the length of the frame that the attempt itself would carry.
- `a9_2_every_attached_route_can_carry_the_minimum_output_dropped_frame`: a suite-wide record of every attach (its `attached` frame size and its `route_queue_bytes`), so that the relation applies to every route that the suite attaches.
- `dp_3_exactly_maximal_snapshot_fits_with_the_type_byte` and `dp_3_history_is_paged_within_max_history_page_bytes`: A8-1 gives no equal-size promise between two snapshots, so a size that one attach measured does not fix the size of another. They need a control that fixes the snapshot size of the tested attach itself.
- `dp_5_stalled_route_with_repeated_queries_stays_bounded`: the bound is the worker's queue; a boundary count of the bytes written after the release bounds the delivered frames, but the premise that the queries alone would exceed the bound needs frames of known size.
- `dp_6_tie_by_receive_order`: the order in which the stream edge handed the two routes' bytes to the worker (a boundary record), with both read in one pump so that their receive times are equal.
- `a3_1_plain_output_frame_charge_is_unchanged`: the encoded size of each output frame at the queue boundary, summed against `route_queue_bytes`.

## The test budget

The first gate run of head `02d322ce` failed three `xtask::commands` tests at 2.002 to 2.027 s (limit 2 s), and the next run of the same head passed them at 0.03 to 0.07 s. The logs show the different timings; they do not show the cause, and this note does not state one. The lead's disposition: the test-budget finding is outside core-p4a-2 and the lead splits those three multi-scenario tests on a separate branch; this feature has no over-budget test of its own, and the gate of its exact head passes.
