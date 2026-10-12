# core-p7: Core transcripts for the service clauses SV-1 to SV-10, A2-5, A4-1 and A6-1

Feature `core-p7`, Stage 1 package p7-services. It adds 50 transcripts to `conformance/core/` and removes the 50 ids from `conformance/pending.txt`. Nine ids stay pending (see "Open gaps"). `conf::sv_4_required_unavailable_bound_refuses` already had a transcript on main and is not touched.

All 50 are `"fake": "structural"` with the ruling R-12. They run in full against the real Core testkit in Stage 1. FakeCore is not changed, and no transcript claims a pass against it. Every id keeps the proof kind of `conformance/replacement-map.json`: 45 `core-testkit+edge`, and the 5 slow-tier ids below.

## Slow-tier ids (structural here)

| Id | Slow proof | What the transcript asks of the real harness |
|---|---|---|
| `sv_1_inherits_only_listed_descriptors` | `slow:service-process` | `service_fds`: no descriptor beyond 0, 1, 2 and the cause descriptor; fd 0 is `/dev/null` |
| `sv_4_supported_bounds_applied_and_reported` | `slow:service-process` | `service_rlimits`: the soft limits of the real process equal the requested values |
| `sv_8_pdeathsig_where_enforced` | `slow:pdeathsig` | `service_payload_alive` is false after the guardian ends (requires `payload_dies_with_guardian`) |
| `sv_9_sigterm_then_group_kill` | `slow:service-process-groups` | SIGTERM first (`HostStop`, signal 15), then SIGKILL after `stop_grace` (`Killed`, signal 9); the descendant that existed before the stop is dead |
| `a6_1_peer_identity_is_real_process_only` | `slow:real-process-identity` | a real peer with the host's uid connects; a service whose `peer_identity` is another uid has its connection closed |

## Prior art (BUILD.md rule 0)

- Vault: `botster plugin runtime uses supervisor plus per plugin workers`, `botster plugin frame schema owns immutable type and lane bytes` and `botster plugin class selects the lane independently of frame type` (the lane byte and the type byte of a frame are fixed data: used for the `service_send_raw` frames). No vault note covers the Core guardian.
- botster-core@053148f, `crates/botster-core-daemon`: `launch.rs`, `supervisor.rs` and the spawn, stop and rollback part of `host.rs` hold the older service mechanism (the contract lists them as reusable mechanism). No guardian, lane endpoint, preamble or epoch exists there. Nothing was read for behavior, and nothing was reused, so there is no `Stolen-From` commit.
- botster-core@0a8c803 `origin/stage1/plan`, `docs/stage1-plan.md` section 5: the testkit's `ServiceLane` and `Process` edges give the scripted service, the refused spawn and the ended guardian. The controls below are exactly those edge inputs.
- This repo: the step idioms (`await_quiet`, `expect_quiet_until` with a session `Create` as the barrier, `pump_collect`, `await_control`) come from core-p1 and core-p4a-ou. `unavailable_bound`, `withhold_feature` and `fail_next` are FakeCore's existing controls. `service_connect`, `service_exit` and `service_send_frame` were already in `docs/core-testkit-controls.md`.
- Hand-rolled with a reason: the service-lane and process-edge controls that no pair has (`docs/core-testkit-controls.md`). Each is an input or a read-only observer at an edge (Core A5-1, A6-1, steward ruling R-14.3) and cites its clauses. The Core step vocabulary gains three calls (`service_send`, `service_recv`, `service_log_tail`) because the operations are in `CoreApi` and no call named them. 

## How a negative assertion is observed

- A lane connection that Core must close: `service_extra_connection` creates it once and returns its number; `service_connection_closed` reads the end of that one in-memory stream (read-only, so `await_control` may repeat it). The lane's authenticated connection stays open (`service_lane_open`, read after `await_quiet`), and no `LaneEnded` event arrives (`expect_quiet_until` with a `Create` barrier, after `await_quiet`).
- Core sends nothing unsolicited: `await_quiet`, then `service_read` reports the bytes that the scripted service received. The same observer then shows the frame that the host gives, so it would change.
- No wake: `await_quiet`, then the quiet barrier. A drained or released stream is first shown by `service_read` (the frames that left), so the quiet check runs after the state it needs.
- A refused send takes no slot: the transcript opens with `pending_ops: 2` and refuses three sends. A new operation is still admitted (the barrier `Create`).

## Where a clause fixes less than a transcript could assert

