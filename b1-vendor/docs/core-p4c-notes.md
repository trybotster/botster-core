# core-p4c notes: Core transcripts for Stage 1 package p4c (WebRTC routes and performance)

Branch `foundation/core-p4c`. The clause list is `docs/stage1-clauses/p4c-webrtc-perf.txt` of `botster-core` (branch `stage1/plan`): 15 ids, all pending on main. All 15 now have a transcript in `conformance/core` and leave `conformance/pending.txt`. No id of the list already had a transcript. FakeCore is not changed. Every transcript is `"fake": "structural"` with ruling R-12.

## Proof kind of each id (`conformance/replacement-map.json`)

| Proof | Ids |
|---|---|
| `core-testkit+perturb` | `dp_3_datachannel_chunking_matches_tr3` |
| `core-testkit+edge` (route-transport) | `ou_2b_connect_deadline_closes_not_connected` |
| `slow:webrtc` (a real peer) | `dp_2_webrtc_answer_has_all_candidates`, `dp_2_webrtc_fingerprint_mismatch_closes_bad_peer`, `dp_2_webrtc_only_the_terminal_channel_is_accepted`, `dp_2_host_never_reads_the_datachannel`, `dp_3_incomplete_datachannel_frame_is_discarded`, `ou_3_webrtc_progress_is_acknowledged_payload_not_retransmission`, `ou_3_webrtc_send_queue_is_bounded_and_backpressures`, `ou_3_webrtc_stuck_peer_with_retransmissions_is_a_stall`, `dp_10_t2_latency_and_throughput_budgets` |
| `slow:performance-budget` | `dp_10_t1_keystroke_to_pty_latency_budget`, `dp_10_t1_pty_to_client_latency_budget`, `dp_10_t1_throughput_and_byte_identity` |
| `slow:packaged-worker` | `dp_11_hop_counts_match_the_stated_topologies` |

The slow-tier transcripts run only against a real peer or real processes in Stage 1 (BUILD.md "Testing": real-process tests are few and named). The default tier checks them structurally: they parse, they use the Core step vocabulary, every control is documented, and every expectation has a `because`.

## Controls (fourteen rows in `docs/core-testkit-controls.md`)

`datachannel_send`, `datachannel_close`, `datachannel_received`, `datachannel_open`, `webrtc_apply_answer`, `webrtc_answer_candidates`, `webrtc_gathered_candidates`, `host_datachannel_io`, `webrtc_peer_pause`, `webrtc_peer_read`, `webrtc_link_loss`, `webrtc_link_stats`, `measure_route_perf`, `route_hops`. The first three and `webrtc_offer` are route-transport edge controls (a scripted peer, Core A5-1). The rest are slow tier. Every control observes at a boundary (the DataChannel, the link between worker and peer, the host process): none reads worker internals (R-14.3, R-18). No terminal byte is hand-written: the frames of `datachannel_send` are the JSON form of a codec frame, and the output bytes of the OU-3 transcripts are scripted PTY bytes that the route carries raw (DP-3).

## Choices that a reader may question

