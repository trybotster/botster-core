# Core contract v1.17, Amendment 4 — candidate 1 (the spawn report is readable while the service exists)

The base files are the Core contract of manifest `final8`: v1.17 and Amendments 2 and 3, which are unchanged. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-09-30. Found by the Foundation build's core-contract review (steward ruling R-9).

**Why.** SV-4 says that the spawn completion is `Ok(SpawnReport{ applied, unsupported })`, "and the same lists are in the service's state". Amendment 2, A2-5, defines `ServiceState` as the event values `Started`, `Reconnected{epoch}`, `Exited`, `Lost` and `Released`, and it names no place for the lists. No call reads them. The only copy is then the spawn completion, which does not survive a host restart (ID-2). A host that adopts the service (AD-3: "indistinguishable from one started in this host") could not learn which bounds apply. This amendment gives SV-4's sentence its reader. It changes no event.

## A4-1. `service_report` (adds to SV-4 and to the A2-1 operation table)

| # | Clause | Test |
|---|---|---|
| A4-1 | **`service_report(id) -> Result<SpawnReport, UnknownService>`** is a sync read.<br>- It returns the `SpawnReport{ applied, unsupported }` of the service's spawn: the same lists as the spawn completion (SV-4).<br>- It answers from the time the spawn completion is `Ok` until `RemoveService` frees the id (A2-1). This includes the states `Exited` and `Lost`.<br>- **After adoption it returns the same lists** (AD-3). The lists are part of the service's recorded state, and a new host never re-applies the bounds.<br>- An id with no successful spawn, or an id already removed, is `UnknownService`.<br>- It posts no event, takes no slot and does not touch the service (AM-4).<br>- **SV-4's phrase** "the same lists are in the service's state" means: readable with `service_report`. `ServiceState` (A2-5) stays the event enum and carries no lists. | `conf::a4_1_service_report_equals_spawn_completion`, `conf::a4_1_service_report_survives_adoption`, `conf::a4_1_service_report_after_exit_until_remove`, `conf::a4_1_unknown_or_removed_service` |

`FakeCore` implements it from the clause, as script data for the bounds (Core section 11).
