# Hub→client contract, Amendment 5 (candidate 2): the real Hub in-process and one transcript player replace FakeHub

Owner: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01. **This file is the amendment, in the post-freeze amendment path.** The base is the Hub→client contract of `frozen/current`, with Amendments 1 to 4, which are unchanged except where A5-5 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes candidate 1** (`5ac1c322…b067`, commit `bc94c71`), which the Amendment reviewer rejected (`verdicts/amendments.md`, 2026-10-01):
1. **A5-3 separates two kinds of transcript.** A **shared** transcript states Hub semantics and passes against the real Hub. A **client-only** transcript feeds a client an input that the current Hub may never emit: an unknown frame, an unknown field, a future enum value, or a malformed or cut frame. It cites the clause that obliges the client, and it asserts only the client's response.
2. **A5-4 separates producer variations from delivery variations.**
   - A different resync cut, shutdown moment, response order or `route_ended` timing is produced at the source, by the real Hub's injected scheduler, or by a scripted alternative in the player.
   - The transport edge varies only delivery, and it keeps every stream's order, batches, baselines and frame boundaries valid.

Companion change: Core Amendment 5. Decision: the orchestrator, under the user's instruction "if it's better long term, let's do that", adopted the hybrid of the steward, the Foundation lead and the Amendment reviewer.

**Why.** Section 12 requires a hand-written FakeHub in Rust and in TypeScript, which makes a second and a third implementation of the Hub, and they drift. This amendment keeps every property that section 12 asks of FakeHub:
- the clauses as transcripts;
- the same semantic cases in every client, through its own transport adapter;
- seeded interleaving;
- the fault hooks;
- real bindings;
- Web's tests in TypeScript.

The Hub's semantics come only from the **real Hub**. The clients' independent counterpart is a **player of the hand-written transcripts**, never a reimplementation.

