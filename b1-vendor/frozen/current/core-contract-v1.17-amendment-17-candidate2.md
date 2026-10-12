# Core contract v1.17, Amendment 17 (candidate 2): Core has no WebRTC; a remote binding reaches Core as a `Stream`

The base is the Core contract of `frozen/current` (manifest final35), with Amendments 2 to 16 and Errata 1 to 4, which are unchanged except where A17-4 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. Authority: **a user decision, relayed by the orchestrator on 2026-10-09:** WebRTC moves out of Core and into the Hub. Core keeps only the DP-2 `Stream` transport. The Hub terminates WebRTC and bridges each route's data channel to a Core `Stream`. The Hub side is stated in the Hub→client contract and the route codec (their own amendments), not here.

**Supersedes** candidate 1 (`6f0f46fc…3813`, commit `79d2bf8`), rejected (verdict `28b78cc`). The reviewer found abbreviated quotations in A17-4. Candidate 2 quotes each replaced or removed sentence exactly from its base file and states its new reading next to it. A17-1, A17-2, A17-3, the trade-offs and the id lists are unchanged from candidate 1.

**Why.**
- DP-2 makes the session worker terminate a WebRTC route (`WebRtc{offer, expected_fingerprint}`: DTLS, SCTP and a DataChannel named `terminal`). DP-11 records the reason: zero local hops through the host. It also records the costs: "every worker process links a WebRTC stack and opens network sockets", "a browser with N sessions holds N peer connections instead of one", and "ICE and TURN reachability are per session". It names the Hub-terminated alternative as "**not** what the user's requirement selects".
- The user now selects that alternative. So the worker links no WebRTC stack and opens no network socket, and Core has one route transport.
- DP-2 already says that "any connected, reliable, ordered byte stream handed to the worker (a Unix stream, a future QUIC or TCP/TLS stream) is a `RouteTransport::Stream` and needs **no Core contract change**". A Hub bridge is such a stream. So Core gains no new mechanism. This amendment only removes text.

**Trade-offs, accepted by the user decision.** They are recorded here because DP-1 and DP-11 state the opposite today:
1. **A remote client reconnects when the Hub restarts.** The Hub holds the remote end of a bridged route, so a Hub restart closes it. The worker and its session keep running, and the client attaches again. A local `Stream` route is unchanged: it ends at the worker.
2. **The Hub sees the plaintext route bytes** of a bridged route. DTLS ends in the Hub, and the bridge copies frames between the data channel and the stream.
3. **One extra local hop per direction** for a bridged route: client to Hub, then Hub to worker over a local stream.
In return, the worker has no network-facing surface, and a client shares one peer connection with the Hub for its control channel and all its routes (the layout that the Hub→client contract states, by the orchestrator's decision of 2026-10-09 under the user's delegation).

| # | Clause | Test |
|---|---|---|
| A17-1 | **Core has one route transport, `Stream`.**<br>- `RouteTransport` is `Stream` only (DP-2). There is no `WebRtc` transport, no `AttachWebRtc` operation, no `Answer` value, no `InvalidOffer` code, no `StartFailed{WebRtcFailed}` and no `connect_deadline` option.<br>- `features()` lists `route_transport:stream` and never `route_transport:webrtc` (A2-6, section 13).<br>- Core opens no network socket for a route, and the worker links no WebRTC, DTLS, SCTP or ICE stack. | `conf::a17_1_features_list_only_the_stream_transport`, `conf::a17_1_attach_webrtc_is_not_an_operation` |
| A17-2 | **A host-terminated binding is an ordinary `Stream`.** A host that terminates a remote transport itself (the Hub terminates WebRTC) passes its end of a connected, reliable, ordered byte stream to `attach` as `RouteTransport::Stream`. Core treats it as any other `Stream` route, with no special case:<br>- the framing is Core's stream framing (DP-3);<br>- the progress point is the `Stream` one: "bytes that the kernel accepted from the worker's write to the route socket" (OU-3). The host's bridge has its own bounded queue toward the remote peer, and its backpressure reaches the worker only as a socket the host does not drain. That queue, the remote peer's progress and the peer's authentication are the host's, and its own contract states them;<br>- the route closes as a `Stream` route closes. When the host closes its end (for example, because the remote peer left or the host restarted), the route closes with the reason that a peer close of a `Stream` has today.<br>A stream that the host creates for itself (for example, one end of a socket pair) has no `route_connect` handshake: TB-L2's single-use token admits a client that connects to the host's listener. It is not a step of `attach`. | `conf::a17_2_host_socketpair_route_is_an_ordinary_stream_route` |
| A17-3 | **The data-plane claims are about Core, not about the host.** DP-1's "the host and the Core library are not on the data path" holds for a route whose client holds the other end of the stream (topology T1, DP-10). For a host-terminated binding, the host's own bridge is on the data path, by the host's choice. Core still copies no terminal byte in the host process, and Core's hop counts (DP-11) are counted from the stream that Core receives. | `conf::dp_11_hop_counts_match_the_stated_topologies` (T1 only) |

