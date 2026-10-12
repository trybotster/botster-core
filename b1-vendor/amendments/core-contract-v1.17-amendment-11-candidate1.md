# Core contract v1.17, Amendment 11 — candidate 1 (the impostor may send worker frames after its failed handshake)

The base is the Core contract of `frozen/current` (manifest final19). It is unchanged except where A11-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the core-p4b pair, through the Stage 0 lead (`conf::a6_3_unauthenticated_result_is_not_trusted`).

**Why.** Core A6-3 reports `Deleted` "only from a complete cleanup result whose source Core trusts: the worker authenticated under AD-6, or Core itself". A result from a link that failed AD-6 gives `NotDeleted{reason: outcome_unknown}`. To prove that Core does not trust such a result, a test must send one on a link that failed AD-6. But A10-1's impostor "implements no worker behavior beyond its handshake answer".

| # | Clause | Test |
|---|---|---|
| A11-1 | **After a failed handshake, the impostor may send worker-shaped frames.** The A10-1 impostor, once Core's real AD-6 check has rejected its handshake, may send frames that a worker would send, for example a cleanup result for a `Remove` that claims every upload deleted (A6-3).<br>- **Core accepts none of them.** No frame from a link that failed AD-6 changes any session state, report or event. The test asserts this as an outcome, not as a mechanism.<br>- For the `Remove` case, the outcome is `RemoveReport{uploads: NotDeleted{reason: outcome_unknown}}` with no paths, the teardown completes, and the session id becomes free (A6-3, LC-7).<br>- The impostor is still boundary input. Its frames are scripted, and it implements no worker logic. | `conf::a11_1_frames_after_a_failed_handshake_change_nothing`, `conf::a6_3_unauthenticated_result_is_not_trusted` (now provable through A11-1) |

## A11-2. Text replaced

1. **A10-1:** "The impostor is boundary input. It implements no worker behavior beyond its handshake answer" becomes "The impostor is boundary input. It implements no worker logic: beyond its handshake answer, it only sends the scripted frames that A11-1 allows".

Every other clause is unchanged.
