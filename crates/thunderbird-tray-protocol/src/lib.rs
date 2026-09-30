// SPDX-License-Identifier: GPL-3.0-only

//! Versioned messages exchanged with the Thunderbird extension.
//!
//! The protocol is independent of Native Messaging length framing. Callers can
//! treat unknown message types as a recoverable condition while unsupported
//! protocol versions and malformed known messages remain explicit errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The only protocol version supported by this release.
pub const PROTOCOL_VERSION: u32 = 1;

const HELLO_TYPE: &str = "hello";
const HELLO_ACK_TYPE: &str = "helloAck";
const FULL_STATE_TYPE: &str = "fullState";
const REQUEST_FULL_STATE_TYPE: &str = "requestFullState";

/// A protocol message with a payload determined by its wire `type`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum Message {
    /// Starts a new extension-to-host protocol session.
    #[serde(rename = "hello")]
    Hello(HelloPayload),
    /// Confirms that the host accepted a hello message.
    #[serde(rename = "helloAck")]
    HelloAck(HelloAckPayload),
    /// Replaces the host's complete Thunderbird unread snapshot.
    #[serde(rename = "fullState")]
    FullState(FullStatePayload),
    /// Asks the extension to resend its complete unread snapshot.
    #[serde(rename = "requestFullState")]
    RequestFullState(RequestFullStatePayload),
}

/// Extension and Thunderbird versions supplied during the handshake.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloPayload {
    /// Version from the extension manifest.
    pub extension_version: String,
    /// Version reported by Thunderbird.
    pub thunderbird_version: String,
}

/// Host version returned during the handshake.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAckPayload {
    /// Version of the native host application.
    pub host_version: String,
}

/// A complete unread-count snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullStatePayload {
    /// Aggregate unread count across the included accounts.
    pub total_unread: u32,
    /// Per-account unread counts used to explain the aggregate.
    pub accounts: Vec<AccountState>,
}

/// Unread state for one Thunderbird account.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    /// Thunderbird's opaque account identifier.
    pub id: String,
    /// User-visible account name, treated as untrusted display text.
    pub name: String,
    /// Aggregate unread count for this account.
    pub unread: u32,
}

/// Empty payload for a full-state refresh request.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct RequestFullStatePayload {}

/// Successful result of decoding a protocol envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeOutcome {
    /// A protocol-v1 message understood by this release.
    Message(Message),
    /// A future message type that the caller may warn about and ignore.
    UnknownMessageType {
        /// Version read from the envelope.
        protocol: u32,
        /// Unknown value of the wire `type` field.
        message_type: String,
    },
}

/// Failure to decode a protocol envelope.
#[derive(Debug, Error)]
pub enum DecodeError {
    /// The input is not a structurally valid known protocol message.
    #[error("invalid protocol JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// The peer uses a protocol version this release cannot interpret.
    #[error("unsupported protocol version {received}; this host supports version {supported}")]
    UnsupportedProtocolVersion {
        /// Version received from the peer.
        received: u32,
        /// Version supported by this release.
        supported: u32,
    },
}

#[derive(Deserialize)]
struct EnvelopeHeader {
    protocol: u32,
    #[serde(rename = "type")]
    message_type: String,
}

#[derive(Deserialize)]
struct IncomingEnvelope {
    protocol: u32,
    #[serde(flatten)]
    message: Message,
}

#[derive(Serialize)]
struct OutgoingEnvelope<'message> {
    protocol: u32,
    #[serde(flatten)]
    message: &'message Message,
}

/// Serializes a known message into a protocol-v1 UTF-8 JSON envelope.
pub fn encode(message: &Message) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&OutgoingEnvelope {
        protocol: PROTOCOL_VERSION,
        message,
    })
}

/// Decodes one UTF-8 JSON protocol envelope.
///
/// Unknown message types are returned as [`DecodeOutcome::UnknownMessageType`]
/// so application code can warn and continue without retaining an unknown
/// payload that might contain private mail data.
pub fn decode(input: &[u8]) -> Result<DecodeOutcome, DecodeError> {
    let header: EnvelopeHeader = serde_json::from_slice(input)?;

    if header.protocol != PROTOCOL_VERSION {
        return Err(DecodeError::UnsupportedProtocolVersion {
            received: header.protocol,
            supported: PROTOCOL_VERSION,
        });
    }

    if !matches!(
        header.message_type.as_str(),
        HELLO_TYPE | HELLO_ACK_TYPE | FULL_STATE_TYPE | REQUEST_FULL_STATE_TYPE
    ) {
        return Ok(DecodeOutcome::UnknownMessageType {
            protocol: header.protocol,
            message_type: header.message_type,
        });
    }

    let envelope: IncomingEnvelope = serde_json::from_slice(input)?;
    debug_assert_eq!(envelope.protocol, PROTOCOL_VERSION);
    Ok(DecodeOutcome::Message(envelope.message))
}