**Ids.**
- **Withdrawn**, with authority A17-1 (the behaviour moves to the Hub's WebRTC binding, where the Hub→client contract and the codec state it): `conf::dp_2_webrtc_answer_has_all_candidates`, `conf::dp_2_webrtc_only_the_terminal_channel_is_accepted`, `conf::dp_2_offer_must_carry_all_candidates`, `conf::dp_2_webrtc_fingerprint_mismatch_closes_bad_peer`, `conf::dp_2_host_never_reads_the_datachannel`, `conf::dp_3_datachannel_chunking_matches_tr3`, `conf::dp_3_incomplete_datachannel_frame_is_discarded`, `conf::dp_10_t2_latency_and_throughput_budgets`, `conf::ou_2b_connect_deadline_closes_not_connected`, `conf::ou_3_webrtc_progress_is_acknowledged_payload_not_retransmission`, `conf::ou_3_webrtc_send_queue_is_bounded_and_backpressures`, `conf::ou_3_webrtc_stuck_peer_with_retransmissions_is_a_stall`, `conf::a7_1_connect_deadline_out_of_range_is_invalid_input`, `conf::a7_1_chunk_cap_above_sctp_max_is_not_refused_and_chunks_stay_within_it` (14 ids).
- **Withdrawn**, with authority A17-4 item 2: `conf::ou_1_unavailable_binding_auth_is_unsupported_and_never_attached_unauthenticated`. `Stream` has no Core-side peer authentication, so Core has no case of this refusal.
- **Kept, with their WebRTC steps or rows removed** (the ids and their other assertions are unchanged): `conf::dp_11_hop_counts_match_the_stated_topologies` (T1 only), `conf::ou_2b_every_core_close_reason_maps_to_one_wire_reason`, `conf::a2_3_every_route_close_reason_maps_to_one_wire_reason`, `conf::a2_1_every_begin_operation_has_a_row_and_the_documented_ok_value`, `conf::a2_1_not_admitted_state_is_wrong_state_sync_for_every_row`, `conf::a2_1_sync_errors_allocate_no_op_for_every_row`, `conf::a2_1_no_deadline_code_exists` (with a deadline other than `connect_deadline`), `conf::a2_6_features_lists_every_documented_name`, `conf::a2_6_per_worker_features_in_worker_features`, `conf::a5_3_every_code_reachable_or_listed_real_only` (no `InvalidOffer`), `conf::lm_1_zero_limit_is_invalid_config` (no `max_connect_deadline`, `route_send_buffer_bytes`), `conf::dp_5_exception_frames_wait_in_the_route_queue_and_full_queue_stops_input_reads` (its setup no longer sets `route_send_buffer_bytes`).
- **New:** `conf::a17_1_features_list_only_the_stream_transport`, `conf::a17_1_attach_webrtc_is_not_an_operation`, `conf::a17_2_host_socketpair_route_is_an_ordinary_stream_route`.

## A17-4. Text replaced

Each quotation below is exact, with no ellipsis. "Base" is `frozen/current/core-contract-v1.17.md`. "A2" is `core-contract-v1.17-amendment-2-candidate4.md`. "A7" is `core-contract-v1.17-amendment-7-candidate4.md`. Each quotation is followed by **is removed** or by **reads:** and its new text.

1. **DP-2 (base).**
   - The sentence
   > `RouteTransport` is one of: **`Stream`**, a connected stream endpoint (a Unix stream socket, or any connected stream) that the host hands over; or **`WebRtc{offer, expected_fingerprint}`**, where the **worker terminates the peer connection** (DTLS, SCTP, one reliable ordered DataChannel named `terminal`).
   **reads:**
   > `RouteTransport` is **`Stream`**, a connected stream endpoint (a Unix stream socket, or any connected stream) that the host hands over (A17-1, A17-2).
   - The sentence
   > **WebRTC:** `begin(AttachWebRtc{client, session, offer, expected_fingerprint, options})` completes with `Answer{route, answer}`.
   **is removed**.
   - The sentence
   > **No trickle in 0.1, on both sides:** the client's **offer carries every ICE candidate** (the client waits for its gathering to finish) and the worker's **answer carries every candidate** (the worker gathers before it answers), so signaling is one offer and one answer through the host's control plane.
   **is removed**.
   - The sentence
   > The worker accepts **only the DataChannel named `terminal`** (a fixed name: each route has its own peer connection, so no reserved label is needed) on that peer connection, and checks the client's DTLS fingerprint against `expected_fingerprint` (the client's signature over its route id and fingerprint is the host's check, BP-4); a mismatch closes the route `BadPeer`, and any other channel name is refused.
   **is removed**.
   - The sentence
   > The host chooses the signaling and never touches the DataChannel.
   **is removed**.
   - The "Bindings are extensible" sentence and the stream handoff sentences are unchanged, and A17-2 applies to them. The DP-2 test column loses the five ids that the A17 **Ids** list withdraws.

2. **OU-1 (base).**
   - The option
   > `connect_deadline: Duration` (for a WebRTC route: the time the peer has to connect; positive and at most `CoreLimits.max_connect_deadline`, default 60 s; expiry closes the route `NotConnected`)
   **is removed**, with the comma and space that follow it.
   - The text
   > `route_limits{max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes, max_chunk_bytes}`
   **reads:**
   > `route_limits{max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes}`
   - The text
   > `max_history_page_bytes` defaults to 256 KiB; `max_chunk_bytes` applies to a WebRTC route and is at most the SCTP maximum message size)
   **reads:**
   > `max_history_page_bytes` defaults to 256 KiB)
   - The sentence
   > **Attach refusals, before any route is reserved (sync `Unsupported`):** `Unsupported{terminal_format}` when the target worker can emit none of the client's `terminal_formats`; `Unsupported{binding_auth_unavailable}` when the requested binding's peer authentication cannot be performed (for WebRTC, no `expected_fingerprint`; for any binding, no authentication mechanism), so **no binding is ever attached unauthenticated**.
   **reads:**
   > **Attach refusal, before any route is reserved (sync `Unsupported`):** `Unsupported{terminal_format}` when the target worker can emit none of the client's `terminal_formats`. A host authenticates the remote peer of a binding that it terminates before it calls `attach` (A17-2).
   - The text
   > `RouteLimits{max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes, max_chunk_bytes, max_paste_bytes, max_query_bytes, max_query_reply_bytes, max_file_bytes, query_deadline, stall_deadline, stall_close_after, route_input_queue_bytes}`
   **reads:**
   > `RouteLimits{max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes, max_paste_bytes, max_query_bytes, max_query_reply_bytes, max_file_bytes, query_deadline, stall_deadline, stall_close_after, route_input_queue_bytes}`
   - The sentence
   > `max_frame_bytes`, `max_screen_frame_bytes`, `max_history_page_bytes`, `max_chunk_bytes`, `max_paste_bytes`, `max_query_reply_bytes` and `max_file_bytes` are the same on the wire.
   **reads:**
   > `max_frame_bytes`, `max_screen_frame_bytes`, `max_history_page_bytes`, `max_paste_bytes`, `max_query_reply_bytes` and `max_file_bytes` are the same on the wire.
   - The OU-1 test column loses `conf::ou_1_unavailable_binding_auth_is_unsupported_and_never_attached_unauthenticated` (A17 **Ids**). TR-3 chunking on a data channel is the Hub's, and the Hub→client contract states it.

