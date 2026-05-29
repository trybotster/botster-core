//! Reusable Botster runtime contracts and transport-neutral primitives.
//!
//! `botster-core` is the shared substrate for Botster hosts and clients. It
//! defines stable data shapes and low-level contracts, while `botster-hub`
//! owns Botster policy and orchestration.

pub mod boundary;
pub mod capability;
pub mod client;
pub mod crypto;
pub mod entity;
pub mod extension;
pub mod package;
pub mod session;
pub mod transport;
pub mod ui;

pub use boundary::{Layer, LayerResponsibility};
pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use client::{ClientId, ClientScope, ClientState};
pub use crypto::{CryptoOperation, IdentityOperation};
pub use entity::{EntityFrame, EntityId, EntityKind};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use package::{PackageManifest, PackageSource};
pub use session::{RequestId, SessionId, SubscriptionId};
pub use transport::{TransportEgress, TransportIngress};
pub use ui::{
    UiAction, UiActionId, UiActionPending, UiActionRequestId, UiActionResult, UiActionStatus,
    UiBind, UiBindIf, UiBindList, UiChild, UiColorToken, UiCondition, UiConditional, UiHeightClass,
    UiNode, UiNodeId, UiNodeKind, UiOrientation, UiPointer, UiResponsiveHeight, UiResponsiveValue,
    UiResponsiveWidth, UiSpaceToken, UiValidationError, UiViewport, UiWidthClass,
};
