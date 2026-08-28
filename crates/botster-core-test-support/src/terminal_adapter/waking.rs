//! Waking-adapter conformance laws.

use std::time::Duration;

use botster_core::contract::terminal_adapter::{TerminalAdapter, TerminalAdapterWriteError};
use botster_core::contract::terminal_wake::{
    TerminalWakeKind, TerminalWakeSource, WakingTerminalAdapter,
};
use botster_core::{SessionId, SubscriptionId, TerminalSubscriptionGeneration};
use botster_terminal_protocol::TerminalFrame;

use super::TerminalAdapterHarnessDriver;

/// Prove waking-adapter laws against `driver`.
pub fn assert_waking_terminal_adapter_conformance<D>(driver: &mut D)
where
    D: TerminalAdapterHarnessDriver + Default + WakingTerminalAdapter,
{
    assert_writable_wake_on_capacity_return(driver);
    assert_close_emits_one_closed_wake::<D>();
    assert_transport_death_emits_closed::<D>();
    assert_no_writable_after_closed::<D>();
    assert_rejected_write_recovery(driver);
    assert_wake_coalesces::<D>();
    assert_idempotent_close::<D>();
}

fn opaque_frame(marker: &str) -> TerminalFrame {
    let json = serde_json::json!({
        "type": "terminal_output",
        "marker": marker,
    });
    TerminalFrame::from_bytes(json.to_string().as_bytes()).expect("opaque fixture frame")
}

fn bind_driver<D: WakingTerminalAdapter>(driver: &mut D) -> TerminalWakeSource {
    let source = TerminalWakeSource::new();
    let sink = source.bind_route(
        SessionId("wake-conformance".into()),
        SubscriptionId("sub".into()),
        TerminalSubscriptionGeneration(1),
    );
    driver.set_wake_sink(sink);
    source
}

fn assert_writable_wake_on_capacity_return<D>(driver: &mut D)
where
    D: TerminalAdapterHarnessDriver + WakingTerminalAdapter,
{
    let source = bind_driver(driver);
    let frame = opaque_frame("occupy");
    assert_eq!(driver.adapter().try_write(&frame), Ok(()));
    let idle = source.wait_wakes(Duration::from_millis(0));
    assert!(
        idle.adapter_routes.is_empty(),
        "occupying the slot must not emit Writable"
    );
    driver.complete_active_write();
    let batch = source.wait_wakes(Duration::from_millis(0));
    assert_eq!(
        batch.adapter_routes.len(),
        1,
        "capacity return must emit one coalesced Writable wake"
    );
}

fn assert_close_emits_one_closed_wake<D>()
where
    D: TerminalAdapterHarnessDriver + Default + WakingTerminalAdapter,
{
    let mut driver = D::default();
    let source = bind_driver(&mut driver);
    driver.adapter().close();
    driver.adapter().close();
    let batch = source.wait_wakes(Duration::from_millis(0));
    assert_eq!(
        batch.adapter_routes.len(),
        1,
        "close must emit one Closed wake"
    );
}

fn assert_transport_death_emits_closed<D>()
where
    D: TerminalAdapterHarnessDriver + Default + WakingTerminalAdapter,
{
    let mut driver = D::default();
    let source = bind_driver(&mut driver);
    driver.force_closed();
    let batch = source.wait_wakes(Duration::from_millis(0));
    assert_eq!(
        batch.adapter_routes.len(),
        1,
        "transport death must emit one Closed wake"
    );
}

fn assert_no_writable_after_closed<D>()
where
    D: TerminalAdapterHarnessDriver + Default + WakingTerminalAdapter,
{
    let mut driver = D::default();
    let source = bind_driver(&mut driver);
    driver.adapter().close();
    let _ = source.wait_wakes(Duration::from_millis(0));
    driver.clear_would_block();
    driver.complete_active_write();
    let later = source.wait_wakes(Duration::from_millis(0));
    assert!(
        later.adapter_routes.is_empty(),
        "closed adapter must not emit later Writable progress"
    );
    assert_eq!(
        driver.adapter().try_write(&opaque_frame("after-close")),
        Err(TerminalAdapterWriteError::Closed)
    );
}

fn assert_rejected_write_recovery<D>(driver: &mut D)
where
    D: TerminalAdapterHarnessDriver + WakingTerminalAdapter,
{
    driver.force_would_block();
    assert_eq!(
        driver.adapter().try_write(&opaque_frame("rejected")),
        Err(TerminalAdapterWriteError::WouldBlock)
    );
    driver.clear_would_block();
    assert_eq!(
        driver.adapter().try_write(&opaque_frame("recovered")),
        Ok(())
    );
    driver.complete_active_write();
}

fn assert_wake_coalesces<D>()
where
    D: TerminalAdapterHarnessDriver + Default + WakingTerminalAdapter,
{
    let mut driver = D::default();
    let source = bind_driver(&mut driver);
    driver.complete_active_write();
    driver.clear_would_block();
    driver.inject_ingress_frame(b"one".to_vec());
    driver.inject_ingress_frame(b"two".to_vec());
    let batch = source.wait_wakes(Duration::from_millis(0));
    assert!(
        batch.adapter_routes.len() <= 1,
        "repeated capacity returns must coalesce to one queued node"
    );
}

fn assert_idempotent_close<D>()
where
    D: TerminalAdapterHarnessDriver + Default + WakingTerminalAdapter,
{
    let mut driver = D::default();
    driver.adapter().close();
    driver.force_closed();
    assert_eq!(
        driver.adapter().try_write(&opaque_frame("still-closed")),
        Err(TerminalAdapterWriteError::Closed)
    );
    let _ = TerminalWakeKind::Closed;
}