3. **OU-2b (base).**
   - The text
   > `SnapshotTooLarge` → `route_closed{attach_failed{snapshot_too_large}}`; `BadPeer` → `route_closed{attach_failed{bad_peer}}`; `BadFrame`
   **reads:**
   > `SnapshotTooLarge` → `route_closed{attach_failed{snapshot_too_large}}`; `BadFrame`
   - The text
   > (the route may lack `route_closed`): `NotConnected` → `not_connected` (the WebRTC `connect_deadline` passed, OU-1), `HandoffFailed`
   **reads:**
   > (the route may lack `route_closed`): `HandoffFailed`
   - The OU-2b test column loses `conf::ou_2b_connect_deadline_closes_not_connected`. The codec's wire values `bad_peer` and `not_connected` stay, for the Hub's own use.

4. **OU-3(a) (base).**
   - The sentence
   > Today's two: **Stream:** bytes that the kernel accepted from the worker's write to the route socket.
   **reads:**
   > Today's one: **Stream:** bytes that the kernel accepted from the worker's write to the route socket.
   - The sentence
   > **WebRTC:** new route payload that **leaves the bounded SCTP send queue through transport acknowledgment** (the cumulative acknowledgment advances); **not** a retransmission, a heartbeat, a successful UDP write, or the application's own enqueue, none of which proves that the peer is receiving (retransmissions can continue while the peer is stuck).
   **is removed**.
   - The sentence
   > **The WebRTC send queue is bounded** (`CoreLimits.route_send_buffer_bytes`, default 1 MiB): the worker enqueues no more than that beyond the acknowledged point, exactly as a full socket buffer stops a Unix write, and that bound is part of route backpressure (OU-3b, OU-3d).
   **is removed**.
   - The rule that every `RouteTransport` binding states its progress point, and that a binding without one cannot be added, is unchanged. A17-2 states the `Stream` point for a host bridge. The OU-3 test column loses the three `ou_3_webrtc_*` ids.

