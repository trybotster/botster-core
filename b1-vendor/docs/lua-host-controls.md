# Lua host test controls (feature lb-cli-suite)

Design 6.3 defines test controls: inputs and observers that only a test can cause. The Lua binding runner (`botster-lua-conformance`) drives a
`LuaHostSubject`: the real Hub's Lua host, loaded in-process with the package of the transcript. The runner plays the Hub with HP frames, so the
frame controls are the host controls of `botster-plugin-conformance` (`HOST_CONTROLS`). This file names the controls a transcript of the suite uses and the
obligation each puts on the harness that adapts the real host. A transcript lists the controls it uses in `requires.controls`. A subject without one
gives `unsupported_control`, which is never a pass.

Every control is built from injected parts of the host (the clock, the random source, the package files, the channel), never from a branch in production
code (BUILD.md, Testing rule 8), and is never a contract obligation.

## The package (`setup.package`)

`setup.package` is `{manifest, files}`. `manifest` is the complete `botster-plugin.json` object. `files` maps each path relative to the package root to its
text. The harness stages every `*.lua` file (LB-17b), records the manifest, and runs the entrypoint when the runner sends `Hello`. `LuaHostSubject::load_package`
is the one method that takes it. The frame templates (`$ref`) are the HP ones of `conformance/hp/fixtures/frames.json`.

## Controls

| Control | Arguments | Result | Edge | Used for |
|---|---|---|---|---|
| `settle` | `conn?` | none; a `{"settled": true}` item follows the frames of the connection | scheduler | The host runs until it has no more work. It is the barrier of `expect_quiet_until`: a frame that the host would send is already in the queue (HP LH-1; LB-2, LB-6, LB-10, LB-11, LB-15, LB-16b, LB-17b, LB-21) |
| `status`, `received`, `counters` | as in HP `HOST_CONTROLS` | as in HP | channel | Observers of the frames that the host received and sent; no Lua transcript of this feature needs them yet |
| `admit_package` | `package: {manifest, files}` | `{error?: {kind, detail}}`: the admission of a complete package by the host's loader. Absent `error` means the package was admitted | package files | LB-17 (an unknown manifest key, `$schema`), LB-18 (`manifest_bad_version`), LB-19 (the agent and accessory definition files). The package is not run |
| `load_entrypoint` | `package: {manifest, files}` | `{error?: {kind, detail}}`: the host loads the package and runs its entrypoint, as for `Hello`, without a Hub peer; an absent `error` means the registration was accepted | package files | LBA2-1: a tool, prompt or resource entry that sets `handler`, sets an unknown field or lacks a required one fails the load `invalid_request` |
| `readable_globals` | none | `{names: [string]}`: **the complete set of global names that plugin code can read** in the host's environment, whatever the representation (stored keys, names supplied through `__index`, and so on) | environment | HP LH-4. The transcript passes this set and `Ready.host.globals` to the plugin, which compares them in both directions and also looks up every published name for real (`_ENV[name]`). The contract fixes the readable names, not how an environment stores or enumerates them, so the harness reports the set rather than the script enumerating a table |
| `package_session_types` | none | `{package_root, session_types: [{id, label, command, args?, role?, interaction?, lifecycle?, traits?, script?, session_type_dir}]}` of the last admitted package | package files | LB-19: what the packaged definitions became. `session_type_dir` is the package root; `role` is `accessory` for a `botster-accessory.json` |

## Where a refusal is decided (steward ruling R-27)

LB-2 (an unknown argument key), LB-15 (`web.set_origin{}` with the key absent), LB-11 (an `http.request` method outside the seven) and LB-16b (an unknown spawn key) fix the **outcome** that the plugin sees:
`{ ok = false, error = { kind = "invalid_request", effect = "none", .. } }`. They do not fix where the check runs. A host may validate locally or forward the call and map the Hub's refusal. The
transcripts accept both paths (the step `answer_host_call` answers a forwarded call and leaves every other item alone) and never assert that a `HostCall` appears or does not appear.
A `detail.code` is asserted only where a clause fixes it.

See `docs/lb-cli-suite-notes.md` for the assumptions that the reviewer should check.
