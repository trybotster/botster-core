# Integration review: #180 (the guardian's proof role, the P7 carry; branch stage1/p5-guardian-proof-role)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 1fe19991

Reviewed head: `1fe19991c61c3665f4695ca0c63cbbca09b1f6c6`, one commit on v1 `1832866c0f7787a442dc29745aca27c41162bc2e`, which is the current v1 (2 files,
+25 -13). The PR's gate log: `shared/core-stage1/gate-logs/guardian-proof-1fe19991.log` (full Linux gate, all 10 jobs
green). This reviewer did not read it.

This closes the carry that this reviewer recorded for P7: guardian-core used `token_proof` in both directions.

- **The roles now match every other end.** `botster-core-link/src/proof.rs` (Roles): a worker proves itself with
  `token_proof`, a host with `host_proof`, and each end checks the role of the other. At the head:
  - `botster-guardian-core` `Link::hello` sends `token_proof` (the service's role), and `authenticate` checks `host_proof`.
  - `botster-worker-core` sends `token_proof` (`worker.rs:251`) and checks `host_proof` (`:358`).
  - `botster-core-host` sends `host_proof` and checks `token_proof` (`adopt.rs:162,185`; `inbound.rs:175,210`).

  So a host that connects to a guardian sends the proof that the guardian now checks, and it checks the proof that the
  guardian sends.
- **The reflection case.** The new `"reflected"` case sends the guardian's own hello back (the right token and epoch, the
  service's role), and the guardian refuses it. The mutant that puts `token_proof` back in `authenticate` fails that case.
  The `"token"` case now uses `host_proof` with the wrong token, so it still tests the token, not the role.
- **The sent-hello assertions** (4) now expect `guardian_hello` (`token_proof`). That is the guardian's unchanged output,
  so they assert the role of what it sends.
- **No other crate** depends on `botster-guardian-core` at the head.

VERDICT: CLEAN (0 open) at 1fe19991c61c3665f4695ca0c63cbbca09b1f6c6
