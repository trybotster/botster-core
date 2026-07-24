//! Downstream conformance for the published runnable-entrypoint Hub connection.

use botster_core::package::{
    RunnableEntrypointHubConnection, RunnableEntrypointHubConnectionTransport,
};
use botster_core_test_support::fixtures::runnable_entrypoint_hub_connection::{
    INVALID_FIXTURES, SCHEMA_JSON, VALID_FIXTURES, VALID_UNIX_SOCKET_JSON,
};

#[test]
fn downstream_consumer_decodes_and_validates_every_canonical_valid_fixture() {
    for fixture in VALID_FIXTURES {
        let connection: RunnableEntrypointHubConnection = serde_json::from_str(fixture.json)
            .unwrap_or_else(|error| {
                panic!("valid fixture {} must deserialize: {error}", fixture.name)
            });
        connection.validate().unwrap_or_else(|error| {
            panic!("valid fixture {} must validate: {error}", fixture.name)
        });
    }

    let connection: botster_core::RunnableEntrypointHubConnection =
        serde_json::from_str(VALID_UNIX_SOCKET_JSON).expect("flat facade must decode fixture");
    assert_eq!(
        connection.transport,
        RunnableEntrypointHubConnectionTransport::UnixSocket {
            path: "/var/run/botster/hub.sock".to_string()
        }
    );
    assert_eq!(
        serde_json::to_value(connection).expect("re-serialize canonical fixture"),
        serde_json::json!({
            "transport": {
                "type": "unix_socket",
                "path": "/var/run/botster/hub.sock"
            }
        })
    );
}

#[test]
fn downstream_consumer_rejects_every_canonical_invalid_fixture() {
    for fixture in INVALID_FIXTURES {
        if let Ok(connection) =
            serde_json::from_str::<RunnableEntrypointHubConnection>(fixture.json)
        {
            assert!(
                connection.validate().is_err(),
                "invalid fixture {} must fail semantic validation",
                fixture.name
            );
        }
    }
}

#[test]
fn published_schema_pins_the_same_closed_transport_and_path_contract() {
    let schema: serde_json::Value =
        serde_json::from_str(SCHEMA_JSON).expect("schema must be valid JSON");

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"], serde_json::json!(["transport"]));

    let transport = &schema["properties"]["transport"]["oneOf"][0];
    assert_eq!(transport["additionalProperties"], false);
    assert_eq!(transport["required"], serde_json::json!(["type", "path"]));
    assert_eq!(
        transport["properties"]["type"]["const"],
        serde_json::json!("unix_socket")
    );
    assert_eq!(transport["properties"]["path"]["minLength"], 1);
    assert_eq!(transport["properties"]["path"]["pattern"], "^/");
}
