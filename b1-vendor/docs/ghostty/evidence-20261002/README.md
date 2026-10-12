# Evidence for the Ghostty upstream sync of 2026-10-02

These files are the raw evidence for `docs/ghostty/upstream-sync-20261002.md`.
Local scratch paths are replaced with `<scratchpad>`.

## Files

| File | What it is |
|---|---|
| `mac-run.sh` | The recorded Mac run: it prints the date, host, head, the count of tracked changes and the Zig path and version, then runs each command and prints its exit status. |
| `mac-recorded-run.log` | The output of `botsterq run --label "ghostty sync mac recorded" --exclusive --deadline 60m -- mac-run.sh <scratchpad>` (botsterq exit 0). Head `3f8eb6810bb673aa782b047de21783ac81fb1121`, 0 tracked changes, Zig 0.16.0 from mise. |
| `mac-first-run-test-lib-vt.log` | The first passing Mac `zig build test-lib-vt --summary all` at the same head, without a header. Same counts as the recorded run. |
| `linux-job-8055.sh`, `linux-job-8055.log` | testq job 8055: the archive symbols, a C link test and `test-lib-vt` (the log keeps only the last 40 lines of the test output, so it has no Build Summary line). The log has the job id on line 1. |
| `linux-job-8056.sh`, `linux-job-8056.log` | testq job 8056: the Rust link program before and after patch 10, and `test-lib-vt` with its Build Summary. |
| `linux-job-8059.sh`, `linux-job-8059.log` | testq job 8059, the recorded Linux run: it prints the sha256 of every input before extraction (on the host and again inside the container), then the allocator symbols, the Rust link program before and after patch 10, and `test-lib-vt` with its Build Summary. This run is the Linux evidence of record. |
| `repro.c`, `repro.rs` | The C and Rust link programs that the Linux jobs compile. |

## Which commit each Linux job built

**Job 8059 (recorded).** Its log prints, before extraction, the sha256 of `new.tar` and `pre10.tar` in the testq
snapshot and again inside the container. They equal the `git archive` hashes in the table below. The log also prints
the sha256 of `job.sh` (`1b6f2b84…3012b`) and `repro.rs` (`2a3ad17d…c5b9`); the committed `linux-job-8059.sh` and
`repro.rs` have the same sha256.

**Jobs 8055 and 8056 (limitation).** Their scripts and logs do not record input hashes. I made their tarballs with
`git archive`, copied them with `scp` and submitted them with `testq submit`; the only record of that is this session,
and testq deleted each snapshot after its job. The sha256 values below were computed after those jobs, from the
tarballs that I kept locally. Their provenance therefore rests on that local copy, not on a job record. Job 8059 repeats
every Linux check of 8055 and 8056 except the C link test, with the same results.

Each job received `new.tar` and `pre10.tar` made with `git archive --format=tar <commit>` (git 2.50.1).
The sha256 of each file equals the sha256 of a fresh `git archive` of the commit:

| Tarball | Commit | sha256 |
|---|---|---|
| `new.tar` | `3f8eb6810bb673aa782b047de21783ac81fb1121` | `f08c4c7e87b5a0c91a649d41431ef4409e8db6cf58f19c8e43afc72de53f238f` |
| `pre10.tar` | `eafd0967b` (patch 9, the parent of patch 10 `f0d70e3f1`) | `56828b7718924fb65f7d8719b01282ea08e6c2d17ee0e0178c85b58019d0bee1` |

Jobs 8055, 8056 and 8059 ran on the gate host's testq with 4 CPUs, under the Botster slot lock `slot.3`, in image
`botster-core-gate:bbc12a44852f`. Job 8055 ran as root inside the container, so the host-side cleanup trap could not
delete the snapshot. The 70654 resulting `rm: cannot remove … Permission denied` lines are removed from
`linux-job-8055.log` (the last line of the log says so). I removed the snapshot with one container
(`docker run --rm … rm -rf`), and jobs 8056 and 8059 ran as the host user.
During job 8059 my local follow client stopped at a 10-minute local time limit, which released the `slot.3` lock
while the job kept running; I followed the job again with `testq follow 8059` to its end. `linux-job-8059.log` is line 1
of the first client (`job=8059`) followed by the complete output of the second follow.

