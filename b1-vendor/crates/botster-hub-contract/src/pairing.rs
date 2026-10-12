//! Pairing and binding handshake frames (HC 9.3, TB-L2, PR-4 to PR-7). They travel before the envelope, so they are not `ServerFrame`s.
//!
//! The tag layout (`"frame": "<name>"`, as `route_connect` in TB-L2) is pending steward ruling H2.

use crate::wire::opt_no_null;
use data_encoding::BASE32_NOPAD;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The first and only handshake frame of a route connection on `local-stream`. It uses the control framing (TR-1).
///
/// Clause: HC TB-L2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteConnect {
    pub route_id: String,
    pub route_token: String,
}

/// The Hub's challenge, sent on the control channel before any envelope frame. Both fingerprints are as the Hub sees them.
///
/// Clause: HC PR-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthChallenge {
    pub protocol: u16,
    pub protocols: Vec<u16>,
    pub route: String,
    pub hub_public_key: String,
    pub challenge: String,
    pub client_fingerprint: String,
    pub hub_fingerprint: String,
}

/// The first pairing: the proof is an HMAC of the pairing transcript (PR-7b).
///
/// Clause: HC PR-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PairRequest {
    pub protocol: u16,
    pub challenge: String,
    pub client_public_key: String,
    /// Unix seconds; it equals the bundle's `expires_at`.
    pub expires_at: u64,
    pub proof: String,
}

/// A later connection: the signature is Ed25519 over the auth transcript (PR-7b).
///
/// Clause: HC PR-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthResponse {
    pub protocol: u16,
    pub challenge: String,
    pub client_public_key: String,
    pub signature: String,
}

/// The Hub's acceptance, signed over the accepted transcript (PR-7b).
///
/// Clause: HC PR-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthAccepted {
    pub protocol: u16,
    pub challenge: String,
    pub hub_public_key: String,
    pub client_public_key: String,
    pub credential_id: String,
    pub paired: bool,
    pub signature: String,
}

/// The frames of the authentication phase and of the route handshake.
///
/// Clause: HC TB-L2, PR-5, PR-6, PR-7.
// pending steward ruling H2: the `frame` tag of the pairing frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum AuthFrame {
    RouteConnect(RouteConnect),
    AuthChallenge(AuthChallenge),
    PairRequest(PairRequest),
    AuthResponse(AuthResponse),
    AuthAccepted(AuthAccepted),
}

/// The signalling messages. Signalling is untrusted and carries no secret (PR-4).
///
/// Clause: HC PR-4.
// pending steward ruling H2: the `frame` tag of the signalling messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum Signal {
    SignalOffer {
        hub_id: String,
        connection_id: String,
        sdp: String,
    },
    SignalAnswer {
        connection_id: String,
        sdp: String,
    },
    SignalRefused {
        connection_id: String,
        reason: String,
    },
    SignalCandidate {
        connection_id: String,
        candidate: String,
    },
}

/// The pairing bundle version that this contract defines (PR-2). A client refuses an unknown version.
///
/// Clause: HC PR-2, PR-6.
pub const BUNDLE_VERSION: u8 = 1;

/// The pairing bundle, carried as the URL fragment of the pairing link:
/// `base32(version:u8 ‖ expires_at:u64be ‖ route_len:u16be ‖ route ‖ hub_public_key[32] ‖ pairing_secret[32])` without padding.
/// `route_len` is the byte length of `route`. The alphabet is RFC 4648 section 6 (`A`–`Z`, `2`–`7`), upper case, with no `=`
/// (ruling R-39). `decode` accepts only that alphabet: lower case, `=` or any other character is a malformed bundle.
/// The codec is `data_encoding::BASE32_NOPAD`, which has these properties.
///
/// Clause: HC PR-2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingBundle {
    pub version: u8,
    /// Unix seconds (PR-7b).
    pub expires_at: u64,
    /// `/hubs/<hub_id>`.
    pub route: String,
    pub hub_public_key: [u8; 32],
    pub pairing_secret: [u8; 32],
}

/// Why a pairing bundle does not decode.
///
/// Clause: HC PR-2, PR-6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    /// A character outside the upper-case base32 alphabet (lower case and `=` included, R-39), a length that base32 does not
    /// write, or nonzero trailing bits.
    Base32,
    /// A version other than [`BUNDLE_VERSION`]: the client asks the user to update it (PR-6).
    UnknownVersion(u8),
    /// The bytes are shorter or longer than the layout, or the route is not UTF-8.
    Layout,
    /// A route longer than `u16::MAX` bytes cannot be encoded.
    RouteTooLong,
}

fn take(rest: &[u8], n: usize) -> Option<(&[u8], &[u8])> {
    (rest.len() >= n).then(|| rest.split_at(n))
}

impl PairingBundle {
    /// The URL fragment text, without `#`.
    pub fn encode(&self) -> Result<String, BundleError> {
        let route_len = u16::try_from(self.route.len()).map_err(|_| BundleError::RouteTooLong)?;
        let mut bytes = Vec::with_capacity(1 + 8 + 2 + self.route.len() + 64);
        bytes.push(self.version);
        bytes.extend_from_slice(&self.expires_at.to_be_bytes());
        bytes.extend_from_slice(&route_len.to_be_bytes());
        bytes.extend_from_slice(self.route.as_bytes());
        bytes.extend_from_slice(&self.hub_public_key);
        bytes.extend_from_slice(&self.pairing_secret);
        Ok(BASE32_NOPAD.encode(&bytes))
    }

