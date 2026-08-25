//! Transport-neutral terminal adapter conformance harness.
//!
//! Always available. Do not hide this module behind `local-runtime` or
//! `ghostty-terminal`. Adapter laws live here, not in the PTY `conformance`
//! module.
//!
//! Assertions are deterministic. Drivers expose hooks instead of sleeps.

mod core;
mod fake;
mod unix_shaped;
mod webrtc_shaped;

pub use fake::{FakeTerminalAdapter, SharedFakeTerminalAdapter};
pub use unix_shaped::UnixShapedTerminalAdapter;
pub use webrtc_shaped::WebRtcShapedTerminalAdapter;

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    MIN_ADAPTER_INGRESS_BUFFER_FRAMES,
};
use botster_terminal_protocol::TerminalFrame;

/// Deterministic driver over a [`TerminalAdapter`] under test.
pub trait TerminalAdapterHarnessDriver {
    /// Adapter implementation driven by the harness.
    type Adapter: TerminalAdapter;

    /// Exclusive access to the adapter.
    fn adapter(&mut self) -> &mut Self::Adapter;

    /// Make the empty write slot report [`TerminalAdapterWriteError::WouldBlock`].
    fn force_would_block(&mut self);

    /// Clear a previously forced would-block condition.
    fn clear_would_block(&mut self);

    /// Finish the single active write, if any, unless the adapter is closed.
    ///
    /// After close this must be a no-op and must not deliver an abandoned frame.
    fn complete_active_write(&mut self);

    /// Simulate transport-side close. Same `Closed` effect as local `close()`.
    fn force_closed(&mut self);

    /// Complete frame byte blobs delivered after accepted writes finished.
    fn delivered_frame_bytes(&self) -> &[Vec<u8>];

    /// Inject one complete ingress frame.
    fn inject_ingress_frame(&mut self, bytes: Vec<u8>);

    /// Buffer an incomplete ingress frame that `try_read` must not return yet.
    fn inject_ingress_partial(&mut self, bytes: Vec<u8>);

    /// Complete the buffered partial ingress frame.
    fn complete_ingress_partial(&mut self);

    /// Drop one buffered complete ingress frame and mark loss.
    fn drop_buffered_ingress_frame(&mut self);
}

/// Prove adapter laws against `driver`.
///
/// `D: Default` lets the harness construct a second adapter so both local
/// `close()` and transport-side `force_closed()` can be asserted while a write
/// is active. Those paths cannot share one already-closed adapter.
pub fn assert_terminal_adapter_conformance<D>(driver: &mut D)
where
    D: TerminalAdapterHarnessDriver + Default,
{
    assert_ready(driver);

    let first = opaque_frame("first");
    let second = opaque_frame("second");
    let third = opaque_frame("third");
    let first_bytes = frame_bytes(&first);
    let second_bytes = frame_bytes(&second);
    let third_bytes = frame_bytes(&third);

    assert_bounds(driver, &first, &second, &first_bytes);
    assert_ordering(
        driver,
        &second,
        &third,
        &first_bytes,
        &second_bytes,
        &third_bytes,
    );
    assert_typed_rejection(driver);
    assert_no_adapter_retry(driver);
    assert_content_blind_write(driver);
    assert_close_during_active_write(driver, ClosePath::Local);
    assert_close_propagation(driver);

    let mut transport_driver = D::default();
    let kept = opaque_frame("kept-before-transport-close");
    let kept_bytes = frame_bytes(&kept);
    assert_eq!(
        transport_driver.adapter().try_write(&kept),
        Ok(()),
        "fresh driver must accept a frame before transport-close proof"
    );
    transport_driver.complete_active_write();
    assert_eq!(
        transport_driver.delivered_frame_bytes(),
        std::slice::from_ref(&kept_bytes),
        "completed frame must remain after later transport close"
    );
    assert_close_during_active_write(&mut transport_driver, ClosePath::Transport);
    assert_close_propagation(&mut transport_driver);

    let mut ingress = D::default();
    assert_ingress_empty(&mut ingress);
    assert_ingress_order(&mut ingress);
    assert_ingress_whole_frames(&mut ingress);
    assert_ingress_closed_local(&mut ingress);
    let mut transport_ingress = D::default();
    assert_ingress_closed_transport(&mut transport_ingress);
    let mut lost = D::default();
    assert_ingress_lost(&mut lost);
    let mut blind = D::default();
    assert_ingress_content_blind(&mut blind);
    let mut idle = D::default();
    assert_ingress_non_blocking(&mut idle);
}

