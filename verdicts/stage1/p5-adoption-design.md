# Integration review: P5 adoption design (branch stage1/p5-adopt-1, `crates/botster-core-host/DESIGN.md`)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate. Design only; no code.

## Round 1 — NOT CLEAN on head 0682aabe

Reviewed head: `0682aabe47f18b6756cc8c5655068f5a9e27e7b9`. P5 announced `d6c2cf6d`; the branch moved two commits past it
(`a19f22f7`, `0682aabe`: the Sim process table is in memory, and `connect_worker` is a `HostEdges` method). Both are
reviewed here. Base: v1 `a0f78fe4`. Contract: core contract v1.17 with the amendments at `contracts-v0.1.13`. Scope: the
cross-package parts (`botster-core-link`, `botster-worker-core`/`botster-worker`, `botster-core-testkit`,
`botster-core-sys`).

### Checked, no finding

- **Direction (the worker listens, the new host connects).** It follows from DP-8 (the hello binds the host epoch, so
  the host speaks first) and from AD-6 ("worker or guardian endpoints"). It matches the old daemon's direction.
- **The role byte in the proof.** The current proof is `SHA-256(DOMAIN, token, epoch, instance)`
  (`botster-core-link/src/proof.rs:18,56`), the same value in both directions. In the adopt handshake the host speaks
  first, so an impostor could echo it. A role byte stops that, and one rule for both handshakes is right. `hello.rs`
  already leaves the proof rule to the AD-6 owner, and protocol 1 is not released, so no adoptable worker breaks.
- **The fence retires the old host's requests (3.4).** Request numbers are per link, so this is required. The test (a
  write in flight at the fence, and the same request number on the new link) is the right red test.
- **Bounded candidates (part 7).** One candidate at a time, the `startup` deadline, and a one-`Hello` frame bound. All
  three are sans-IO decisions with default-tier tests. This matches the decision rule of plan r22 section 8.
- **Testkit (part 6).** The Sim process table is in memory only, and real processes stay in `botster-test-process`.
  `connect_worker` is a `HostEdges` method, so `RealCoreHarness` does not change. This agrees with P6.
- **No rebind and no repair of a missing endpoint.** AD-2: a `Lost` session is never restarted in place.
- **A52 `Option<u64>`.** `None` never matches. This is better than a sentinel 0.

### D1 MEDIUM — "E above every epoch it has seen" blocks the AD-2 retry by the same host

Step 3 requires the host's epoch E to be **above** every epoch that the worker has seen. Step 4 records E before the
host has checked the worker's answer. DP-8 says: "A worker obeys only the **highest** epoch it has seen". So E equal to
the highest seen epoch is a valid epoch.

Failure case:
1. Host H (epoch E) adopts worker W. W passes step 3, records E, fences, and answers.
2. H does not complete the handshake: the report is late (3.7, `startup`), or the link breaks. The row is
   `Lost(WorkerUnreachable)`.
3. AD-2: "`begin(Adopt(id))` may be retried". H retries with the same E. W refuses it, because E is not above E.
4. Under H, the session can never be adopted, and every retry is `Lost(WorkerUnreachable)`, which says "may be retried".

Fix: the worker accepts E when E is **at least** the highest seen epoch. A hello with E equal to the current link's
epoch replaces that link after the proof passes, and the 3.4 fence applies to it too (the request numbers of the old
link of the same host also collide). A lower E is refused, as now. This keeps fencing: only one host holds epoch E,
under the data-directory lock (DP-8, LC-2). Add a test: an adoption that the host abandons after step 4, then an
`Adopt(id)` with the same epoch, which succeeds.

### D2 LOW — the worker has no way to learn `startup`

Part 7 and AD-7 need the worker to use `CoreLimits.startup`: for the self-exit of a worker with no payload and no host,
and for the candidate deadline. `WorkerLaunch` carries only `--role`, `--control`, `--instance` and `--epoch`
(`botster-core-link/src/launch.rs:34-40`), and the design adds only `--endpoint`. Name the launch argument that carries
`startup` (in `botster-core-link`, with a parse test), so the value comes from the host's `CoreLimits` and the worker
invents no deadline.

### D3 LOW — the endpoint file has no owner for its removal

Part 1 creates `<data_dir>/w/<InstanceId>`, but no step removes it. State who unlinks it, and when. For example:
- the worker, when it exits;
- `Remove` (LC-7), after the worker's end is verified. SV-9 names "the endpoints" among the things that a remove
  releases.

Otherwise every removed session leaves a socket file in `w/`.

### D4 LOW — state that the hello and the proof rule are the same for protocols T and T - 1

AD-4 requires an N - 1 worker to adopt (`conf::ad_4_previous_worker_version_adopts`) and any other version to give
`Lost(WorkerVersion)`, not `WorkerUnreachable`. That works only if a worker of protocol P can read the hello of a host of
protocol T and check its proof. The hello is JSON, and its decoder ignores unknown fields (`hello.rs`), so the format
supports this. Write it into part 2 or 3 as a rule: the five hello fields and the proof rule (domain, role byte, field
order) do not change between protocol numbers. A future change to them would turn `WorkerVersion` into
`WorkerUnreachable`.

VERDICT: NOT CLEAN (4 open: D1 MEDIUM; D2, D3, D4 LOW)
