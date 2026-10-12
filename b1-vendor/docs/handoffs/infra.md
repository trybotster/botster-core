# Infra handoff: the Botster gate (paused 2026-10-02)

The user paused Botster v1 on 2026-10-02. This file lets a fresh infra engineer, or the same one resumed, run and repair
the gate without asking anything. BUILD.md "How each stage runs" has the rules; this file has the machinery.

## Who and where

- Infra engineer: botster session `sess-1790920314-00ac-0c0ba2e7e13aded5cfe3af28a328c550`, workspace "Botster v1 — Infra",
  worktree `~/botster-sessions/trybotster-botster-contracts-infra-remote-gate` (branch `infra/remote-gate`, merged; the
  worktree is parked on detached main). Resume: `claude --resume 967c3de3-cd41-4ec7-83a0-0f9dbb0cc1dd` in that worktree.
- No infra reviewer is alive. Every infra PR had a Sol review (CLEAN) before merge; spawn a new `sol` reviewer per PR.
- Code: `ci/remote/` in botster-contracts and botster-core. `job.sh` holds only each repository's settings and sources
  `job-common.sh`. botster-contracts owns `job-common.sh`; botster-core keeps a byte copy at its pinned botster-contracts
  revision, and a botster-core check fails on any byte difference (steward ruling, 2026-10-11). Change `job-common.sh`
  here first, then copy it into botster-core at the next pin move. The client source is botster-contracts `ci/remote/botster-gate`; the
  installed copy is `~/botster-sessions/shared/tools/botster-gate` (reinstall it with `cp` after a merge).

## The gate: `botster-gate`

`~/botster-sessions/shared/tools/botster-gate <worktree>` runs ONE gate of the exact HEAD commit:
`env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci`, with a cleared environment, `TZ=UTC`, and
`BOTSTER_CI_BASE_REF` set to the base commit recorded at the start (`origin/main` for botster-contracts, `origin/v1` for
botster-core). A green log counts as "CI green" whichever machine ran it.

**The machine (auto):**
1. Linux (`structvr@192.168.1.185`, via testq) when the host answers, testq is not paused, no job waits, 4 CPUs are free,
   and a Botster slot is free (3, or 4 while testq has no HyperFlex job).
2. Else the Mac (botsterq, exclusive), when its 1-minute load is under 12 or its CPUs are at least 50% idle, and botsterq
   runs no job.
3. Else the shorter line. The Mac is the only choice when the host does not answer or the head has no `ci/remote/job.sh`.
`--on linux|mac` forces the machine. `--queue` shows both queues. `--clean <worktree>` removes the Mac gate tree.

**Exit status:** the gate's own; 4 = a step was SKIPPED (not a full gate); 65 = uncommitted tracked changes; 75 (Linux) =
the host disk is below 40 GiB free; 125 = cancelled; 3 (Mac) = the gate tree could not be prepared.

**Logs:** `~/botster-sessions/gates/<repo>-<branch>-<sha8>-<machine>-<time>-<pid>.log`. The header names the repo, branch,
head, base, machine, the reason for the choice and the command; the last line is the exit status and the time. A failed
Linux job copies its mutation reports to `~/botster-sessions/gates/artifacts-<run>/`.

**How a Linux gate runs:**
- A **holder** process on the host (a bash heredoc inside `botster-gate`) owns the gate. It takes, in order, the branch
  lock, a Botster slot, the lease, then the worktree's source lock; it runs `testq submit` itself. The client sends a
  heartbeat every 10 s and runs `testq follow`. When the client ends (normally, Ctrl-C, a kill, or 180 s without a
  heartbeat), the holder cancels a queued or running job, waits until it has stopped and its containers are gone, and only
  then frees its locks. A job that does not stop keeps the locks; the holder reports it every minute.
- The head reaches the host by `git push` into `~/testq/projects/<repo>/src/<worktree-name>` (one checkout per worktree).
  Only committed content travels: no target/, .env, mise.local.toml or untracked file.
