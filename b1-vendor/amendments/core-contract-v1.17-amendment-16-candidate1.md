# Core contract v1.17, Amendment 16 (candidate 1): `StopService` of a `Lost` service completes with the loss

The base is the Core contract of `frozen/current` (manifest final33), which is unchanged except where A16-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-04. Found by the Stage 1 Core lead (P7, services).

**Why.**
- Core A2-1 admits `StopService` in `Started`, `Reconnected`, `Exited` and `Lost`. Its `Ok` value is "`ServiceExit` (the `Exited` payload, SV-5)", its sync errors are `UnknownService` and `WrongState`, and its async error is `RegistryFailed`.
- A `Lost` service has no exit: its guardian is gone (SV-8, SV-10, AD-2 `GuardianLost`), and Core observed no exit status. So no admitted outcome fits.
- The session operation already handles this case: `Stop` is admitted in `Lost`, and its `Ok` value is "`SessionEnd` = `Exited(Exit)` or `Lost(reason)`". `StopService` follows the same pattern.

| # | Clause | Test |
|---|---|---|
| A16-1 | **`StopService` completes with `ServiceEnd`**, which is **`Exited(ServiceExit)` or `Lost(reason)`**, as `Stop` completes with `SessionEnd`.<br>- **`Started` or `Reconnected`:** SV-9 is unchanged. The service ends, and the result is `Exited(ServiceExit)` with the observed exit.<br>- **`Exited`:** the result is `Exited(ServiceExit)` with the recorded exit, as before.<br>- **`Lost`:** the result is **`Lost(reason)`** with the reason of the service's `Lost` state (for example `GuardianLost`, with its `payload_may_remain`). Core **sends no signal and ends nothing**: SV-9's stop runs through the guardian, which is gone, and SV-8 says that Core then guarantees nothing about the payload. No `ServiceState` event is posted, because the state does not change. The id stays in use until `RemoveService` (SV-10).<br>- **If the service is lost while a stop is in flight**, the result is `Lost(reason)`, as for any operation whose state changed in flight (ER-0).<br>- Core never invents an exit: there is no `ServiceExit` for a service that it did not see exit. | `conf::a16_1_stop_lost_service_completes_lost_with_its_reason`, `conf::a16_1_stop_lost_service_sends_no_signal`, `conf::a16_1_service_lost_during_stop_completes_lost` |

## A16-2. Text replaced

1. **Core A2-1, the `StopService` row, `Ok` value:** "`ServiceExit` (the `Exited` payload, SV-5)" reads "`ServiceEnd`".
2. **Core A2-1, "Value types used in the table":** a new entry "`ServiceEnd` = `Exited(ServiceExit)` or `Lost(reason)` (A16-1)", after `SessionEnd`.

Every other clause is unchanged.
