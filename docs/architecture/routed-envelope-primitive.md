# Routed Envelope Primitive

`botster-core` owns a transport-neutral routed envelope primitive for multiplexer coordination. It gives hosts and plugins stable endpoint identity, typed target routes, ordered cursors, bounded per-target queues, delivery state, acknowledgement state, topic/stream fanout, and backpressure observations.

This is not a built-in messaging product. Core does not define chat, questions, `post_message`, `receive_messages`, Project Pipelines gates, workflow policy, auth, persistence, retention, or UI behavior. Those meanings belong in hub host profiles and plugins that compose this primitive.

## Core Contract

The public contract lives in `botster_core::routed_envelope`:

- `EndpointId` and `EnvelopeId` identify generic endpoints and envelopes.
- `EnvelopeTarget` names typed routes for endpoints, clients, sessions, subscriptions, plugins, streams, and topics.
- `RoutedEnvelopePayload` carries typed content metadata and opaque bytes. `BoundaryJson` is available only as an extension-owned schema field, not for core routing, delivery, cursor, or queue controls.
- `EnvelopeCursor` is an in-memory monotonic cursor assigned when a target copy is queued.
- `EnvelopeDeliveryStatus` and `EnvelopeDeliveryState` track policy-free states such as queued, delivered, acknowledged, dropped, expired, failed, and backpressured.
- `RoutedEnvelopeQueueConfig` bounds each target queue so one slow target cannot block unrelated targets.

## Engine Behavior

`RoutedEnvelopeRouter` is a pure in-memory router. It can:

- subscribe and unsubscribe targets from stream or topic routes;
- publish one envelope to direct targets or current subscribers;
- drain target queues after an optional cursor with a caller-selected limit;
- acknowledge one delivered envelope for one target;
- report per-target backpressure without retry, retention, or product policy.

`MultiplexerEngine` wires the same behavior through public facade methods: `subscribe_envelopes`, `unsubscribe_envelopes`, `publish_envelope`, `drain_envelopes`, and `acknowledge_envelope`. That makes the primitive reachable from the assembled core multiplexer path while still leaving concrete host tools above core.

## Relationship To Notifications

`NotificationInbox` remains a notification-specific specialization. The routed envelope primitive intentionally lives beside it instead of replacing it. Notifications keep their existing public content and severity vocabulary, while generic coordination payloads use typed envelope routes and opaque payload bytes.

## Host Conformance

`botster-core-test-support` exposes `host_coordination_envelope_fixture` as a hub-facing helper. It builds a synthetic host-owned coordination payload on top of the generic envelope contract so downstream hosts can prove native tools and plugin tools share the same core route/delivery primitive without core learning workflow semantics.