    /// Decodes the URL fragment text (without `#`). An unknown version is refused before the rest is read.
    pub fn decode(text: &str) -> Result<Self, BundleError> {
        let bytes = BASE32_NOPAD
            .decode(text.as_bytes())
            .map_err(|_| BundleError::Base32)?;
        let (&version, rest) = bytes.split_first().ok_or(BundleError::Layout)?;
        if version != BUNDLE_VERSION {
            return Err(BundleError::UnknownVersion(version));
        }
        let (expires_at, rest) = take(rest, 8).ok_or(BundleError::Layout)?;
        let (route_len, rest) = take(rest, 2).ok_or(BundleError::Layout)?;
        let route_len = usize::from(u16::from_be_bytes([route_len[0], route_len[1]]));
        let (route, rest) = take(rest, route_len).ok_or(BundleError::Layout)?;
        let (hub_public_key, pairing_secret) = take(rest, 32).ok_or(BundleError::Layout)?;
        if pairing_secret.len() != 32 {
            return Err(BundleError::Layout);
        }
        Ok(PairingBundle {
            version,
            expires_at: u64::from_be_bytes(expires_at.try_into().map_err(|_| BundleError::Layout)?),
            route: String::from_utf8(route.to_vec()).map_err(|_| BundleError::Layout)?,
            hub_public_key: hub_public_key.try_into().map_err(|_| BundleError::Layout)?,
            pairing_secret: pairing_secret.try_into().map_err(|_| BundleError::Layout)?,
        })
    }
}

/// The first line of `pairing_transcript`.
///
/// Clause: HC PR-7b.
pub const PAIRING_TRANSCRIPT_TAG: &str = "botster-pairing-v1";
/// The first line of `auth_transcript`.
///
/// Clause: HC PR-7b.
pub const AUTH_TRANSCRIPT_TAG: &str = "botster-auth-v1";
/// The first line of `accepted_transcript`.
///
/// Clause: HC PR-7b.
pub const ACCEPTED_TRANSCRIPT_TAG: &str = "botster-auth-accepted-v1";

/// The mode line of `accepted_transcript`: `pair` after a first pairing, `auth` after a later connection.
///
/// Clause: HC PR-7b.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptedMode {
    Pair,
    Auth,
}

impl AcceptedMode {
    /// The word on the line.
    pub fn as_str(self) -> &'static str {
        match self {
            AcceptedMode::Pair => "pair",
            AcceptedMode::Auth => "auth",
        }
    }
}

/// `pairing_transcript`, the input of the PR-6 proof: each field on its own line, joined with `\n`. Keys are the base64url text of
/// the frames, `expires_at` is decimal Unix seconds, and the fingerprints are the trimmed, lowercased SDP values.
///
/// Clause: HC PR-7b, PR-6.
pub fn pairing_transcript(
    route: &str,
    hub_public_key: &str,
    challenge: &str,
    client_public_key: &str,
    expires_at: u64,
    client_fingerprint: &str,
    hub_fingerprint: &str,
) -> String {
    let expires_at = expires_at.to_string();
    [
        PAIRING_TRANSCRIPT_TAG,
        route,
        hub_public_key,
        challenge,
        client_public_key,
        &expires_at,
        client_fingerprint,
        hub_fingerprint,
    ]
    .join("\n")
}

/// `auth_transcript`, the input of the PR-6 Ed25519 signature of a later connection.
///
/// Clause: HC PR-7b, PR-6.
pub fn auth_transcript(
    route: &str,
    hub_public_key: &str,
    challenge: &str,
    client_public_key: &str,
    client_fingerprint: &str,
    hub_fingerprint: &str,
) -> String {
    [
        AUTH_TRANSCRIPT_TAG,
        route,
        hub_public_key,
        challenge,
        client_public_key,
        client_fingerprint,
        hub_fingerprint,
    ]
    .join("\n")
}

/// `accepted_transcript`, the input of the Hub's PR-7 signature.
///
/// Clause: HC PR-7b, PR-7.
pub fn accepted_transcript(
    route: &str,
    challenge: &str,
    client_public_key: &str,
    credential_id: &str,
    mode: AcceptedMode,
    client_fingerprint: &str,
    hub_fingerprint: &str,
) -> String {
    [
        ACCEPTED_TRANSCRIPT_TAG,
        route,
        challenge,
        client_public_key,
        credential_id,
        mode.as_str(),
        client_fingerprint,
        hub_fingerprint,
    ]
    .join("\n")
}

/// `GET <web origin>/hubs/<hub_id>/ice`: the ICE servers.
///
/// Clause: HC PR-4, A2 G-30.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IceServers {
    pub servers: Vec<IceServer>,
}

/// One ICE server.
///
/// Clause: HC A2 G-30.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub username: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub credential: Option<String>,
}
