# Core contract v1.17, Amendment 6 — candidate 1 (service lanes in the testkit, the first worker protocol, uploads at Remove)

The base files are the Core contract of `frozen/current` (manifest final12): v1.17, Amendments 2 to 5, and erratum 1. They are unchanged except where A6-4 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01.
- A6-1 was found by the Foundation lead while it built the replacement map.
- A6-2 and A6-3 were found by the Stage 1 Core lead (questions Q2 and Q5).

## A6-1. The service-lane edge of the testkit (adds to Core A5-1)

| # | Clause | Test |
|---|---|---|
| A6-1 | **The testkit gains a service-lane transport edge.** Under it, a **scripted service** connects to the service's lane endpoints over in-memory connected streams, in place of the sockets in `CORE_SERVICE_ENDPOINT`.<br>- The service sends the real SV-2 preamble (magic, version, id, secret, lane, epoch).<br>- Core's real preamble, epoch and lane code checks it (SV-2, SV-3, SV-5, SV-6, A2-5), with the same code path as for real sockets (A5-1).<br>- The scripted service sends and receives frames, connects with a new epoch, closes a lane, and exits at script points.<br>- **Process and guardian:** a refused spawn, and the end of the service or of the guardian, use the existing process edge (A5-1). `SpawnReport` (A4-1) comes from the process edge's script data.<br>- **What stays in the named real-process tests** (section 11): the **peer OS identity** check of SV-2 (an in-memory stream has no OS peer), orphan grace on host loss (SV-8), `payload_dies_with_guardian`, the enumeration of descendants, and the stop that sends SIGTERM and then kills the group. | `conf::a6_1_scripted_service_over_in_memory_lanes_uses_the_real_preamble_and_epoch_checks`, `conf::a6_1_lane_and_epoch_outcomes_match_real_lanes`, `conf::a6_1_peer_identity_is_real_process_only` |

## A6-2. The first worker protocol (clarifies AD-4)

| # | Clause | Test |
|---|---|---|
| A6-2 | **At the first worker protocol there is no previous protocol.** The first v1 worker protocol number is **1**.<br>- For a Core whose protocol T is 1, `adoptable_worker_protocols()` returns `{1}`, and `worker_protocol_compatibility(Some(p))` is `Compatible` only for `p = 1`. AD-4's "N and N−1" means "N, and N−1 when N−1 ≥ 1". A protocol 0 never exists.<br>- **No old protocol is fabricated for a test.** A worker built to speak an invented earlier protocol is not a test subject. The behaviors that only a real N−1 worker can show are **deferred until protocol 2 ships**:<br>&nbsp;&nbsp;- the previous worker adopts;<br>&nbsp;&nbsp;- a capability that the previous worker lacks gives `Unsupported`;<br>&nbsp;&nbsp;- a per-worker feature that is absent from the previous worker (for example DP-12's `focus_report`).<br>&nbsp;&nbsp;From the first release whose T is 2, those tests run against **the pinned worker binary of the last release with protocol 1**, as AD-4 says.<br>- **Testable now:** the predicate and the sets (`p = T` or `p = T − 1`, `Unknown` for `None`), and a hello whose number is outside the adoptable set, which gives `Lost(WorkerVersion)`. The real worker can announce another number for that test, because only the version check is under test, never an old protocol's behavior.<br>- **Ledger:** a deferred test id is recorded as **deferred until worker protocol 2**, with this clause as its authority. It is neither passed nor pending, and it must pass at that release. | `conf::a6_2_first_protocol_adoptable_set_is_one`, `conf::a6_2_out_of_set_hello_is_lost_worker_version` |

## A6-3. Uploaded files at Remove (clarifies LC-7 step 3 and DP-5b)

| # | Clause | Test |
|---|---|---|
| A6-3 | **Core deletes uploads through the worker that wrote them, and it reports what it could not delete.**<br>- **Who deletes:** the worker of the session knows the names of the files its routes wrote (DP-5b). At LC-7 step 3 it deletes them. Core records **no** per-file name in the registry (no registry write per upload).<br>- **(a) A delete that fails** (for example `EACCES` or `EIO`): it does **not** stop the teardown. Steps 4 and 5 still run, and the session id becomes free.<br>- **(b) No reachable worker:** this happens for a `Lost` session when no stray worker is found. Core cannot find the names, and nothing is deleted.<br>- **The report:** `Completed{Remove}` carries `RemoveReport{ uploads }`, where `uploads` is:<br>&nbsp;&nbsp;- `Deleted`: every file is gone, or there were none;<br>&nbsp;&nbsp;- `NotDeleted{ reason: delete_failed, paths }`: the files that stayed;<br>&nbsp;&nbsp;- `NotDeleted{ reason: worker_unreachable }`: unknown files may remain in the directories the host supplied.<br>&nbsp;&nbsp;`RemoveReport` is non-exhaustive. Remove's only asynchronous error stays `RegistryFailed` (A2-1).<br>- **The host owns its directories** (OU-1 `file_directory`). A host that gives each session its own directory can delete that directory itself, whatever the report says. The Hub→client contract's data-root layout (RT-2, `sessions/<session_id>/uploads/`) does this.<br>- **DP-5b's** "deleted at `Remove` (LC-7) at the latest" means "deleted at `Remove`, or reported as not deleted in `RemoveReport`". | `conf::a6_3_uploads_deleted_by_the_worker_at_remove`, `conf::a6_3_delete_failure_does_not_stop_teardown_and_is_reported`, `conf::a6_3_lost_without_worker_reports_worker_unreachable` |

## A6-4. Text replaced

1. **A2-1, the `Remove` row:** the result "`()` after LC-7 step 5" becomes "`RemoveReport` (A6-3) after LC-7 step 5".
2. **LC-7 step 3** reads with A6-3.
3. **DP-5b, "Cleanup"** reads with A6-3.
4. **AD-4** reads with A6-2. Its tests `conf::ad_4_previous_worker_version_adopts` and `conf::ad_4_missing_worker_capability_is_unsupported`, and every per-worker feature test that needs an N−1 worker, are deferred until worker protocol 2.
5. **Core A5-1's edge list** gains the service-lane edge of A6-1.

Every other clause is unchanged.
