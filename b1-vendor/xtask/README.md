# xtask

Repo tooling. Run a command with `cargo xtask <command>` (an alias in `.cargo/config.toml`).

| Command | What it does |
|---|---|
| `ledger` | Writes `conformance/ledger.json` from the current contract files. |
| `ledger --check [--base <git-ref>]` | Regenerates the ledger in memory and checks it against the checked-in file, the transcripts and `conformance/pending.txt`. |
| `manifest` | Checks the sha256 of every file that `frozen/final6/MANIFEST.md` lists. |
| `taint` | Scans every tracked file for the names in `docs/taint-denylist.txt`. |
| `timers` | Fails on an unmarked sleep in test code, and on any timer in the production code of a fake. |
| `test-budget [--slow [--deadline <d>]] [--compare <file>]` | Runs a test tier under cargo-nextest and enforces its budgets. |
| `clause-lint` | Checks the clause citations in the fake crates against the ledger. |
| `codegen [--check]` | Writes, or checks, the generated artifacts. |
| `replacement-map [--check]` | Writes, or checks, `docs/fake-replacement-map.md` from `conformance/replacement-map.json`. Fails when an id, an R-1.4 fault or a Core error code is unmapped. |
| `ci [--job <name>] [--keep-going] [--list]` | Runs every job of the merge gate; `.github/workflows/ci.yml` runs the same jobs one by one (README "The merge gate"). |
| `real-only [--check]` | Writes, or checks, the README list of ids that a fake does not prove (`fake: structural`). |

## The ledger

`contracts.toml` lists the current contract files in owner order (core, codec, hc, hp, lb, ui, cli) and the superseded files that the ledger ignores.
Every `.md` file in `frozen/final6/` must be in one of the two lists.
The ledger holds one row for each distinct `conf::<id>`, sorted by id:

| Field | Meaning |
|---|---|
| `id` | The test id. |
| `contract` | The owning prefix: the first contract in owner order that names the id. |
| `clause` | The clause id of the row, bullet or section that holds the id. |
| `file`, `section` | The owner file, and the nearest heading above the id. |
| `also_in` | The other files that name the id. |

A `conf::` token that ends in `_` (for example `conf::cli_*`) is a wildcard mention, not an id.

**Attribution rules** (the code is `xtask/src/extract.rs`):

1. A heading starts a section and clears the clause. When the heading starts with a clause id (`## A2-5. Services`), that id is the clause.
2. A table row whose first cell is a clause id sets the row clause. A cell may span several lines. Lines that do not start with `|`, and one-cell lines that start with `|`, belong to the row above.
3. A bullet or a line that starts with a bold clause id (`- **LB-2 Arguments.**`) sets the section clause.
4. An id takes the row clause, or else the section clause.
   A row that is keyed by something else (for example an error code) never borrows the clause of an earlier row.
5. A clause id has letters, digits, `-` and `.`, and at least one digit.

An id that gets no clause makes `ledger` fail, and the message lists the id with its file, line and section.
The fix is a `[[clause_override]]` in `contracts.toml`: an `id_prefix`, the `clause`, and a `note` that says why.
An override applies only to an id that the rules cannot attribute. An override that matches no such id fails the command.

## `ledger --check`

It fails when:

- the regenerated ledger differs from `conformance/ledger.json`;
- a ledger id has no transcript and is not in `conformance/pending.txt`, `conformance/deferred.txt` or `conformance/withdrawn.txt`;
- a ledger id is in more than one of the four states (a transcript, pending, deferred, withdrawn);
- a transcript has an id that is not in the ledger, or its file name is not `<id>.json`, or its `contract` or `clause` differs from the ledger row;
- `pending.txt` lists an id that has a transcript, or an id that is not in the ledger;
- with `--base <ref>`: `pending.txt` lists an id that the file at `<ref>` does not list, and the ledger at `<ref>` already had. The file may only shrink, except for ids that the ledger gained since `<ref>` (an accepted amendment adds its new ids as pending). A base without `pending.txt` counts as an empty file.