| # | Clause | Test |
|---|---|---|
| A5-1 | **The real Hub in-process.** The Rust runner drives the **real Hub** over every binding (unchanged). It also drives the real Hub **in-process**, over the Hub's internal in-memory transport, for the shared transcripts.<br>- The in-memory transport is test plumbing, not a binding. It never appears in `welcome.features` as `binding:*` (TB-0), and it never satisfies a binding clause.<br>- The Hub runs on Core's testkit (Core Amendment 5), so a Hub run needs no worker process, no PTY and no sleeps. | `conf::a5_1_real_hub_in_process_runs_the_suite`, `conf::a5_1_in_memory_transport_is_not_a_binding` |
| A5-2 | **The transcript player.** **One implementation**, in Rust, compiled to wasm for the TypeScript runner and published from the same contract release (one version, one source).<br>- **What it does:** it plays the **server role** of a transcript. It matches each client frame against the transcript's client frames, using the suite's matchers, and sends the transcript's server frames. A client frame that the transcript does not expect fails the test.<br>- **What it does not do:** it holds **no Hub logic**. It computes no state, permission, watch or route behavior that a transcript does not script.<br>- **Bindings:** it serves the **real bindings** that the client's adapter uses (TB-0: `local-stream`, `webrtc`, and the partial `mcp`). The client under test drives it **through its own transport adapter**, never through an internal shortcut.<br>- **Faults:** it executes the scripted faults that a transcript names, as listed for FakeHub (steward ruling R-1.4). | `conf::a5_2_player_serves_transcripts_without_hub_logic`, `conf::a5_2_unexpected_client_frame_fails`, `conf::a5_2_player_serves_real_bindings`, `conf::a5_2_one_player_rust_and_wasm` |
| A5-3 | **Transcripts are the oracle. There are two kinds**, and each transcript declares its kind:<br>- **Shared:** it states Hub semantics. It runs against the **real Hub** in Hub CI, in the client role, and the player may serve it to clients. Every shared transcript that the player serves must pass against the real Hub.<br>- **Client-only:** it feeds the client a server input that a clause obliges the client to handle, but that the current Hub need not emit. Examples:<br>&nbsp;&nbsp;- an unknown optional field, an unknown error code, or an unknown enum value, including a future operation state (EN-6);<br>&nbsp;&nbsp;- an unknown frame type or a malformed frame (EN-6, A.1);<br>&nbsp;&nbsp;- unusual limits (BD-3);<br>&nbsp;&nbsp;- a transport lost mid-frame (TS-9).<br>&nbsp;&nbsp;It **cites that clause**, it **asserts only the client's response**, and it is never run as an assertion about the Hub. Only the player serves it.<br>Both kinds are written **by hand from the clauses**. A recording of a real Hub run is **never an oracle**. It may be a reviewed regression aid, and it is never regenerated into an expected result. | `conf::a5_3_shared_transcripts_pass_on_the_real_hub`, `conf::a5_3_client_only_transcripts_cite_a_clause_and_assert_only_the_client` |
| A5-4 | **Seeded interleaving, by source.** Section 12's list of open orders stands. Each item has one source.<br>**Producer variations** (they change what the Hub emits, so they arise where the Hub decides):<br>- where resync snapshots fall (WT-3): the Hub takes its baseline from a valid source state at a seed-chosen point;<br>- response order (EN-3): the order in which concurrent requests complete;<br>- the timing of `route_ended` against the session watch update (TS-9);<br>- the moment of `hub_shutdown`: when the Hub shuts down, with the EN-3 answers to each outstanding request.<br>On the **real Hub**, these come from its injected, seeded scheduler (BUILD.md, sans-IO), plus shuttle for internal schedules. In the **player**, they are **scripted alternatives** in the transcript. Each alternative is a valid variant with clause-cited expectations, and the seed picks one.<br>**Delivery variations** (they change only how already-produced frames reach the client): the batching of frames per read, the chunking of route frames (TS-3, TR-3), and the interleaving of independent streams (watches, events, responses, routes; WT-8). A **permute-only** decorator on the transport edge produces them, for the real Hub and for the player alike. It **keeps** each stream's own order, every batch and baseline whole and in order (WT-3), frame boundaries within each frame's bound, and the shutdown answers before `close` (EN-3). It never invents, alters or drops a frame. A dropped frame is only a scripted fault (A5-2). | `conf::a5_4_producer_variants_are_valid` (each alternative passes its clause expectations), `conf::a5_4_delivery_variation_keeps_stream_order_batches_and_frames` |

## A5-5. Text replaced

1. **Section 12, the heading** "Conformance and FakeHub" becomes "Conformance".
2. **Section 12, the runners:**
   - "a Rust runner that drives a real Hub over every binding, and the Rust FakeHub" becomes "… over every binding, and in-process (A5-1)";
   - "a TypeScript runner that drives the TS FakeHub" becomes "a TypeScript runner that drives the transcript player (A5-2)".
3. **Section 12, the FakeHub bullet** is replaced by A5-1 to A5-3.
4. **Section 12, "Seeded interleaving"** comes from A5-4.
5. **Section 12, "Client CI":** clients run the client-side checks against **the transcript player**, with shared and client-only transcripts.
6. **Section 12, "Changing the contract":** "the transcript and the fake behavior" becomes "the transcript, and the player's scripted fault or alternative where the clause needs a new one".
7. **WT-8's test note** "(FakeHub seeds)" becomes "(A5-4 seeds)".
8. **BD-3's test note:** "FakeHub sends unusual limits" becomes "the player sends unusual limits (a client-only transcript)".
9. **Every other mention of FakeHub** in this contract, and in the informative CLI spec (CLI-P8, CLI-CV1), means the transcript player of A5-2. The CLI-CV1 run over a real Unix socket is the player on the `local-stream` binding.

**Not changed:** the Hub→plugin contract's FakeHub and FakePlugin (HP section 19), which are already frame-script players; the real-binding and acceptance runs of section 12; and every other clause.