- The accounting of the outbound queue (payload, `len` or the whole wire frame) is not fixed. Every send/refuse pair uses sizes where each accounting gives the same answer (two frames of 100 bytes fit in 256, a third does not; the frame of 200 bytes never fits beside a 100-byte frame).
- `LaneEnded.reason` for the lanes that did not fault, and for a length under 2, is not fixed: the transcripts accept any value (the four-value set for a length under 2, any string for the other lane).
- `dropped_unsent_bytes` is asserted between the payload size and the wire size of the frame.
- `InvalidInput` for a `service_id` that is not 32 bytes (A2-1) cannot be written: `ServiceId` is a 32-byte type, so the operation does not parse. `max_frame_bytes` over `2^32 - 1` cannot be written for the same reason (it is a `u32`).
- The lane numbers are `0..lanes`, as the `lanes` list of `ServiceLimits` is indexed.
- `LaneConnected` is not asserted before the commit (SV-6 names only `Reconnected`), and the frame that a service sends before the commit is not asserted after it.

## Open gaps (ids kept in `pending.txt`)

Review rounds 1 and 2 added six ids to the three below.

1. `conf::a2_5_service_state_values`: the value `Lost{EpochExhausted}` has no fixed trigger. SV-6 says "An exhausted epoch counter is `Lost(EpochExhausted)`" and does not say when Core knows it (at the commit of epoch `u64::MAX`, at the next fault, or at a refused reconnect). A transcript that covers the other four values and omits this one would not prove the whole enum. A steward answer on the moment would unblock it.
2. `conf::a2_5_lane_end_reason_values`: `ServiceRemoved` is reachable only when a lane is still connected at `RemoveService`, which A2-1 admits only in `Exited` and `Lost`. No clause says whether the lanes of a `Lost` service stay connected, so no clause fixes a state with a live lane at `Remove`. A steward answer on the lanes of a `Lost` service would unblock it. The other seven reasons are covered by `sv_2_authenticated_lane_faults_end_epoch` (`BadLane`, `Oversize`, `Truncated`, `PeerClosed`), `sv_6_*` and `a6_1_lane_and_epoch_outcomes_match_real_lanes` (`EpochEnded`, `Superseded`); `ServiceExited` is not asserted anywhere, because no clause says that a service exit posts `LaneEnded` for every connected lane.
3. `conf::sv_9_escaped_descendant_reported`: SV-9 fixes `descendants_may_remain = true` only for a live descendant "found after the kill". A descendant that left the group between two enumerations is by definition not found, so no external observer can say that the guardian found it. The proof needs an observer of what the guardian enumerated, or a process edge that creates a descendant between its two enumerations. Neither is named in the contract.
4. `conf::sv_2_len_checked_before_allocation`: a closure `Oversize` from the header alone does not show the order of the check and the allocation (an implementation that allocates and then refuses also ends the epoch). It needs an observer of the allocation at the lane buffer, which the contract does not name.
5. `conf::sv_5_each_exit_cause`: the cause `OrphanGrace` needs the real guardian timer with no host attached and a later host that reads the cause (adoption, package p5). Without it the id would not prove every named cause. Of the other causes, `Normal`, `Signal`, `HostStop`, `StartupTimeout` and `Killed` are exercised in `a2_5_startup_runs_to_epoch_one_commit`, `sv_5_no_unobservable_limit_causes`, `sv_9_sigterm_then_group_kill` and `a2_5_send_to_exited_is_service_ended`; `ChildReported` is exercised nowhere else. The draft with all six causes is in the branch history (commit 6e03d4e).
6. `conf::sv_6_host_crash_loses_unpolled_frames`: `slow:real-host-crash` needs a host process that dies. `drop` ends only a handle of this process. A harness step with a defined host process boundary is needed (the adoption package p5 may define it).
7. `conf::sv_9_enumerated_descendants_killed`: SV-9 kills "every descendant the guardian can enumerate". Which descendants a guardian can enumerate is its own mechanism, so a descendant outside the group is killed only if enumerated, and no observer reports the enumeration. A descendant in the group is already covered by `sv_9_sigterm_then_group_kill`.
8. `conf::sv_5_service_events_ordered`: SV-5 lists the events and SV-6 fixes only that the commit follows the authentication of every lane. No clause fixes the relative order of `LaneConnected` and `Reconnected` in the event queue, or a stronger order. The order-free content (each event arrives) is proven by `sv_2_preamble_authenticates` and the `sv_6_*` ids. The chained draft is in the branch history (6e03d4e).
9. `conf::sv_8_orphaned_service_ended_after_grace`: SV-8 fixes the end of the service after `orphan_grace` on the guardian's own monotonic clock. A transcript can show both processes dead after the grace, but a guardian that ends them at once passes too. Proving "not before the grace" and "after it" needs the guardian's clock reading and the instant at which it ended the service, and the contract names no observer for either (TM-3 covers the Core library's clock only, and R-14.3 allows measurement at the testkit boundary, never inside the guardian). It stays pending until a ruling or an amendment names an observer of the guardian's clock. The slow-tier proof for the cause `OrphanGrace` is part of the same gap as item 5.