- `job.sh` (one testq job): image build under `image.lock`; volume housekeeping under the Botster-wide `volumes.lock`;
  for botster-core a token container (`fetch.sh`: cargo dependencies, private botster-contracts) and a public container
  (`fetch-public.sh`); then the gate container: `--cpus 4`, `--memory 16g`, `--init`, `--network none` (core),
  `--tmpfs /tmp` (4 GiB). Every container runs with `--network none` (the volume chown, the gate after a fetch step, the
  artifacts copy), except the steps that download: `fetch.sh` and `fetch-public.sh` (botster-contracts: cargo's crates and
  the npm packages of `ts/`). After a fetch step, the gate runs with `CARGO_NET_OFFLINE=true` and `npm_config_offline=true`.
  A project with no fetch step would download in its gate, so its gate would run online. A container on the default bridge adds and removes a host veth,
  and the host's browsers abort requests in flight at that change (net::ERR_NETWORK_CHANGED in other projects' suites).
- A detached head (the gate of a merge commit) uses its base branch's target volume and lock.

**How a Mac gate runs:** `botsterq run --exclusive` in a detached gate tree `~/botster-sessions/gate-trees/<repo>-<name>`
(its own target/). Inside the job: create the tree if needed, check out the head, `git clean -ffdx -e /target`, run the
repo's `ci/remote/fetch-public.sh` if present (else `git submodule update`), clean every submodule, then the command.

## The Linux host layout

- **testq** (`~/testq`, the shared queue; README there): capacity 20 CPUs for all projects. HyperFlex's CI jumps the queue
  with `--priority`; running jobs are never interrupted.
- **Botster's share:** 3 slots always (12 CPUs; HyperFlex keeps 8), plus 1 elastic slot (16 CPUs) only while testq has no
  HyperFlex job queued or running; a failed queue read counts as busy. Slots are flock files
  `~/testq/projects/botster-locks/slot.1..4`; also there: `branch-*.lock`, `volumes.lock`, `use-<volume>.lock`, `leases/`.
- **Images:** `botster-contracts-gate:<hash>` and `botster-core-gate:<hash>` (hash of the Dockerfile, rust-toolchain.toml
  and, for core, the pinned nightly of `xtask/src/tools.rs`). They hold the toolchains, cargo-nextest 0.9.146,
  cargo-mutants 27.1.0 (behind a shim that sets TMPDIR=/work/target/tmp), cargo-bolero 0.13.5, node 22 (contracts),
  cargo-public-api 0.52.0 and Zig 0.16.0 (core). A superseded image is removed after a build.
- **Volumes:** `<repo>-cargo` (registry, git checkouts, submodule mirrors), `<repo>-target-<branch-slug>`, `<repo>-npm`,
  `botster-core-zig` (Zig cache and the package store `/zig/packages`). Stamps in `~/testq/projects/<repo>/targets/`.
- **Disk guard (job.sh):** at most 8 target volumes per repo (least recently used first, main/v1 last); below 60 GiB free,
  least recently used Botster target volumes of either repo go; below 40 GiB after that, the gate exits 75. A volume used
  in the last 2 hours, or held by a job's in-use lock, is never removed. Target volumes unused for 7 days are removed.
- **fetch-public.sh (core):** with network and no token: the ghostty submodule through a mirror at the pinned commit, then
  cleaned; the Zig packages of `build_data.rs` published atomically (staging + rename, `gzip -t` checked) under a lock.
  `BOTSTER_ZIG_NETWORK_DENIED=1` in the gate container: `unshare --net` cannot work there (Docker seccomp, and the host
  has `kernel.apparmor_restrict_unprivileged_userns=1`; changing either needs sudo, which we do not use).
- **tmpfs /tmp:** the kernel throttles every writer of a container whose dirty page cache is at its limit. A heavy writer
  (rustc, test binaries) stalled the git-heavy `xtask::commands` tests for 2–3 s, four at once. Measured: git ops 6 ms
  quiet, 566 ms median and 1.8 s max under an in-container writer, 13 ms on tmpfs. A bigger memory limit did not help.

## Operational gotchas

- **Never pattern-kill.** Stop a Linux job with `testq cancel <id>` (or end the client: the holder cancels). Stop a Mac
  gate by ending its client (it TERMs its botsterq run) or `botsterq cancel <id>`.
