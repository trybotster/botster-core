//! Canonical runnable-entrypoint Hub connection contract artifacts.

/// Named JSON fixture for downstream conformance tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnableEntrypointHubConnectionFixture {
    /// Stable fixture name.
    pub name: &'static str,
    /// Canonical fixture JSON.
    pub json: &'static str,
}

/// Draft 2020-12 JSON Schema for the runnable-entrypoint Hub connection.
pub const SCHEMA_JSON: &str =
    include_str!("../../fixtures/runnable-entrypoint-hub-connection/schema.json");

/// Canonical valid Unix-domain-socket descriptor.
pub const VALID_UNIX_SOCKET_JSON: &str =
    include_str!("../../fixtures/runnable-entrypoint-hub-connection/valid/unix-socket.json");

/// Valid descriptors that every consumer must accept.
pub const VALID_FIXTURES: &[RunnableEntrypointHubConnectionFixture] =
    &[RunnableEntrypointHubConnectionFixture {
        name: "unix_socket",
        json: VALID_UNIX_SOCKET_JSON,
    }];

/// Invalid descriptors that every consumer must reject.
pub const INVALID_FIXTURES: &[RunnableEntrypointHubConnectionFixture] = &[
    RunnableEntrypointHubConnectionFixture {
        name: "missing_transport",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/missing-transport.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "unknown_root_field",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/unknown-root-field.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "malformed_transport",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/malformed-transport.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "unknown_transport",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/unknown-transport.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "missing_path",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/missing-path.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "extra_transport_field",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/extra-transport-field.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "blank_path",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/blank-path.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "whitespace_path",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/whitespace-path.json"
        ),
    },
    RunnableEntrypointHubConnectionFixture {
        name: "relative_path",
        json: include_str!(
            "../../fixtures/runnable-entrypoint-hub-connection/invalid/relative-path.json"
        ),
    },
];
