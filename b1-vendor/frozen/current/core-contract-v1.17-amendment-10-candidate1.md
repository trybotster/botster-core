# Core contract v1.17, Amendment 10 — candidate 1 (two more testkit edge abilities: an impostor at the adopt handshake, and a corrupted registry row)

The base is the Core contract of `frozen/current` (manifest final17). It is unchanged except where A10-3 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the core-p5 pair, through the Stage 0 lead (`conf::ad_6_token_and_instance_checked`, `conf::ad_2_lost_reasons`).

**Why.** Core A5-1 and A6-1 list what each testkit edge can do. Two adoption clauses need inputs that no listed ability gives:
- AD-6: "The adopt handshake proves the token and the `InstanceId`. A process that does not match is never signalled";
- AD-2's `RegistryCorrupt`.

Every other AD-2 reason is reachable with existing abilities:
- `GuardianLost`: the process edge ends the guardian (A5-1);
- `EpochExhausted` (SV-6): the scripted service of A6-1 chooses its own epoch, so it can commit the largest epoch, and its next connection cannot exceed it.

| # | Clause | Test |
|---|---|---|
| A10-1 | **The process edge can present an impostor at the adopt handshake.** At adoption, the edge can answer the handshake for a session's recorded worker with a **wrong token** or a **wrong `InstanceId`**, from an endpoint that is not the session's worker.<br>- Core's real handshake check (AD-6) runs unchanged and decides the outcome.<br>- The test asserts that Core never signals the impostor, and that the row's outcome is what AD-6 and AD-2 give. For example, a non-matching process at the recorded pid is `WorkerGone`.<br>- The impostor is boundary input. It implements no worker behavior beyond its handshake answer. | `conf::a10_1_wrong_token_is_never_signalled`, `conf::a10_1_wrong_instance_is_never_signalled` |
| A10-2 | **The storage edge can corrupt one stored registry row.** The edge damages the stored bytes of a chosen row so that Core's real registry decoder must reject them.<br>- The damage starts from bytes that Core's own encoder wrote. The testkit never hand-writes a registry format.<br>- Core's real decoding and recovery run unchanged, and the row becomes `Lost(RegistryCorrupt)` (AD-2). | `conf::a10_2_corrupted_row_is_lost_registry_corrupt` |

**Observers need no amendment.** A read-only observer at an edge boundary (for example, which signals Core sent to which process) is not an edge ability. Steward ruling R-14.3's conditions apply: it counts at the boundary, adds no synchronization point, is testkit-only, and cites the clause it observes.

## A10-3. Text replaced

1. **Core A5-1's edge list:** the process edge gains A10-1, and the storage edge gains A10-2.
2. **`conf::ad_2_lost_reasons`** covers every listed reason through the testkit: `RegistryCorrupt` (A10-2), `GuardianLost` (process edge), `EpochExhausted` (A6-1's scripted service at the largest epoch), and the reasons already reachable.

Every other clause is unchanged.