#[derive(Clone, Copy)]
enum ClosePath {
    Local,
    Transport,
}

fn opaque_frame(marker: &str) -> TerminalFrame {
    let json = serde_json::json!({
        "type": "terminal_output",
        "marker": marker,
    });
    TerminalFrame::from_bytes(json.to_string().as_bytes()).expect("opaque fixture frame")
}

fn frame_bytes(frame: &TerminalFrame) -> Vec<u8> {
    frame.to_bytes().expect("fixture frame emits bytes")
}

fn assert_ready<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    assert_eq!(
        driver.adapter().pressure(),
        TerminalAdapterPressure::Ready,
        "fresh adapter must start Ready"
    );
}

fn assert_bounds<D: TerminalAdapterHarnessDriver>(
    driver: &mut D,
    first: &TerminalFrame,
    second: &TerminalFrame,
    first_bytes: &[u8],
) {
    assert_eq!(driver.adapter().try_write(first), Ok(()));
    assert_eq!(driver.adapter().pressure(), TerminalAdapterPressure::Full);
    assert_eq!(
        driver.adapter().try_write(second),
        Err(TerminalAdapterWriteError::Full)
    );
    assert!(
        driver.delivered_frame_bytes().is_empty(),
        "occupied slot must not deliver or queue a second frame"
    );
    driver.complete_active_write();
    assert_eq!(driver.delivered_frame_bytes(), &[first_bytes.to_vec()]);
    assert_eq!(driver.adapter().pressure(), TerminalAdapterPressure::Ready);
}

fn assert_ordering<D: TerminalAdapterHarnessDriver>(
    driver: &mut D,
    second: &TerminalFrame,
    third: &TerminalFrame,
    first_bytes: &[u8],
    second_bytes: &[u8],
    third_bytes: &[u8],
) {
    assert_eq!(driver.adapter().try_write(second), Ok(()));
    driver.complete_active_write();
    assert_eq!(driver.adapter().try_write(third), Ok(()));
    driver.complete_active_write();
    assert_eq!(
        driver.delivered_frame_bytes(),
        &[
            first_bytes.to_vec(),
            second_bytes.to_vec(),
            third_bytes.to_vec()
        ],
        "accepted frames must deliver in write-accept order"
    );
}

fn assert_typed_rejection<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    driver.force_would_block();
    assert_eq!(
        driver.adapter().pressure(),
        TerminalAdapterPressure::WouldBlock
    );
    assert_eq!(
        driver.adapter().try_write(&opaque_frame("blocked")),
        Err(TerminalAdapterWriteError::WouldBlock)
    );
    driver.clear_would_block();
    assert_eq!(driver.adapter().pressure(), TerminalAdapterPressure::Ready);

    let occupying = opaque_frame("occupying");
    assert_eq!(driver.adapter().try_write(&occupying), Ok(()));
    assert_eq!(
        driver.adapter().try_write(&opaque_frame("while-full")),
        Err(TerminalAdapterWriteError::Full)
    );
    driver.complete_active_write();
}

fn assert_no_adapter_retry<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    let rejected = opaque_frame("rejected-no-retry");
    let accepted = opaque_frame("accepted-after-reject");
    let accepted_bytes = frame_bytes(&accepted);
    let before = driver.delivered_frame_bytes().to_vec();

    driver.force_would_block();
    assert_eq!(
        driver.adapter().try_write(&rejected),
        Err(TerminalAdapterWriteError::WouldBlock)
    );
    driver.clear_would_block();
    assert_eq!(driver.adapter().try_write(&accepted), Ok(()));
    driver.complete_active_write();

    let mut expected = before;
    expected.push(accepted_bytes);
    assert_eq!(
        driver.delivered_frame_bytes(),
        expected.as_slice(),
        "rejected frame must not appear unless the caller writes it again"
    );
    assert!(
        !driver
            .delivered_frame_bytes()
            .iter()
            .any(|delivered| delivered == &frame_bytes(&rejected)),
        "adapter must not retry a rejected write"
    );
}

fn assert_content_blind_write<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    let frame = opaque_frame("content-blind");
    let bytes = frame_bytes(&frame);
    let mut expected = driver.delivered_frame_bytes().to_vec();
    assert_eq!(driver.adapter().try_write(&frame), Ok(()));
    driver.complete_active_write();
    expected.push(bytes);
    assert_eq!(
        driver.delivered_frame_bytes(),
        expected.as_slice(),
        "one accepted write delivers one complete frame byte blob"
    );
}

