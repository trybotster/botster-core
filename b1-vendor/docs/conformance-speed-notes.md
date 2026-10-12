# conformance-speed notes

Feature: the default test tier finishes in under 60 s (BUILD.md, Testing, rule 4). The budget is not changed.

## Profile

Nextest runs each test in its own process. A generated conformance test ran this code in that process:

1. Start the test binary.
2. Parse every transcript of the embedded directory (`load_dir`). 589 Core transcripts, 500 HP transcripts.
3. Look up its own id in the result.
4. Run the one transcript.

Step 2 was the cost. Each test paid for the parse of all transcripts, so the total cost grew with the square of the transcript count.
The plugin runner paid a second cost: it compiled the JSON Schema of every HP frame type (`HpSchemas::new`) in each process, and used a few.

Measured on the Mac (debug build, one test per process, `time` of the test binary with `--exact`):

| Step | Core test | HP test |
| --- | --- | --- |
| Before: parse all transcripts, compile all frame schemas | 0.101 s | 0.121 s (mean of the tier) |
| After the parse change only | 0.005 s | 0.049 s |
| After the lazy schema change | 0.005 s | 0.029 s (mean of the tier) |

Measured on the Linux gate (4 test threads), from the gate logs:

| | Before (main b2868f0, the lead's numbers) | After |
| --- | --- | --- |
| Default tier wall time | 56.0 s | 9.4 s |
| Mean per transcript test, `core-conformance::fake` and `plugin-conformance::fakes` | 0.19 s | 0.021 s |

The Mac default tier went from 21.7 s (Core fixed, HP not yet) to 10.2 s.

## Change

1. `botster_conformance::load_one(dir, id)` parses only the file named `<id>.json` (design 5.1: the file stem equals the id). It skips `fixtures` directories, as `load_dir` does. Two files with that name in different directories are an error, as a duplicate id is in `load_dir`. `run_one_test` and the HP `run_test` use it.
2. `HpSchemas` compiles the schema of a frame type on its first use. A used schema is the same validator as before. A name that is not a frame type gives the same error as before ("no schema registered").

No transcript, seed, validation check or assertion changed. Each id keeps its test name, its own failure report, and the 2 s per-test limit. `load_dir` is unchanged and still runs the whole-directory checks (duplicate ids, `because` citations, clause existence).

## Tool choice (BUILD.md rule 0)

The pin names libtest-mimic, `LazyLock` and a nextest test group as options. The cost was not in the process start or in shared setup that a static could share. A `LazyLock` cannot share state across nextest processes, and batching transcripts would lose the per-id process limits. The fix removes the work that no test needs. No new dependency.

## Projection

The cost per transcript test is now below 0.05 s. A test parses one transcript, so the parse cost no longer grows with the transcript count. `load_one` still walks the directory entries to find its file name, so the search cost still grows with the directory size. That walk compares file names only. It is small against the parse (it is inside the 0.005 s of the Core test above, at 589 entries), and the measured projection below includes it only by extrapolation.

At 2,400 transcripts (1,089 now, about 1,100 to come, plus the other tests):

- Linux: the 1,089 transcript tests sum to 22.5 s. At 0.021 s each, 2,400 sum to about 50 s. The other tests sum to about 20 s. With 4 threads the wall time is about 18 s, against the 60 s budget.
- The Mac is within the same range (0.017 s and 0.029 s per test).

The remaining mean cost per transcript is the test process start and the transcript run. The tier needs no further change before the next transcripts land.
