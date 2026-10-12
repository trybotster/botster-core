# contracts #101 — steward/b5-build (backlog B-5 step 2: the rulings-only rule also covers `verdicts/amendments.md` and new `amendments/*.md` files), reviewer 017f

HIGH (BUILD.md merge rule). Author: the steward. Package reviewer: 016c. Reviewed head: `aa7bef8f` (one commit on main `80f022e9`). Landing head: `192d70bc`, the steward's merge of main `5ce2cff1`. Only `docs/BUILD.md` changes.

## Round 1: CLEAN on 192d70bca1fdfbb1948f636cf4820ee081a2df9b, with the Q7 receipt

### The change

The exception "a rulings-only main moves no gate" now covers three kinds of change, each by its `git diff --name-status` letter:
- `M docs/steward-rulings.md`;
- `M verdicts/amendments.md`;
- `A amendments/<name>.md` (a new file only).

Step 3 of the steward's check prints the merge's `--name-status` diff. Every line must be one of the three forms, and every path must end in `.md`. Any other status under `amendments/`, any other file under `verdicts/`, or any other extension needs a gate. This includes my #98 note (a code file under these paths) as part of the condition.

### The safety claim, checked on 192d70bc

"No gate step reads these files":
- `taint` skips `docs/`, `amendments/` and `verdicts/` (`xtask/src/taint.rs:14` `SKIPPED_PREFIXES`, from #98).
- `timers` scans only `.rs`, `.ts`, `.tsx`, `.js` and `.mjs` files (`xtask/src/timers.rs:18` `SCANNED_EXTENSIONS`, and `:156`).
- `crate-symbols` reads only the files that `contracts.toml` lists, and `contracts.toml` lists no path under `amendments/` or `verdicts/`. The `verdicts/…` keys in `crate-symbols/*.toml` are `n/a` text rows. Nothing opens them.
- `git grep` on the head over `xtask`, `crates`, `ci`, `.github`, `*.sh`, `Cargo.toml` and `contracts.toml` finds no other code or config that names `amendments/` or `verdicts/`, apart from `taint.rs`.
- The A24 code of #96 (`TESTKIT_PROVEN_ROWS`) reads `frozen/current/…`, not `amendments/`.

### Note (not a finding)

Step 3 checks the net diff of the merge. A file that is added and then changed in two commits on main shows as one `A`, so the check cannot see the `M`. Nothing reads these paths, so this is a policy gap only. The steward agrees, and records it in backlog B-5.

### Q7 receipt: 192d70bca1fdfbb1948f636cf4820ee081a2df9b

- **The parents** are the reviewed head `aa7bef8f` and main `5ce2cff1`. Tree `77f63bc0` is equal to `git merge-tree --write-tree` of the parents.
- **The diff from main** is `docs/BUILD.md` only. Its stable patch-id `593419b1` is equal to the patch-id of `aa7bef8f` against its base `80f022e9`. `git diff aa7bef8f 192d70bc -- docs/BUILD.md` is empty.
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-steward-b5-build-192d70bc-pool-20261012-133716-18682.log`, sha256 `a127294ce9600376d5b5b80f99b43bfe60382d6206fcfd6f95aa18abedda5507`. All ten steps PASS. Exit 0 after 422 s on gaming, base main `5ce2cff1`.
- The first four gate attempts (three on `aa7bef8f`, one on `192d70bc`) were infrastructure failures (exit 75 on gaming, exit 141 on "no node"), not results. The pool lead cleared gaming before this run.
- **Main** is still `5ce2cff1`, so main fast-forwards to `192d70bc`.

CLEAN on `192d70bca1fdfbb1948f636cf4820ee081a2df9b`. Q7 receipt given.