fn assert_close_during_active_write<D: TerminalAdapterHarnessDriver>(
    driver: &mut D,
    path: ClosePath,
) {
    let abandoned = opaque_frame("abandoned-in-flight");
    let later = opaque_frame("after-close");
    let before = driver.delivered_frame_bytes().to_vec();

    assert_eq!(driver.adapter().try_write(&abandoned), Ok(()));
    assert_eq!(driver.adapter().pressure(), TerminalAdapterPressure::Full);

    match path {
        ClosePath::Local => driver.adapter().close(),
        ClosePath::Transport => driver.force_closed(),
    }

    assert_eq!(
        driver.adapter().pressure(),
        TerminalAdapterPressure::Closed,
        "close during an active write must set Closed pressure"
    );
    assert_eq!(
        driver.delivered_frame_bytes(),
        before.as_slice(),
        "close during an active write must not deliver the abandoned frame"
    );
    assert_eq!(
        driver.adapter().try_write(&later),
        Err(TerminalAdapterWriteError::Closed)
    );
    driver.complete_active_write();
    assert_eq!(
        driver.delivered_frame_bytes(),
        before.as_slice(),
        "complete_active_write after close must be a no-op"
    );
}

fn assert_close_propagation<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    driver.adapter().close();
    driver.force_closed();
    assert_eq!(
        driver.adapter().pressure(),
        TerminalAdapterPressure::Closed,
        "close is idempotent"
    );
    assert_eq!(
        driver.adapter().try_write(&opaque_frame("still-closed")),
        Err(TerminalAdapterWriteError::Closed)
    );
}

fn assert_ingress_empty<D: TerminalAdapterHarnessDriver + Default>(driver: &mut D) {
    let mut fresh = D::default();
    assert_eq!(fresh.adapter().try_read(), TerminalIngress::Empty);
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Empty);
}

fn assert_ingress_order<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    driver.inject_ingress_frame(b"one".to_vec());
    driver.inject_ingress_frame(b"two".to_vec());
    driver.inject_ingress_frame(b"three".to_vec());
    assert_eq!(
        driver.adapter().try_read(),
        TerminalIngress::Frame(b"one".to_vec())
    );
    assert_eq!(
        driver.adapter().try_read(),
        TerminalIngress::Frame(b"two".to_vec())
    );
    assert_eq!(
        driver.adapter().try_read(),
        TerminalIngress::Frame(b"three".to_vec())
    );
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Empty);
}

fn assert_ingress_whole_frames<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    driver.inject_ingress_partial(b"partial".to_vec());
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Empty);
    driver.complete_ingress_partial();
    assert_eq!(
        driver.adapter().try_read(),
        TerminalIngress::Frame(b"partial".to_vec())
    );
}

fn assert_ingress_closed_local<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    driver.inject_ingress_frame(b"drop-on-close".to_vec());
    driver.adapter().close();
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Closed);
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Closed);
}

fn assert_ingress_closed_transport<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    driver.inject_ingress_frame(b"drop-on-transport-close".to_vec());
    driver.force_closed();
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Closed);
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Closed);
    assert_ne!(driver.adapter().try_read(), TerminalIngress::Empty);
}

fn assert_ingress_lost<D: TerminalAdapterHarnessDriver + Default>(_driver: &mut D) {
    let mut floor = D::default();
    for index in 0..MIN_ADAPTER_INGRESS_BUFFER_FRAMES {
        floor.inject_ingress_frame(vec![index as u8]);
    }
    assert_ne!(floor.adapter().try_read(), TerminalIngress::Lost);

    let mut loss = D::default();
    loss.inject_ingress_frame(b"keep".to_vec());
    loss.inject_ingress_frame(b"drop".to_vec());
    loss.drop_buffered_ingress_frame();
    assert_eq!(loss.adapter().try_read(), TerminalIngress::Lost);
    assert_eq!(
        loss.adapter().try_read(),
        TerminalIngress::Frame(b"keep".to_vec())
    );
}

fn assert_ingress_content_blind<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    let bytes = b"\x00\xffopaque".to_vec();
    driver.inject_ingress_frame(bytes.clone());
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Frame(bytes));
}

fn assert_ingress_non_blocking<D: TerminalAdapterHarnessDriver>(driver: &mut D) {
    assert_eq!(driver.adapter().try_read(), TerminalIngress::Empty);
}