5. **DP-3 (base).**
   - The text
   > (the query steps, the route part of TS, TR-3, A.1, A.4 to A.6 and the close mapping)
   **reads:**
   > (the query steps, the route part of TS, A.1, A.4 to A.6 and the close mapping)

   TR-3 is data-channel chunking, which Core no longer does.
   - The sentence
   > **On a WebRTC DataChannel the framing is the client contract's TR-3, exactly:** each message is `[u8 flags][chunk]`, bit 0 of `flags` is `final` and the other bits are 0; a frame is the concatenation of its chunks up to and including the one marked `final`, **without the length prefix** (`[u8 type][body]`); each chunk is at most `max_chunk_bytes` (chosen by the host at attach, at or below the SCTP maximum message size); a frame that is incomplete when the channel closes is discarded.
   **is removed**.
   - The sentence
   > The per-type frame limits below apply to the reassembled frame, and a frame is refused as soon as it passes its limit, before more chunks are buffered.
   **reads:**
   > The per-type frame limits below apply to each frame.
   - The text
   > , and on a WebRTC route TR-3 chunking carries it past the SCTP message limit
   **is removed**.
   - The text
   > limits: {max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes, max_chunk_bytes, max_paste_bytes, max_query_reply_bytes, max_file_bytes, query_client_deadline_ms, stall_deadline_ms, stall_close_after_ms}
   **reads:**
   > limits: {max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes, max_paste_bytes, max_query_reply_bytes, max_file_bytes, query_client_deadline_ms, stall_deadline_ms, stall_close_after_ms}
   - The DP-3 test column loses `conf::dp_3_datachannel_chunking_matches_tr3` and `conf::dp_3_incomplete_datachannel_frame_is_discarded`. The `attached.limits` change needs the agreed codec revision (DP-3: "A change to any of it needs a new extract and hash agreed by both pairs").

