# PC package review

VERDICT: CLEAN

- PR: `trybotster/botster-core#231`.
- Reviewed head: `4521237e8200768a36ab4c8a27f268f3914a88ab`.
- Reviewed base: `6c2bccdad9ab6cfac860e63c5eb15d6f8ec5ac6c` (`v1`).
- Risk tier: HIGH. BUILD.md rule 4 applies because the PR moves the contracts pin.
- Open findings: 0.

The review covers the contracts pin move only. The program controls still need their implementation review.

The binding sources are `brief-program-controls.md`, contracts `docs/BUILD.md`, and plan revision 23ab at `stage1-plan.77739f96.md`.
The verified plan SHA-256 is `969711aafa5e79cd617b3a6b9777ac6042c2116c038b9fc8a20b5ca6572dc682`.

The seven manifest dependencies name `contracts-v0.1.27`.
The tag resolves to `b161db87ca82873b4f6b21d8433a69f086abb7cf`, as plan section 0 requires.
All nine contracts sources in `Cargo.lock` use that tag and commit.
The lockfile has no other package change. The manifest has no other value change.

The pending, real-pending, held, deferred, ledger, and minimum ID lists are unchanged.
The PR changes only comments in `core-pending.txt` and `status.rs`.
The six IDs that need steward approval remain pending.

The reviewer checked the dependency changes to `CoreHarness::probe_env`, operation normalization, and the probe's default path.
The harness keeps the empty default environment. The Core harnesses do not enable the control socket on this head.
Only `ad_3_adopted_equals_started` changes among the original 32 PC transcripts.
Its new ActivityOutput fence precedes the terminal-state check. The ID remains real-pending.
The contracts manifest and ID lists do not change between the two tags.

The existing full Linux gate passed on the reviewed head and base.
The gate ran `env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci` through the pool.
Every CI step passed. The mutation step found no mutant in the diff.
The gate exited with code 0 after 816 seconds.
Gate log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.27-4521237e-pool-20261010-145612-73744.log`.

The reviewer ran no tests. The reviewer checked logic, parsed the manifests, compared the lists, and read the supplied gate evidence.
The integration reviewer must also report CLEAN before the lead merges this HIGH PR.
