# Hub→client contract, Amendment 5 (candidate 1): the real Hub in-process and one transcript player replace FakeHub

Owner: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01. **This file is the amendment, in the post-freeze amendment path.** The base is the Hub→client contract of `frozen/current`, with Amendments 1 to 4, which are unchanged except where A5-5 names a replacement. Other contracts are cited by contract name and clause id only.

**Decision.** The orchestrator, under the user's instruction "if it's better long term, let's do that", adopted the hybrid of the steward, the Foundation lead and the Amendment reviewer. The companion change is Core Amendment 5.

**Why.** Section 12 requires a hand-written FakeHub in Rust and TypeScript. That means a second and a third implementation of the Hub, and they drift. This amendment keeps every property that section 12 asks of FakeHub:
- the clauses as transcripts;
- the same semantic cases in every client, through its own transport adapter;
- seeded interleaving;
- the fault hooks;
- real bindings for the clients;
- Web's tests in TypeScript.

The Hub's semantics come only from the **real Hub**. The clients' independent counterpart is a **player of the hand-written transcripts**, never a reimplementation.

| # | Clause | Test |
|---|---|---|
| A5-1 | **The real Hub in-process.** The Rust runner drives the **real Hub** over every binding (unchanged). It also drives the real Hub **in-process**, over the Hub's internal in-memory transport, for the semantic transcripts.<br>- The in-memory transport is test plumbing, not a binding. It never appears in `welcome.features` as `binding:*` (TB-0), and it never satisfies a binding clause.<br>- The Hub runs on Core's testkit (Core Amendment 5), so a Hub run needs no worker process, no PTY and no sleeps. | `conf::a5_1_real_hub_in_process_runs_the_suite`, `conf::a5_1_in_memory_transport_is_not_a_binding` |
| A5-2 | **The transcript player.** **One implementation**, in Rust, compiled to wasm for the TypeScript runner and published from the same contract release (one version, one source).<br>- **What it does:** it plays the **server role** of a transcript. It matches each client frame against the transcript's client frames, using the suite's matchers, and it sends the transcript's server frames. A client frame that the transcript does not expect fails the test.<br>- **What it does not do:** it holds **no Hub logic**. It computes no state, no permission, no watch and no route behavior that a transcript does not script.<br>- **Bindings:** it serves the **real bindings** that the client's adapter uses (TB-0: `local-stream`, `webrtc`, and the partial `mcp`). The client under test drives it **through its own transport adapter**, never through an internal shortcut.<br>- **Faults:** it executes the scripted faults that a transcript names, as listed for FakeHub (steward ruling R-1.4): drop the transport mid-route, a partly written frame, a frame that cannot finish, a dropped committed response, and so on. | `conf::a5_2_player_serves_transcripts_without_hub_logic`, `conf::a5_2_unexpected_client_frame_fails`, `conf::a5_2_player_serves_real_bindings`, `conf::a5_2_one_player_rust_and_wasm` |
| A5-3 | **Transcripts are the oracle.**<br>- Transcripts are written **by hand from the clauses**. Every transcript that the player serves also passes against the **real Hub** in Hub CI, in the client role. So the player and the Hub are checked against the same text, and neither can drift from it.<br>- A recording of a real Hub run is **never an oracle**. It may be a reviewed regression aid, and it is never regenerated into the expected result. | `conf::a5_3_served_transcripts_pass_against_the_real_hub` |
| A5-4 | **Seeded interleaving.** Section 12's list of open orders stands. Its source is a **permute-only decorator** on the transport edge. The decorator serves both the in-process real Hub and the player. It varies:<br>- response order (EN-3);<br>- order across watches, events and route frames (WT-7);<br>- where resync snapshots fall (WT-3);<br>- batching per read;<br>- route chunking (TS-3, TR-3);<br>- the timing of `route_ended` against the session watch update (TS-9);<br>- the moment of `hub_shutdown`.<br>It never invents, alters or drops a frame except by a scripted fault (A5-2). The Hub's internal schedules are explored with the adopted scheduler tool (shuttle). | `conf::a5_4_seed_varies_only_open_orders`, `conf::a5_4_decorator_is_permute_only` |

## A5-5. Text replaced

1. **Section 12, the heading** "Conformance and FakeHub" becomes "Conformance".
2. **Section 12, the runners:**
   - "a Rust runner that drives a real Hub over every binding, and the Rust FakeHub" becomes "… over every binding, and in-process (A5-1)";
   - "a TypeScript runner that drives the TS FakeHub" becomes "a TypeScript runner that drives the transcript player (A5-2)".
3. **Section 12, the FakeHub bullet** is replaced by A5-1 to A5-3.
4. **Section 12, "Seeded interleaving"** comes from A5-4.
5. **Section 12, "Client CI":** clients run the client-side checks against **the transcript player**.
6. **Section 12, "Changing the contract":** "the transcript and the fake behavior" becomes "the transcript, and the player's scripted fault where the clause needs a new one".
7. **WT-8's test note** "(FakeHub seeds)" becomes "(A5-4 seeds)".
8. **BD-3's test note:** "FakeHub sends unusual limits" becomes "the player sends unusual limits".
9. **Every other mention of FakeHub** in this contract, and in the informative CLI spec (CLI-P8, CLI-CV1), means the transcript player of A5-2. The CLI-CV1 run over a real Unix socket is the player on the `local-stream` binding.

**Not changed:** the Hub→plugin contract's FakeHub and FakePlugin (HP section 19), which are already frame-script players; the real-binding and acceptance runs of section 12; and every other clause.
