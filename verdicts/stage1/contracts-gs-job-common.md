# Contracts #45: shared gate script

## Round 1 — 2026-10-11

- Repository: `trybotster/botster-contracts`.
- Branch: `stage1/gs-job-common`.
- Exact head: `56e5b744588b98ccbbb892d6b0a63522317284f0`.
- Base: `7176d02b577c9043c72ca4eda3b293104aad6961`.
- Tree: `36b00c4db40b542dd69e279a8ff877d49fd39826`.
- Risk: HIGH. The lead assigns this review under the steward's drift ruling (b), step 1.
- Package verdict: `07b31fddfa0678acb1d712040453e66807380223`, `verdicts/gs-gate-speed.md`, section “Contracts PR #45”.

### Source and boundaries

Sol reads the complete three-file delta, both scripts, the published body, and the supplied evidence script. The base is an ancestor. The diff check passes.

The executable `job.sh` keeps its strict shell options and settings. It sources the adjacent common file in the same shell. The common file needs no executable mode. Positional arguments, commands, mounts, traps, and exit paths remain intact.

An independent static comparison finds only the artifact comment and invocation changes after the common file's guard and header. The new array contains the exact three old paths in the same order. The `sh -c` invocation supplies `sh` as argument zero. Quoted `"$@"` then supplies all three paths. The copy command, destination naming, mounts, and saved command status remain unchanged. Artifact failure cannot replace the saved command status.

The disk decisions, container limits, private fetch, lease handling, cleanup, and retention remain unchanged. A missing common file fails before a container starts. The project guard rejects the supplied standalone invocation without settings.

The pinned old Core tree `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has no remote job script. The body states this prior-art result. The change uses existing Bash sourcing, arrays, and filesystem tools. It introduces no new scheduler or dependency.

Contracts owns the common file. The body and handoff assign Core a byte copy at its next pin move. Core will retain its stage-2 artifact path in its settings. Core will add the byte comparison check. This PR completes only Contracts step 1. It does not supply or approve the future Core copy, check, or pin move. Contracts gets no check under the ruling.

No schema, manifest, lock pin, pending list, test selection, or mutation acceptance rule changes.

### Supplied evidence

Focused log: `~/botster-sessions/gates/botster-contracts-stage1-gs-job-common-56e5b744-pool-20261011-063127-22026.log`.

SHA256: `addfe627c2a1771924037a5383206c0468be26b0f99f101c9316ca821b9cfdde`.

Sol reads the complete log and statically decodes its complete evidence script. Sol executes neither script. All three input hashes match the exact Git blobs.

- All nine disk scenarios match the old exits and normalized Docker run arguments.
- Failed disk decisions exit 75 before container start.
- A failed main command returns exit 3 in both versions.
- The artifact invocation retains argument zero and all three paths.
- The extracted artifact scripts produce the same file names from the supplied fake tree.
- Other Docker run arguments match.
- Missing-common and standalone cases exit 1 before container start.
- The evidence reports zero failures and zero job and wrapper exits.

The artifact comparison checks file names. It does not independently compare copied content hashes. The unchanged `cp -a` command supports the source conclusion. These fixtures do not prove behavior on a real low-disk node.

Full gate: `~/botster-sessions/gates/botster-contracts-stage1-gs-job-common-56e5b744-pool-20261011-063138-24237.log`.

SHA256: `10974fc1e6b211f5d44dfdd620ffb134a7a0df913e2c45b4fb8db2ff1f6e1ec4`.

The header names the exact head and base. The Linux pool job runs the split script. All ten steps pass in 336.1 seconds. The job and wrapper exit zero after 387 seconds. The default suite reports 2076 passed and zero skipped. The slow suite reports 33 passed and zero skipped. TypeScript reports 235 passed; both consumer tests pass. The route decoder completes 940158 fuzz runs in 61 seconds. The mutation step reports no changed Rust source.

Sol reads the complete package section and verifies that `origin/stage1/review-gs` contains its commit. The package and integration conclusions agree. A GitHub fetch later fails because DNS cannot resolve GitHub. The local artifact remains available; no new publication is inferred from that failure.

### Verdict

Zero findings remain open. This verdict covers only the exact Contracts head, base, source, body, and supplied evidence. The steward retains the merge decision and current-base check.

Sol ran no build, test, gate, mutation, reversal, measurement, or executable base-check job.

VERDICT: CLEAN (0 open)
