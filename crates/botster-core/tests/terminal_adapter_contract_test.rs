//! Public terminal adapter contract compilation and type tests.

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_core::transport::{TransportEgress, TransportIngress};

#[test]
fn public_types_are_reachable_without_prelude() {
    fn assert_object_safe(_: &dyn TerminalAdapter) {}
    let _ = assert_object_safe;
    let _ = TerminalAdapterWriteError::WouldBlock;
    let _ = TerminalAdapterWriteError::Full;
    let _ = TerminalAdapterWriteError::Closed;
    let _ = TerminalAdapterPressure::Ready;
    let _ = TerminalAdapterPressure::WouldBlock;
    let _ = TerminalAdapterPressure::Full;
    let _ = TerminalAdapterPressure::Closed;
}

#[test]
fn write_error_and_pressure_are_exhaustive_at_0_1_0() {
    match TerminalAdapterWriteError::Closed {
        TerminalAdapterWriteError::WouldBlock => {}
        TerminalAdapterWriteError::Full => {}
        TerminalAdapterWriteError::Closed => {}
    }
    match TerminalAdapterPressure::Ready {
        TerminalAdapterPressure::Ready => {}
        TerminalAdapterPressure::WouldBlock => {}
        TerminalAdapterPressure::Full => {}
        TerminalAdapterPressure::Closed => {}
    }
}

#[test]
fn adapter_trait_does_not_reuse_transport_frame_enums() {
    let _ingress = std::any::type_name::<TransportIngress>();
    let _egress = std::any::type_name::<TransportEgress>();
    assert_ne!(
        std::any::type_name::<TerminalAdapterWriteError>(),
        std::any::type_name::<TransportEgress>()
    );
    assert_ne!(
        std::any::type_name::<dyn TerminalAdapter>(),
        std::any::type_name::<TransportEgress>()
    );
}

#[test]
fn rustdoc_names_the_scaffold_boundary() {
    let docs = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/contract/terminal_adapter.rs"
    ));
    assert!(docs.contains("advanced host/adapter seam"));
    assert!(docs.contains("TransportEgress"));
    assert!(docs.contains("spawn → attach → drain → input → shutdown"));
}
