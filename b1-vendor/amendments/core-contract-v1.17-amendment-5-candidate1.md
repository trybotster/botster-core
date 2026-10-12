# Core contract v1.17, Amendment 5 — candidate 1 (real Core with injected edges replaces FakeCore)

The base files are the Core contract of `frozen/current`: v1.17 and Amendments 2 to 4, which are unchanged except where A5-5 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01. Decision: the orchestrator, under the user's instruction "if it's better long term, let's do that", adopted the hybrid of the steward, the Foundation lead and the Amendment reviewer.

**Why.** Section 11 requires a hand-written `FakeCore`. A second implementation of Core drifts from the first. It also cannot be faithful on terminal behavior without a terminal model, which steward rule R-7 forbids (libghostty is the only terminal oracle). This amendment keeps the **goal** of section 11 ("A bug is fixed on one side without the other side's tests changing") and every **property** that the fake provided:
- seeded orders;
- failure injection per error code;
- no worker process, no PTY and no sleeps in the Hub's unit tests.

It gets them from **real Core with injected edges**, plus a permute-only decorator.

## A5-1. The test kit

| # | Clause | Test |
|---|---|---|
| A5-1 | **`botster-core-testkit`** is a Rust-only crate, versioned and released with `botster-core` (semver). It is **not** part of the facade (`botster_core::prelude`, `botster_core::contract`) and **not** part of `botster-core-ffi` or the C ABI (section 13 rule 1: no callback into host code).<br>It builds a **real `Core`** (the same `CoreApi`) whose session worker runs **in-process**. **One code path:** the in-process worker is the same worker code as `botster-session-worker`. Only the process boundary and the edges below differ; there is no second implementation of any clause.<br>**Edges** (each scripted by the test):<br>- **program:** a scripted program in place of the PTY child. It writes output bytes, reads the bytes written to it, and exits with a code or a signal at script points;<br>- **storage:** the registry. It can fail a write on command;<br>- **process:** spawning the worker or a service can be refused (an `errno`), and the guardian or the worker can be ended at a script point;<br>- **route transport:** an in-memory connected stream in place of a socket.<br>**Clock:** the testkit adds none. The host already passes `now` to `pump` (TM-1, TM-3).<br>**Terminal:** the in-process worker runs **real libghostty**. Every terminal byte and terminal state comes from it (steward rule R-7). A scripted program supplies only the program's output, never a terminal state.<br>**Determinism:** the same script, seed and inputs give the same events and results. | `conf::a5_1_testkit_is_real_core_one_worker_code_path`, `conf::a5_1_edges_scripted`, `conf::a5_1_deterministic_under_script_and_seed`, `conf::a5_1_not_in_facade_or_ffi` |

## A5-2. Seeded perturbation

| # | Clause | Test |
|---|---|---|
| A5-2 | **`with_seed(n)`** is a **decorator over `CoreApi`**, in the testkit, written from the ordering clauses. It randomizes every order that section 11 lists as open (event order across sessions and completion order for concurrent operations, OR-3; how many `pump` calls an operation takes, never zero, OR-1; the batching of events per `poll_events`; spurious wakes, OU-6 and TH-2; the order of route delivery across routes; output chunking and frame boundaries on a route, ST-1; partial progress per `pump`; coalescing of class K events; the number of pumps between `Stop` and `Exited` and between `Start` and `Running`; the order inside one `pump` of a due deadline's event relative to the other events of that pump).<br>**It is permute-only:**<br>- it never invents, alters or drops an event, a completion or a byte;<br>- it never varies which class D event is dropped (EV-2: always the oldest);<br>- it implements no operation.<br>Internal schedules are explored with the adopted scheduler tool (shuttle), not by the decorator. | `conf::a5_2_seed_varies_only_open_orders`, `conf::a5_2_decorator_is_permute_only` |

## A5-3. Failure injection

| # | Clause | Test |
|---|---|---|
| A5-3 | **Every error code of section 9.3 and Amendment 2, and every typed outcome**, is reachable in the testkit by one of three means:<br>1. **an edge** (for example storage → `RegistryFailed`, including a failed write-ahead row write before the worker starts, AD-7; process → `StartFailed{ExecFailed{errno}}`; ending the guardian or the worker → `Lost(GuardianLost)`, `WorkerGone`);<br>2. **a `CoreLimits` value** (the limit codes);<br>3. **a scripted refusal** by the decorator, for a chosen call. A scripted refusal returns only a code that the contract allows for that call (the A2-1 operation table).<br>A code that only a real OS condition can produce (a real crash, fsync durability, descriptor handoff, `payload_dies_with_guardian`) is a **named real-process test** (section 11) and is listed as such in the suite. | `conf::a5_3_every_code_reachable_or_listed_real_only`, `conf::a5_3_scripted_refusal_only_allowed_codes` |

## A5-4. Where the suite runs

| # | Clause | Test |
|---|---|---|
| A5-4 | - **Core's repo** runs the conformance suite against **real Core with real worker processes**, and against **the testkit**. Both run the same suite and both must pass, so the in-process worker is proven to match the real process.<br>- **The Hub's repo** runs its unit and subsystem tests against the **testkit** (no worker process, no PTY, no sleeps), under several seeds and under shuttle. It also runs the small named set of real-Core runs that section 11 lists, unchanged.<br>- **The Hub pins a released Core**, so a Core bug fix never changes the text of a Hub test (the section 11 goal). | `conf::a5_4_suite_runs_real_and_testkit`, `conf::a5_4_hub_tests_on_testkit_under_seeds` |

## A5-5. Text replaced

1. **Section 11, the bullet "Run against real Core … and against `FakeCore`":** replaced by A5-4.
2. **Section 11, the bullet "`FakeCore` is a deterministic in-memory implementation …", including the budget "1.5–2k lines":** replaced by A5-1. The size check moves to A5-2: the decorator implements no operation.
3. **Section 11, "Seeded interleaving mode":** its list of open orders stands. Its subject `FakeCore::with_seed(n)` becomes the A5-2 decorator.
4. **Section 11, "The Hub's tests":** "`FakeCore` only" becomes "the testkit (A5-4)". The rest stands.
5. **Section 11, "Changing the contract":** "the `FakeCore` behavior" becomes "the testkit edge or decorator change, where the clause needs one".
6. **Section 11, "Versioning":** "`0.x` until the suite, `FakeCore` and the example consumer pass" becomes "… the suite (on real Core and on the testkit) and the example consumer pass".
7. **TM-3:** "`FakeCore` models them as events" and "`FakeCore` exposes the same query" refer to the testkit. Guardian and starting-worker deadlines are process-edge events at script points, and the testkit's `Core` exposes the same `next_deadline` query.
8. **OR-3's test note** "(runs under FakeCore's seeds, section 11)" becomes "(runs under the A5-2 seeds)".
9. **Amendment 4, the last line:** "`FakeCore` implements it from the clause" becomes "the testkit returns the real `SpawnReport`, with the process edge's applied and unsupported bounds as script data".

Every other clause is unchanged, including the named real-process tests of section 11 and the C ABI of section 13.