6. **Section 5.4, the preamble (base).** After the sentence
   > **The claims below are stated for defined topologies (DP-10); there is no unqualified "minimum latency" claim.**
   add:
   > A remote binding that the host terminates (the Hub's WebRTC) adds one local hop per direction, by the user's decision of 2026-10-09 (A17). Core's claims are stated for the stream that Core receives (A17-3).

7. **DP-1 (base).** The sentence
   > **After an attach, the host and the Core library are not on the data path, and Core copies no terminal byte in the host process.**
   **reads:**
   > **After an attach, the host and the Core library are not on the data path of a route whose client holds the other end of the stream (T1, DP-10), and Core copies no terminal byte in the host process.** For a host-terminated binding, the host's own bridge is on the data path, by the host's choice (A17-3).

8. **DP-10 (base).** The sentence
   > **Topologies:** **T1** (local): a client process on the same machine, a connected Unix stream route, a session running a byte echo program and a bulk writer; **T2** (WebRTC, worker-terminated): the same with a loopback DataChannel.
   **reads:**
   > **Topology:** **T1** (local): a client process on the same machine, a connected Unix stream route, a session running a byte echo program and a bulk writer. Core states T1 only. A host that bridges a remote binding states its own topology (A17-3).

   The DP-10 test column loses `conf::dp_10_t2_latency_and_throughput_budgets`.

9. **DP-11 (base).** Each of these three sentences **is removed**:
   > **T2 (WebRTC):** client to worker to PTY over the network with the DTLS termination **in the worker**; **zero local hops through the host.**

   > The alternative that **Hub-terminated WebRTC forwards to the worker over a local socket adds one local copy per direction** (estimated tens of microseconds; to be measured), and is **not** what the user's requirement selects.

   > **Costs of the worker-terminated choice, stated so nobody is surprised:** every worker process links a WebRTC stack and opens network sockets (a larger network-facing surface in each worker, reduced by the DTLS fingerprint pin of DP-2); a browser with N sessions holds N peer connections instead of one; and ICE and TURN reachability are per session.
   The sentence
   > **T1 (local):** client to worker to PTY; **zero hops through the host.**
   is unchanged, and A17-3 applies to it. A17's trade-offs record the costs of the Hub-terminated choice.

10. **Section 5.4, "What exists today, and what is new" (base).** The sentence
   > New: descriptor handoff, the route wire and its codec, worker-side route I/O and state machine, worker WebRTC termination, host-epoch fencing, and route adoption.
   **reads:**
   > New: descriptor handoff, the route wire and its codec, worker-side route I/O and state machine, host-epoch fencing, and route adoption.

   The text "no WebRTC stack" in the same paragraph is unchanged.

11. **9B `CoreLimits` (base).** Each of these three rows **is removed**:
   ```text
   | `max_connect_deadline` | 60 s | OU-1: the largest WebRTC `connect_deadline` a host may give an attach. |
   ```

   ```text
   | `default_route_chunk_bytes` | 16 KiB | OU-1: the WebRTC chunk size when the host gives none; range 1 KiB to the transport maximum. |
   ```

   ```text
   | `route_send_buffer_bytes` | 1 MiB | OU-3a: the most unacknowledged payload the worker keeps in a WebRTC route's send queue. |
   ```
   In the ranges paragraph, the text
   > `max_file_bytes` 1 KiB to 1 GiB; `route_send_buffer_bytes` 4 KiB to 64 MiB; `max_paste_bytes` 1 KiB to 64 MiB.
   **reads:**
   > `max_file_bytes` 1 KiB to 1 GiB; `max_paste_bytes` 1 KiB to 64 MiB.

12. **Section 13, item 2 (base).** The sentence
   > `features()` lists `route_transport:stream` and `route_transport:webrtc`.
   **reads:**
   > `features()` lists `route_transport:stream`.

13. **Amendment 2.**
   - **A2-1, the value types (A2).** The text
   > `Facts` (ST-7); `Answer{route, answer}` (DP-2); `SpawnReport` (SV-4).
   **reads:**
   > `Facts` (ST-7); `SpawnReport` (SV-4).
   - **A2-1, `StartFailReason` (A2).** The text
   > `StartFailReason` (non-exhaustive) = `ExecFailed{errno}`, `CwdMissing`, `StartupTimeout`, `WorkerFailed`, `GuardianFailed`, `WebRtcFailed`.
   **reads:**
   > `StartFailReason` (non-exhaustive) = `ExecFailed{errno}`, `CwdMissing`, `StartupTimeout`, `WorkerFailed`, `GuardianFailed`.
   - **A2-1, the operations table (A2).** The row
   ```text
   | `AttachWebRtc` | `client`, `session`, `offer`, `expected_fingerprint`, `options` | `Answer` | as `attach`, plus `InvalidInput` for an `options` field | `InvalidOffer{reason}`, `StartFailed{WebRtcFailed}` | as `attach` |
   ```
   **is removed**.
   - **A2-1, the deadlines (A2).** The text
   > (A2-5); a WebRTC `connect_deadline` closes the route `NotConnected` (OU-1); a `query_deadline` hands the query to the shadow (EV-8).
   **reads:**
   > (A2-5); a `query_deadline` hands the query to the shadow (EV-8).
   - **A2-1, the new codes (A2).** The sentence
   > **New codes (additive, 9.3).** `UnknownService` (caller, sync: the id names no service); `InvalidOffer{reason: bad_offer | offer_without_candidates}` (caller, **async**: the worker parsed the offer and refused it).
   **reads:**
   > **New code (additive, 9.3).** `UnknownService` (caller, sync: the id names no service).
   - **A2-3, the closed set (A2).** The text
   > `BadFrame{code}` (`code` is one of the codec's `frame_too_large`, `malformed_frame`, `unknown_frame`, `invalid_input`); `BadPeer`; `NotConnected`; `SnapshotTooLarge`;
   **reads:**
   > `BadFrame{code}` (`code` is one of the codec's `frame_too_large`, `malformed_frame`, `unknown_frame`, `invalid_input`); `SnapshotTooLarge`;
   - **A2-3, the wire mapping (A2).** The sentence
   > `Detached`, `Replaced`, `Revoked`, `SessionEnded`, `SessionRemoved`, `SnapshotTooLarge`, `BadPeer` and `BadFrame` map as OU-2b says.
   **reads:**
   > `Detached`, `Replaced`, `Revoked`, `SessionEnded`, `SessionRemoved`, `SnapshotTooLarge` and `BadFrame` map as OU-2b says.

     The text
   > The failed-route reasons map to the codec's route-ended reasons: `NotConnected` → `not_connected`, `HandoffFailed` → `handoff_failed`,
   **reads:**
   > The failed-route reasons map to the codec's route-ended reasons: `HandoffFailed` → `handoff_failed`,
   - **A2-6, the feature table (A2).** The row start
   ```text
   | `route_transport:stream`, `route_transport:webrtc` | Core |
   ```
   **reads:**
   ```text
   | `route_transport:stream` | Core |
   ```
   The rest of the row is unchanged.
   - **A2-10, item 1 (A2).** The text
   > Section 9.3: `DeadlineExpired` removed; `UnknownService` and `InvalidOffer` added;
   **reads:**
   > Section 9.3: `DeadlineExpired` removed; `UnknownService` added (`InvalidOffer`, which A2 added, is removed by A17);

14. **Amendment 7.**
   - **A7-1 (A7).** The sentence
   > It applies to `attach` and to `AttachWebRtc` alike.
   **reads:**
   > It applies to `attach`.
   - **A7-1 (A7).** The rule
   > `connect_deadline` that is zero, or above `CoreLimits.max_connect_deadline`;
   **is removed**.
   - **A7-1 (A7).** The bullet
   > **Not refused: a bound known only from the transport.** `route_limits.max_chunk_bytes` above the WebRTC route's SCTP maximum message size is not an error. **The worker never sends a chunk larger than the smaller of the two.** The SCTP maximum is the one negotiated for that route's data channel (the offer's `max-message-size`) (steward ruling R-9.2; HC TR-3: "chosen at or below the SCTP maximum message size"). No result or read changes. On a stream route, `max_chunk_bytes` does not apply.
   **is removed**.
   - **A7-2, item 2 (A7).** The item
   > **A2-1, the `AttachWebRtc` row:** "plus `InvalidInput` for an `options` field" reads with A7-1.
   is void, because item 13 removes that row.
   - **A7, Why (A7).** The two list items
   > `connect_deadline` "positive and at most `CoreLimits.max_connect_deadline`";

   > A2-1 names "`InvalidInput` for an `options` field" only in the `AttachWebRtc` row;
   are a record of the reason for A7. They are not normative and are unchanged.

15. **Steward ruling R-9.2** (the transport maximum of `default_route_chunk_bytes`) no longer applies to Core.

Every other clause is unchanged.
