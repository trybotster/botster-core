# Core contract v1.17, Amendment 16 (candidate 2): `StopService` of a `Lost` service completes with the loss

The base is the Core contract of `frozen/current` (manifest final33), which is unchanged except where A16-2 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes candidate 1** (`82b0ca58…aada`, commit `8d7a39e`), which the Amendment reviewer rejected (verdict `bd9dcee`):
1. `Lost` has two reasons (A2-1 `ServiceState`): `GuardianLost` and `EpochExhausted`. Under `EpochExhausted` the same guardian and service continue (SV-6), so "the guardian is gone" was wrong for it. Candidate 2 gives each reason its own rule, and it names the one change to the service state model.
2. Candidate 1 did not say whether the `Lost` result carries `payload_may_remain`. It does: the result repeats the state's `Lost` payload.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-04. Found by the Stage 1 Core lead (P7, services).

**Why.**
- Core A2-1 admits `StopService` in `Started`, `Reconnected`, `Exited` and `Lost`. Its `Ok` value is "`ServiceExit` (the `Exited` payload, SV-5)", its sync errors are `UnknownService` and `WrongState`, and its async error is `RegistryFailed`.
- A `Lost` service has not exited as far as Core knows. `ServiceState` gives `Lost{reason, payload_may_remain}` with `reason` `GuardianLost` (the guardian died, SV-8) or `EpochExhausted` (the epoch counter is exhausted, SV-6, while the same guardian and service continue). Neither gives `StopService` an exit to return. So no admitted outcome fits.
- The session operation already handles this case: `Stop` is admitted in `Lost`, and its `Ok` value is "`SessionEnd` = `Exited(Exit)` or `Lost(reason)`". `StopService` follows the same pattern.

| # | Clause | Test |
|---|---|---|
| A16-1 | **`StopService` completes with `ServiceEnd`**, which is **`Exited(ServiceExit)` or `Lost{reason, payload_may_remain}`**, as `Stop` completes with `SessionEnd`. A `Lost` result carries the same two fields as the service's `Lost` state, so the result alone says what is known.<br>- **`Started` or `Reconnected`:** SV-9 is unchanged. The result is `Exited(ServiceExit)` with the observed exit.<br>- **`Exited`:** the result is `Exited(ServiceExit)` with the recorded exit, as before.<br>- **`Lost{GuardianLost, payload_may_remain}`:** SV-9's stop runs through the guardian, and the guardian is gone. **This is the one exception to SV-9:** Core sends no signal and ends nothing, and SV-8 says that Core then guarantees nothing about the payload. The result is `Lost{GuardianLost, payload_may_remain}`, the state's own payload. No event is posted, because the state does not change.<br>- **`Lost{EpochExhausted, payload_may_remain}`:** the guardian and the service continue (SV-6), and Core still holds the guardian's verified control link (AD-6). **SV-9 runs unchanged** through the guardian: `SIGTERM`, then the group and descendant kill after `stop_grace`. When the guardian reports the exit, Core posts `ServiceState{Exited{code, signal, cause, descendants_may_remain}}`, and the result is `Exited(ServiceExit)`. **This is the only transition out of `Lost`**, and only a host's `StopService` causes it: the service is then `Exited`, with an observed exit, and `RemoveService` follows as for any `Exited` service.<br>- **If the guardian is lost while a stop is in flight** (from any admitted state), the result is `Lost{GuardianLost, payload_may_remain}`, as for any operation whose state changed in flight (ER-0).<br>- Core never invents an exit: an `Exited` result always carries an exit that the guardian reported. | `conf::a16_1_stop_guardian_lost_service_completes_lost_and_sends_no_signal`, `conf::a16_1_lost_result_carries_payload_may_remain`, `conf::a16_1_stop_epoch_exhausted_service_runs_sv_9_and_completes_exited`, `conf::a16_1_epoch_exhausted_stop_posts_exited`, `conf::a16_1_guardian_lost_during_stop_completes_lost` |

## A16-2. Text replaced

1. **Core A2-1, the `StopService` row, `Ok` value:** "`ServiceExit` (the `Exited` payload, SV-5)" reads "`ServiceEnd`".
2. **Core A2-1, "Value types used in the table":** a new entry "`ServiceEnd` = `Exited(ServiceExit)` or `Lost{reason, payload_may_remain}` (A16-1)", after `SessionEnd`.
3. **SV-9, `begin(StopService(id))`:** reads with A16-1 for a `Lost` service (no signal under `GuardianLost`; unchanged under `EpochExhausted`).
4. **The service state model:** `Lost{EpochExhausted}` moves to `Exited` only through `StopService` (A16-1). Every other `Lost` stays `Lost` until `RemoveService` (SV-10).

Every other clause is unchanged.
