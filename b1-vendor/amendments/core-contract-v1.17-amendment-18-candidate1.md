# Core contract v1.17, Amendment 18 (candidate 1): `last_output_at` is the host's observation, and an adopted session starts a new one

The base is the Core contract of `frozen/current` (manifest final35), with Amendments 2 to 16 and Errata 1 to 4, which are unchanged except where A18-2 names a replacement. Core Amendment 17 (in review) touches no clause named here. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. Found by the Stage 1 Core lead (P5, `conf::ad_3_adopted_equals_started`, step 13).

**Why.**
- `terminal_state(id)` returns `last_output_at` (6.1). TM-1: "The host passes a monotonic `Instant` and a unix time to `pump` and to stamped calls … Unix time only stamps `at` fields. Core reads no clock of its own." So `last_output_at` is the unix time that the host passed to the `pump` that observed the output. The worker has no clock to stamp it with (TM-1), so the worker does not hold it.
- AD-3: "After adoption a session is indistinguishable from one started in this host: same reads, same input, same events (**the state is re-read from the worker**). An event history before adoption is not replayed." A new host can re-read only what the worker holds, so it cannot know the old host's stamp. `conf::ad_3_adopted_equals_started` requires the two `last_output_at` values to be equal, which no implementation can meet without breaking TM-1.
- TM-4 posts `Silent` by "the first `pump` with `now ≥ last_output_at + threshold`", and by TM-1 the threshold is measured on the monotonic clock. For an adopted session, no clause says where the idle period starts.

| # | Clause | Test |
|---|---|---|
| A18-1 | **`last_output_at` is the host's observation, and adoption starts a new one.**<br>- `last_output_at` is the unix time that the host passed to the `pump` in which Core observed the session's latest output (TM-1). It is `None` while this host has observed no output of the session.<br>- **After adoption,** `last_output_at` is `None` until the adopting host observes output. Core does not carry the earlier host's stamp, and the worker does not stamp time (TM-1).<br>- **AD-3's "same reads"** covers the state that is re-read from the worker. `last_output_at` is not part of it: it is a host observation, like the event history that AD-3 does not replay.<br>- **The silence edge (TM-4) of an adopted session** measures its idle period from the adoption, on the adopting host's monotonic clock, until output re-arms it. So an idle adopted session still gets `Silent` once, a threshold after its adoption. | `conf::a18_1_adopted_last_output_at_is_none_until_output`, `conf::a18_1_adopted_idle_session_is_silent_a_threshold_after_adoption`, `conf::ad_3_adopted_equals_started` (its `terminal_state` comparison excludes `last_output_at`) |

## A18-2. Text replaced

1. **Section 6.1, `terminal_state`:** `last_output_at` reads with A18-1.
2. **AD-3:** after "(the state is re-read from the worker)" add "; `last_output_at` is a host observation and starts again at adoption (A18-1)".
3. **TM-4:** after "the next output re-arms it" add ". For an adopted session, the idle period starts at the adoption (A18-1)".

Every other clause is unchanged.