## Mac runs that are not in these files

- The first Mac test attempt failed before any test ran: the Zig package fetch failed with `TlsInitializationFailed`.
  Its log was overwritten by the rerun, so it is not kept.
- `zig build --fetch=all` then failed once and succeeded on the second try.
- The first Mac library build (exit 0) printed nothing. The recorded run repeats it.

## Action audit

Writes:

| Repository or host | Action |
|---|---|
| trybotster/ghostty | One push: `git push origin HEAD:refs/heads/botster/upstream-sync-20261002` (a new branch, `3f8eb6810`). After the sync, `git ls-remote origin 'refs/heads/botster/*'` shows the earlier branches at their earlier heads: `botster/vt-core-stage1-c` `ada251c5e`, `-b` `85a8d8eb1`, `botster/vt-core-stage1` `c78b4beb4`, `botster/p2-old-stack-backup` `1fd093bd3`. No force-push. |
| trybotster/botster-contracts | Push of the new branch `docs/ghostty-upstream-sync-20261002` and PR #6 to `main`, as the task requires ("Write the analysis … as a PR to botster-contracts main"). |
| trybotster/botster-core | None. Only `git fetch origin` and `git show` / `git ls-tree`. |
| ghostty-org/ghostty (upstream) | None. Only `git fetch ghostty-org main` and read-only `gh search prs`, `gh search issues` and `gh pr view`. No comment, issue, pull request or reaction. |
| Gate host | Created `~/testq/projects/ghostty-sync`, ran testq jobs 8055 and 8056, ran one cleanup container, then removed the directory. Created it again for job 8059, then removed it. No container with the label `testq.job=ghostty-sync` remains. |

The Ghostty `AGENTS.md` rule "Never create a PR" belongs to the ghostty repository. I created no pull request in
trybotster/ghostty or in ghostty-org/ghostty.

## botster-core pin check (2026-10-02, after `git fetch origin`)

| Ref | Head | Ghostty submodule |
|---|---|---|
| `origin/v1` | `f96a9dfde243cd9a125c0d1d553ede284a89d596` | none (no `crates/botster-terminal-ghostty/vendor/ghostty` entry) |
| `origin/stage1/p2-libghostty` | `f711184885f933723c64124a1eaaa3bbc547b51f` | `ada251c5e99cdb753de8bf72d5d1f307d474518f`, `.gitmodules` branch `botster/vt-core-stage1-c` |

Both are unchanged by this sync.

## Upstream search scope (read-only, 2026-10-02)

- `gh search prs --repo ghostty-org/ghostty "<query>" --limit 6` (all states): `startHyperlink`,
  `OSC 777 notification libghostty`, `paste bracketed libghostty`, `query reply libghostty`, `terminfo libghostty`,
  `OSC 52 selection`, `modifyOtherKeys libghostty`, `link_libc linux libghostty`, `calloc libghostty`, `OSC 5522`,
  `keypad equal`, `snapshot decoder kitty image`, `vt_write_until`, `XTSHIFTESCAPE`, `mouse encode pixel libghostty`,
  `F26 key`, `hyper meta key encode`.
- `gh search prs --repo ghostty-org/ghostty "<query>" --state open --limit 8`: `libghostty allocator`,
  `libghostty static library linux`, `notification source`, `bracketed paste encode`, `query`,
  `terminal query callback`, `clipboard selection`, `key encoder`, `kitty clipboard ack`, `snapshot`.
- `gh search issues --repo ghostty-org/ghostty "<query>" --limit 5`: `libghostty calloc`, `libghostty linux static`,
  `libghostty query`, `hyperlink use after free`, `libghostty paste`, `libghostty terminfo`.
- `gh pr view` of #13332, #14439, #14508, #14483, #14362, #14200, #14515 (title, state, files, first 700 characters
  of the body).

Result: no search result implements one of our patches.
The issue searches returned no results.
The search covers these keyword queries and result limits only; it is not an exhaustive review of upstream.