- a deferred or withdrawn id is not in the ledger, a line is malformed, an authority is no clause of its own contract in the current manifest (`Core A6-2` needs a `core` ledger row with clause `A6-2`, or a table row `| A6-2 |` in a current `core` file that lists no test id, as a clause that only withdraws does; another contract's `A6-2` cannot authorize it), a withdrawn id's replacement is not a ledger id (or is itself, or is withdrawn), or a not-applicable case names an id that is not active;
- the rules of a line (the lead's ruling): **every line, old or new, cites an authority that is a clause of the current manifest, qualified by its contract.** A line added or changed since `--base <ref>` cites it too, whether the file of that clause is new or already existed. Edits and removals of existing lines are allowed under those rules, and the reviewer checks their meaning. A base without the files counts as an empty file: there is no exemption for the first lines. A deferred id that leaves the file must then have a transcript (Core A6-2: the id must pass when its start condition holds; `pending.txt` may only shrink, so the id cannot return to it). The replacement map must agree (`replacement-map --check`: a deferred id has the proof `deferred:<its reason>`, a withdrawn id has the proof `withdrawn`).

A transcript is any `*.json` under `conformance/`, except `ledger.json`, `engine-vectors/` and `fixtures/` directories. Files under `crates/botster-ui-contract/vectors/` are transcripts of `ui` ids.
Phase 0 is done when `pending.txt` is empty. Then the file is deleted.

## `clause-lint`

A fake crate (`crates/botster-fake-*` and `crates/botster-plugin-fakes`) cites the clause that requires each behavior, in a comment.
A citation is a comment (`//`, `///` or `//!`) whose text starts with a contract prefix and a clause id:

```text
// HC EN-8
// Core OR-3, OR-4
/// Clause: HC EN-1, HC A.1, Core 9B
```

- Prefixes: `Core`, `Codec`, `HC`, `HP`, `LB`, `UI`, `CLI`.
- After the first clause id, a comma may add more clause ids, or a new prefix with its clause id.
- The first clause id has at least one digit. A later clause id in a list has a digit and a letter.
- A comment that starts with a prefix and no clause id (`// HC frames are sent`) is not a citation.
- A cited clause is valid when a contract file of that prefix holds a clause row or a heading with that id, and the ledger has a test id under it. A clause that an id override names is also valid.

The lint prints `file:line` for each unknown clause and fails.
It does not check that a branch cites a clause. The reviewer checks that rule.

## `codegen`

`xtask/src/codegen.rs` holds a registry of (output path, generator function) pairs. `codegen` writes each output. `codegen --check` fails when a file on disk differs from what its generator makes.
A later feature adds its schemas to the registry.

## `timers`

A timer is `sleep(`, `setTimeout(` or `setInterval(` (a whole word, outside a `//` comment). The command scans tracked `.rs`, `.ts`, `.tsx`, `.js` and `.mjs` files, except `frozen/` and `docs/`. The xtask crate is scanned too.

- Test code is a file under a `tests`, `test` or `__tests__` directory, a `*_test.rs`, `*.test.*` or `*.spec.*` file, and and test items as below.
  In a Rust file, test code inside `src` is an item (a module, a function, a `use`) with `#[cfg(test)]`, `#[test]` or `#[tokio::test]`, from its attribute to the end of the item (brace matching; a multi-line raw string that holds a brace can confuse it). Production items after a test module are not test code.
  A timer in test code needs `// timer: deadline — <reason>` on the same line or the line before. The reason must not be empty.
- In a fake crate (`crates/botster-fake-*`, `crates/botster-plugin-fakes`), a timer outside test code always fails, marked or not.

## `test-budget`

`cargo xtask test-budget` builds the tests, then runs `cargo nextest run` with the profile `default` (`ci` when `CI` is set).
The profiles are in `.config/nextest.toml`; each writes JUnit to `target/nextest/<profile>/junit.xml`, and the xtask reads the times from it.

It fails when:

- nextest fails; a test over 2 s is terminated by `slow-timeout`, and a test that leaves a child that holds its output pipes fails by `leak-timeout`;
- a test time in the JUnit file is over 2 s;
- the run takes over 60 s (the run is killed at 60 s); the build time does not count;
- a process outlives the run (below).

It prints every test time, slowest first, and writes `target/test-times.json` (`{ "tier", "wall_secs", "tests": { "<binary> <test>": seconds } }`).
`--compare <file>` reads an earlier `test-times.json` and prints every test that at least doubled (a test with a zero baseline counts when it now takes more than zero). A test that is not in the earlier file is new. It never fails on a doubling.

**Leftover processes.** The xtask starts nextest in its own process group, with `BOTSTER_TEST_PIDFILE` set. nextest gives each test its own group, so the group alone cannot find a test's leftovers. A run wrapper (`.config/test-wrapper.sh`, a nextest `run-wrapper`) records each test pid before the test starts; the group id of a test is its pid. A tracker thread also watches the process tree while the run goes.
A process belongs to the run when it is the root, a descendant of the run (the tracker saw its parent), or in the group of a recorded test pid. Age, command names and other processes never decide ownership.

After nextest exits, and before any other step that can fail, the xtask:

1. prints each owned process that is still alive (pid, group and command);
2. kills exactly those pids and the group that it started;
3. fails the run.

If the process list is not readable, it kills only its own group and fails. An unrelated process is never reported or killed.
Limits: a pid that the OS reuses as a recorded group id during one run would be taken as owned; a process that a test moves to a new process group is found only if the tracker saw it as a descendant. A real-process test owns and cleans its group (BUILD.md Testing rule 10). The xtask does not forward Ctrl-C to the run group.

**Slow tier.** `--slow` runs the `slow*` test targets of the packages that have a `slow` feature (README, "Tests: tiers and budgets"), with the profile `slow` (no per-test limit).
`--deadline <90s|10m|2h>` (default 10m) kills the run when it passes. With no such package, the command succeeds.

## Deferred and withdrawn ids

`conformance/deferred.txt` and `conformance/withdrawn.txt` hold the two states that a ruling can give an id besides "has a transcript" and "is pending". Fields are separated by two or more spaces; `#` lines and blank lines are skipped.

```
conf::<id>  <reason>  (<Contract clause>)  until: <end condition>                     deferred.txt: the id waits for a release condition (Core A6-2)
not-applicable conf::<id>  <case>  (<Contract clause>)  because: <why>               deferred.txt: a case of an ACTIVE id that has no subject; not an id state
conf::<id>  withdrawn by <Contract clause>  replaced by conf::<id>|none             withdrawn.txt: the id is withdrawn (Core A9-2)
```

The replacement map keeps an entry for a withdrawn id with the proof `withdrawn` (it stays a ledger id because the frozen text still names it, and the map covers every ledger id; the entry makes the withdrawal visible and checkable, and removing it would break the completeness check).
