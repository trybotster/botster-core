# ClientWorker terminal egress and subscription teardown

Core ClientWorker is the sole terminal egress and teardown authority for
**adapter-bound** subscriptions.

This is an advanced host/adapter seam. The embedder start-here path remains
spawn → attach → drain → input → shutdown. Bind is not part of that prelude.

## Ownership

Core owns:

- Subscription identity `(session_id, subscription_id, generation)`
- The only subscription egress queue and slow-client policy
- Incremental attach phases (READY / PAGE / FINISH) and input/resize barriers
- Mechanical detach when an adapter returns `Closed`
- Control-plane inventory without terminal state

Hub owns host session policy: spawn, worktrees, retention, and cleanup after
process exit. ProcessExited closes terminal subscriptions. It does not shut
down the host session.

## Bind sequence

1. `attach` assigns a monotonic generation and publishes an inventory row with
   `adapter_bound=false`.
2. `bind_terminal_adapter` must present that live generation. Pre-attach bind
   is a typed error. There is no reservation generation.
3. After bind, that route's terminal frames leave only through
   `TerminalAdapter::try_write`. `drain` / `drain_subscription` do not also
   return those terminal frames.
4. Unbound subscriptions keep today's `TransportEgress` drain path.

## Queue and retry

ClientWorker queue capacity is `QueueSource::ClientWorker.default_capacity()`
(512). A new frame that would exceed 512 fails the subscription. The same head
frame fails after 512 unsuccessful `try_write` results (`WouldBlock` or
`Full`), or 512 host ticks where an accepted write stays non-Ready, on host
pump ticks. Terminal frames never retry. Recovery is detach plus a fresh attach.

A lost READY / PAGE / FINISH / other snapshot frame fails that subscription.
There is no terminal-frame replay helper.

## Delivery and ProcessExited

`Ok(())` occupies the adapter's one-slot write. A frame is delivered only after
that active write completes (pressure returns to Ready, or an auto-completing
adapter is already Ready).

On session `ProcessExited`, each live subscription:

1. Delivers remaining output
2. Delivers `process_exit`
3. Then Core calls `close()` and drops the adapter on the same host tick

If the 512 write budget expires or the adapter returns `Closed` first, Core
fails that subscription without claiming `process_exit` was delivered.

## Close hard stop

There is no ClientWorker OS thread. The hard stop is ownership teardown, then
a contractually non-blocking `close()`, then drop of the adapter on the same
host tick. `close()` and `Drop` must not wait for transport I/O. Core does not
spawn a closer thread. A blocking `close()` is an adapter defect and fails the
published conformance harness.

## Inventory

`list_terminal_subscriptions()` reports `client_id`, `session_id`,
`subscription_id`, `generation`, and `adapter_bound`. It does not report
attach phases, snapshot bytes, queue contents, or decoder state.

## Production path

`CoreDaemon::bind_terminal_adapter` plus the existing host tick
(`CoreDaemon::drain` / `drain_runtime_once`) pumps ClientWorker. Unix and
WebRTC adapters are later Hub tickets.
