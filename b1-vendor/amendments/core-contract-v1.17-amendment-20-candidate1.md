# Core contract v1.17, Amendment 20 (candidate 1): a proof that observes the worker's own allocator runs on the testkit only

The base is the Core contract of `frozen/current` (manifest final38), with Amendments 2 to 19 and Errata 1 to 4, which are unchanged except where A20-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. Found by the Stage 1 Core lead (P3's reading of the real-Core tier, `conf::dp_3_frame_limit_checked_before_allocation`).

**Why.**
- A5-4 says (`core-contract-v1.17-amendment-5-candidate4.md`, line 38):
  > Both run the same suite and both must pass, so the in-process worker is proven to match the real process.
- A5-3 names the one class of ids that runs on the real tier only (`core-contract-v1.17-amendment-5-candidate4.md`, line 32):
  > A code that only a real OS condition can produce (a real crash, fsync durability, descriptor handoff, `payload_dies_with_guardian`) is a **named real-process test** (section 11), and the suite lists it as such.
- DP-3 says that "The limit is **checked before any allocation**". Steward ruling R-37 allows one proof of it: an allocation-peak observer, a `#[global_allocator]` wrapper with one atomic `fetch_max` per allocation, in the test binary, observing "the process in which the worker's route reader runs". On the testkit, the worker runs in the test process (A5-1), so the observer sees the route reader's allocations.
- On the real tier, the route reader is a real worker process. No clean form observes its allocator:
  - an allocator hook or a peak report in the production worker is a change of production code for a test (Testing rule 8);
  - a test-only worker binary is not the production worker, so the real tier would no longer prove what A5-4 says it proves;
  - an outside reading of the worker's resident memory does not see a large allocation whose pages the worker never touches, and a peak of virtual size is not available on every gate host.
- So `conf::dp_3_frame_limit_checked_before_allocation` can never pass on the real tier, and A5-4 has no exception for it. A ruling cannot add one.

| # | Clause | Test |
|---|---|---|
| A20-1 | **A proof that observes the worker's own allocator is a testkit proof.**<br>- An id whose proof needs an observation **at the process allocator boundary of the process in which the worker's code runs** is proven on the testkit only. On the testkit, that process is the test process (A5-1), and the observer follows steward ruling R-37's conditions: it observes at the allocator boundary, adds no wait, lock or synchronization point, lives only in `botster-core-testkit`, and never instruments worker code.<br>- On the real tier, the suite lists such an id as **testkit-proven**, with this clause as its reason. It is not run, and it is not a failure.<br>- The class has **one member**: `conf::dp_3_frame_limit_checked_before_allocation`. A new member needs a Core amendment that names it.<br>- The class does not weaken the real tier for DP-3. The real tier still runs every other DP-3 id, including the refusal of an over-limit frame header (`frame_too_large`).<br>- A5-4's rule stands for every other id: both run the same suite, and both must pass. A5-3's named real-process tests are unchanged. | `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list` |

**The test.** `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list`: the real-tier runner's list of testkit-proven ids equals the list in A20-1. Each listed id passes on the testkit. A static check is enough; it needs no harness.

**Rejected.**
1. **An allocator hook in the production worker.** It changes production code for a test (Testing rule 8).
2. **A test-only worker binary.** The real tier would not run the production worker, so it would not prove what A5-4 says.
3. **An outside memory reading.** It cannot see an allocation whose pages are never touched, and the gate hosts do not all report a virtual-size peak.

**Ids.**
- **New:** `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list`.
- **Unchanged:** `conf::dp_3_frame_limit_checked_before_allocation`. Its proof on the testkit is unchanged. Only its real-tier status changes.

## A20-2. Text replaced

1. **A5-4, Core's repo.**
   - Old (`frozen/current/core-contract-v1.17-amendment-5-candidate4.md`, line 38):
     > Both run the same suite and both must pass, so the in-process worker is proven to match the real process.
   - New: "Both run the same suite and both must pass, so the in-process worker is proven to match the real process. The only exceptions are A5-3's named real-process tests and A20-1's testkit-proven ids."

Every other clause is unchanged.