- A branch gets infra fixes only when its head contains them: `job.sh` runs from the snapshot of the head. Tell leads to
  merge main (contracts) or v1 (core) after an infra change.
- The ledger job compares the branch with the current base; an infra branch behind main fails it. Merge main first.
- A background-job test of SIGINT is invalid (bash starts background jobs with SIGINT ignored); test with SIGTERM.
- `rustup` refuses `nightly` as a custom toolchain name: the image symlinks the dated nightly as `nightly`.
- `cargo install --git … --tag` offline needs `refs/remotes/origin/tags/<tag>` in cargo's git database (core fetch.sh).
- Perl's flock fallback (macOS has no flock) must not `exec`: perl marks the descriptor close-on-exec.
- Zig's package fetch is flaky on the loaded Mac (TlsInitializationFailed, HttpConnectionClosing); retry, or let a Linux
  gate fill its own store.
- The macOS-only test list is empty; the client warns when `cfg(target_os = "macos")` appears, and such a PR also runs
  `botster-gate --on mac`.

## Incident history (2026-10-01 to 2026-10-02)

- First runs: the image lacked `procps` (leftover check uses `ps`); zombies of reparented test children needed `--init`.
- Review rounds 1–5 of the gate (PR #1): share enforcement, branch lock, lease, immutable Mac tree, cleared environment,
  pinned base, SKIPPED as failure, holder-owned submit and cancel, strict reads before releasing locks.
- A healthy gate was cancelled at 77 s when a loaded Mac stalled the 15 s heartbeat; now 10 s with a 180 s deadline.
- Disk full (ENOSPC in a core mutants gate): one ~10 GB target volume per branch had grown to ~375 GB. 26 stale volumes
  removed by hand (5.8 → 260 GB free); the disk guard followed (PR #4, core #130).
- botster-contracts' default tier reached its 60 s budget on 4 Linux CPUs (1628 tests); conformance-speed brought it to
  8–15 s. A product budget, owned by the Stage 0 lead.
- P2 libghostty on Linux: every test aborted ("memory allocation failed"): the Zig static library defined `calloc` and
  `free` next to glibc's `malloc`. P2's fork patch 10 (link libc on Linux) addresses it; not verified on Linux at pause.
- Tail stall of `xtask::commands` tests: dirty-page throttling, fixed with the tmpfs /tmp (PR #5, core #131).

## Parked: step 6, self-hosted GitHub Actions runners

Not done. Registering a runner service on the host was refused by the Claude Code permission classifier (persistence),
and org-level runners for trybotster need the `admin:org` scope (`gh auth refresh -h github.com -s admin:org`; the Mac's
token has `repo`, `workflow`, `read:org`). The orchestrator recommended skipping it: the resource-aware gate already
satisfies the merge rule. Do not retry without the orchestrator. If resumed: register 2–3 runners labelled `botster-ci`
under `~/botster-runner-*` as user systemd services (linger is on), and make their jobs call `testq submit --priority`
like HyperFlex's CI; first test whether self-hosted jobs run despite the billing block.

## On resume: is the host healthy?

```sh
ssh structvr@192.168.1.185 'df -h / ; uptime'                       # free disk well above 60 GiB; low load
ssh structvr@192.168.1.185 '~/testq/bin/testq | tail -n 5'          # no Botster job stuck queued or running
ssh structvr@192.168.1.185 'ls ~/testq/projects/botster-locks/leases ~/testq/projects/botster-*/jobs'   # both empty
ssh structvr@192.168.1.185 'docker ps --filter label=testq.job'     # no Botster container without a running job
ssh structvr@192.168.1.185 'docker volume ls | grep -c botster-.*-target-'   # at most 16
~/botster-sessions/shared/tools/botster-gate --on linux ~/botster-sessions/trybotster-botster-contracts-infra-remote-gate
```

The last line gates main on Linux (warm: about 100 s for botster-contracts). Then `botster-gate --on mac` on the same
worktree checks the Mac path. Pausing testq (`testq pause`) also stops HyperFlex; never do it for Botster.