- **DP-10 asserts no threshold.** The clause fixes the topologies, the same-run baseline, byte identity, losslessness and recorded numbers. It marks `perf.added_latency_p99` (250 us), `perf.throughput_floor` (90 percent) and the 1 GiB transfer as "proposal, pending first measurement". The four transcripts assert `recorded`, `baseline_same_run`, and for a bulk transfer `byte_identical` and `lossless`. The benchmark is `measure_route_perf` on the T1 or T2 topology of the clause. The gate of the proposed numbers is the `perf-budgets` profile, which is non-normative and not in this repo.
- **`dp_2_webrtc_fingerprint_mismatch_closes_bad_peer`.** The host event `RouteClosed{BadPeer}` is the testkit proof. The wire form `route_closed{attach_failed{bad_peer}}` is read from the real peer (steward answer, core-p4a-ou notes, item 3).
- **`dp_3_incomplete_datachannel_frame_is_discarded`** sends one complete frame first, so the observer (`pty_input`) would change if the partial frame were delivered. It expects `RouteClosed{PeerClosed}` after the channel closes (OU-2b, OU-5).
- **`dp_3_datachannel_chunking_matches_tr3`** is the perturbation: two routes with `max_chunk_bytes` 256 and 1024 give the same reassembled `screen` frame (equal digests). It `assume`s that the 256-byte route needed more than one chunk for some frame, so it is inconclusive, never vacuous. The inbound half sends one frame in 1-byte and 7-byte chunks.
- **`ou_2b_connect_deadline_closes_not_connected`** uses a clock that only the transcript moves. A quiet check at half the deadline precedes the close. It does not assert whether the deadline counts from the attach or from the answer: both start at clock 0.
- **OU-3 transcripts** use real time (the slow tier cannot move a real worker's clock): `real_wait` 600 ms five times against a 2000 ms `reader_progress_deadline` (progress by acknowledgment keeps the route open while the link drops packets); a 5000 ms deadline for the stuck peer, so that at least two retransmission timeouts pass; `route_send_buffer_bytes` 4096 (the range minimum). `webrtc_link_stats` gives the premise (retransmissions occurred): in the progress transcript the premise is an `assume`, in the stall transcript a `check`.
- **`dp_2_webrtc_answer_has_all_candidates`** observes that a peer connects from the answer alone (`trickled_candidates` 0) and that the answer carries candidates.

## Prior art (BUILD.md rule 0)

- Vault: `botster v1 terminal routes terminate at session workers` (the worker owns the transport and the peer connection) and `backpressure recovery tests must cover empty and failed snapshot branches`.
- This repo: `conf::dp_2_offer_must_carry_all_candidates` (the `webrtc_offer` control and `AttachWebRtc` step shape), `conf::tp_1_tap_never_back_pressures_a_route_and_overhead_is_measured` (the slow-tier measurement form `recorded`), `conf::ou_3_*` of core-p4a-ou (`pty_output`, the stall and the quiet check by `UpdateMetadata` barrier), `conf::dp_5_*` (`pty_input` as the observer of what reached the PTY), and `conf::ou_2b_handoff_failure_is_reported_as_handoff_failed` (the host event of a failed route).
- botster-core@origin/stage1/plan: only the clause list was read. botster-core has no WebRTC data plane in the old daemon, so nothing was reused and there is no `Stolen-From` commit.
- Hand-rolled with a reason: fourteen controls (above). No existing control observes a DataChannel, a WebRTC link or a DP-10 measurement, and each is an injected edge of the testkit (Core A5-1) or a slow-tier measurement.

## Review round 1 (REJECT e7a5f6a5) and the fixes

1. `dp_3_incomplete_...`: the partial frame is now `bytes` op 1 with payload 62 63 and the last byte withheld (`withhold_final`), so the op and byte 62 are on the channel. The observer `pty_input` stays at 61 after the close.
2. `ou_3_..._send_queue_is_bounded_and_backpressures`: the bound is read at the SCTP send call (`peak_enqueued_beyond_acknowledged_bytes`), and the transcript proves source backpressure with a progressing reader (`pty_output_unread` > 0 while the queue is full), then drain after acknowledgment (`pty_output_unread` 0, all bytes in order).
3. `ou_3_..._progress_is_acknowledged_...`: `forbid_events` on `RouteStalled` is active through the whole interval.
4. `dp_2_webrtc_answer_has_all_candidates`: new control `webrtc_answer_candidates` (gathering complete before the answer, and every address of the worker's sockets for the route in the answer).
5. The nested frame of `datachannel_send` is validated by `stimulus::prepare` (new case in `crates/botster-core-conformance/tests/datachannel_frame.rs`: valid, missing field, unknown frame).
6. `dp_2_webrtc_fingerprint_mismatch_...`: an `assume` that the two fingerprints differ.
7. Every transcript that asserts a PTY write waits for `live_received` first.

## Review round 2 (REJECT e990f202) and the fixes

2. `ou_3_..._send_queue_is_bounded_and_backpressures` proves both parts with the boundary counters of `webrtc_link_stats`, as relations to earlier readings (round 4). The peer first receives `live`, and an `await_control` waits until nothing is enqueued beyond the acknowledged point (the baseline frames are acknowledged). Then the acknowledgment is held, and the counters `e0`, `ack0` are read. After 16 KiB of output, an `await_control` waits until the enqueue beyond the acknowledged point is in `[1, 4096]` (a subject that enqueues nothing never passes: inconclusive). The transcript checks that the enqueue progressed (`e1 > e0`), that the acknowledgment did not move (`ack1 = ack0`), that the enqueue beyond the acknowledged point and its peak are at most `route_send_buffer_bytes`, and that it is below the 16384 bytes of output (the enqueue stopped). After the peer takes 1024 output bytes, the transcript waits for the acknowledgment to change, then for the enqueue total to change, and checks `ack2 > ack1`, `e2 > e1` and the bound again. After release every byte arrives in order. No literal cumulative total is asserted, and no source backpressure (`pty_output_unread`) is asserted: no clause fixes the PTY read size, and `route_queue_bytes` (default 32 MiB) absorbs output first.
4. `dp_2_webrtc_answer_has_all_candidates`: the inventory is `gathered` of `webrtc_gathered_candidates` (the stack's own candidate reports at the peer-connection boundary). The transcript checks that the gathering-complete event precedes the answer, and that the candidates of the answer equal `gathered` as sets.
5. `stimulus::prepare` validates the frame of `datachannel_send` before it substitutes the other arguments, so a structural run (no bindings, unbound route) finds a bad frame. The tests use an unbound route (`boundary.rs`, `datachannel_frame.rs`).
8. `requires.controls` of every transcript now lists the control of each `await_control` step.
