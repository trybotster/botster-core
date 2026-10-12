# core-p5 notes: Core transcripts for Stage 1 package p5 (adoption)

Branch `foundation/core-p5`. The clause list is `docs/stage1-clauses/p5-adoption.txt` of `botster-core` (branch `stage1/plan`).

## What this branch adds

- 47 transcripts in `conformance/core` (ids removed from `conformance/pending.txt`).
- The check `type_check: "a6_2_deferred_set"` (Core A6-2). It reads `conformance/deferred.txt` and requires exactly the two enumerated deferred ids, each with its start condition.
- Fifteen test controls (`docs/core-testkit-controls.md`, rows after `pty_output_unread`).
  Edges are named in A5-1: `registry_row` (storage), `hold_start_at` (scheduler), `announce_protocol` and `process_identity` (process), `lose_host` (testkit host loss).
  Slow-tier controls (real processes): `host_crash`, `reuse_pid`, `bystander_alive`.
- FakeCore is not changed. A transcript that FakeCore cannot run is marked `fake: structural` with ruling R-12.

## The three ways that a host goes away

| Control or step | Meaning |
|---|---|
| `drop` | A graceful Core drop (LC-12). The directory lock is released. Workers keep running. |
| `lose_host` | Testkit host loss. The lock goes, the links to workers stay, and the old handle is a stale host (DP-8). |
| `host_crash` | A real host crash. Slow tier only (AD-7). |

## Core Amendment 10 (steward answer to the gaps of this branch)

The first draft of this branch left two ids pending because the named edges could not give an impostor at the handshake, a read of signals, or a corrupted row. The steward ruled in Core Amendment 10 (A10-1, A10-2). The branch now holds `conf::a10_1_wrong_token_is_never_signalled`, `conf::a10_1_wrong_instance_is_never_signalled`, `conf::a10_2_corrupted_row_is_lost_registry_corrupt`, `conf::ad_6_token_and_instance_checked` and `conf::ad_2_lost_reasons`, with the controls `impostor_worker`, `signals_received` (an observer, R-14.3), `corrupt_registry_row` and `guardian_exit`. `ad_2_lost_reasons` covers each reason: `StartInterrupted`, `WorkerGone`, `WorkerUnreachable`, `WorkerVersion`, `RegistryCorrupt`, `GuardianLost` and `EpochExhausted` (a scripted service at the largest epoch). The mapping of an unknown reason to `Lost(Other)` is a host rule, so no transcript for Core covers it.

`wait_worker_exit` and `worker_alive` are process-edge controls for the exit of an orphan worker (AD-7). `guardian_exit` is not named in A10. It is the A5-1 ability "the guardian can be ended" as a control. The `payload_may_remain` field is matched by type only, because it depends on the OS (SV-8).

## Ids skipped

- Five ids that are already on main: `id_2_instance_survives_adoption`, `id_2_ops_do_not_survive_restart`, `lc_11_adoptall_posts_a_state_for_every_row`, `lc_12_drop_leaves_workers_running` and `lc_5_worker_survives_stop`.
- Two deferred ids (A6-2): `ad_4_previous_worker_version_adopts` and `ad_4_missing_worker_capability_is_unsupported`.

## Choices that a reader may question

- **`ad_1_stopping_resumes_stop`.** The first Stop runs 500 ms on the first host. After the adoption the session is still `Stopping` 600 ms later, and it ends after 1000 ms from the adoption. AD-1 says that `stop_grace` is measured from the adoption. FakeCore measures it from the Stop, so the transcript is structural.
- **`ad_1_starting_with_identity_adopts`.** `hold_start_at before:"running"` gives a row that is `Starting` with a recorded identity while the payload runs. The `registry_row` expectation states that precondition; the transcript does not guess it. The `before:"payload"` hold is used in `ad_7_payload_launches_after_durable_identity` only. A row `Starting` with identity and no payload is not adopted in any transcript, because AD-1 does not say what a new host does with it.
- **`dp_8_adoption_fences_the_previous_host` and `dp_8_old_epoch_commands_and_descriptors_are_refused`.** An old-host `WriteInput` is `Ok(InputResult)` (A2-2): `NotWritten(SessionEnded)` (never sent) or `Unknown` (sent, not acknowledged). The stale `Signal` completes `WorkerLinkFailed` (A2-1). The program edge then holds exactly the new host's bytes, read after `await_pty`. A route descriptor of the old host gives `RouteClosed{HandoffFailed}` (DP-2: an accepted attach whose hand-over fails).
- **`ad_7_uncertain_registry_write_is_reported`.** The certain failure leaves the base labels. After the next open the record equals the storage edge's own read (`registry_row`), and it is the base labels or the uncertain write's labels.
- **`a6_2_out_of_set_hello_is_lost_worker_version`.** The worker announces 2 and 9. The transcript never builds a worker of an earlier protocol (A6-2). The predicate test calls `worker_protocol_compatibility` with 0, which is a call, not a worker.
- **`ev_8_open_query_survives_host_restart`.** The client answers after the restart with an `encoded` reply. The bytes that reach the PTY are the client's own bytes, so no terminal byte is hand-written for the oracle to produce.
- **`st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut`.** The program output is a script of escape sequences. The comparison is the libghostty oracle's (`oracle_resume`).

## Prior art

The adoption pattern (`drop`, `open as h2`, `AdoptAll` on the new handle) comes from `conf::id_2_instance_survives_adoption` and `conf::lc_11_adoptall_posts_a_state_for_every_row`. The withheld-link fixture comes from `conf::a2_1_every_begin_operation_has_a_row_and_the_documented_ok_value` (A6-1). The route and gate idioms come from core-p4a-ou and core-p3 transcripts (`route_gate`, `queue_overflow`, `storage_fail_write`).

`storage_fail_write` is core-p3's control. Its row is in `docs/core-testkit-controls.md` on that branch and not yet on main.

## Handles, orders and real waits

- Every step after a `drop` names the new handle (`h2` or `b`). `CoreDriver` defaults to `h`.
- The events of an adoption (one `SessionState` per row, `RouteAdopted`, `FocusChanged`, the completion) arrive in no fixed order (LC-11, EV-5), and the adoption also posts events that a transcript does not name (for example `FocusChanged`, which 6.2 requires once per session). Each adoption is one `pump_collect` of the wanted events and the completion. A second event for a wanted matcher fails it. `dp_8_route_adopted_event_per_live_route` adds `forbid` for the closed route (a new key of `pump_collect`), and both once-checks end with a quiet check up to a barrier that Core orders after the adoption's events.
- Slow-tier timing uses two steps. `wait_worker_exit{session, deadline_ms}` blocks on the process edge's own exit notification for the worker that the edge spawned (`ad_7_...`). `deadline_ms` is a marked limit, not a contract value. The transcript binds `exited`, then `assume`s it is true (inconclusive, not failed, if the exit was not observed within the limit), then reads `worker_alive` of the same worker. `real_wait` (new, in the shared runner: it pumps and polls into the handle's buffer for a real interval, asserts no wake and drops no event) delimits the observation after the adoption in `sv_8_adoption_cancels_grace`; the guardian's `orphan_grace` is in `SpawnService.limits`. `advance_clock` does not move a real worker's clock.
- `ad_7_crash_between_steps_leaves_no_unregistered_payload` covers two crash points: after the worker and before its identity, and after the identity and before the payload. The gap before the worker holds trivially, because no process exists (R-71). Its old section, a plain `hold_start` crash, is removed (R-71 follow-up): under R-71 (a) that hold crashes with a live held worker, which is the next section's span, and its `AdoptAll` raced the worker's own `startup` exit.
