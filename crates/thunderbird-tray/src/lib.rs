// SPDX-License-Identifier: GPL-3.0-only

//! Native host application boundary.

pub mod cli;
pub mod config;
pub mod core;
pub mod doctor;
pub mod i18n;

use std::io::{Read, Write};

use thiserror::Error;
use thunderbird_tray_native_messaging::{FramingError, read_frame, write_frame};
use thunderbird_tray_protocol::{
    DecodeError, DecodeOutcome, HelloAckPayload, Message, decode, encode,
};
use tracing::{info, warn};

use crate::core::{AccountState, AppEvent, AppState, ConnectionState, StateError};

/// Failure while serving one Thunderbird Native Messaging connection.
#[derive(Debug, Error)]
pub enum HostError {
    /// Native Messaging framing or stream I/O failed.
    #[error(transparent)]
    Framing(#[from] FramingError),
    /// A framed payload was not a supported protocol message.
    #[error(transparent)]
    Protocol(#[from] DecodeError),
    /// A host response could not be serialized.
    #[error("could not serialize native host response: {0}")]
    Encode(#[from] serde_json::Error),
    /// An incoming snapshot was inconsistent with core application state.
    #[error(transparent)]
    State(#[from] StateError),
}

/// Serves framed protocol messages until the peer closes its input stream.
///
/// `output` receives framed JSON only. All diagnostics are emitted through
/// `tracing`, whose binary-level subscriber is explicitly configured for
/// stderr.
pub fn run_host(input: &mut impl Read, output: &mut impl Write) -> Result<(), HostError> {
    let mut state = AppState::default();
    state.apply(AppEvent::ConnectionStarted)?;

    while let Some(frame) = read_frame(input)? {
        match decode(&frame)? {
            DecodeOutcome::UnknownMessageType { message_type, .. } => {
                warn!(message_type, "ignoring unknown protocol message type");
            }
            DecodeOutcome::Message(Message::Hello(_)) => {
                state.apply(AppEvent::Connected)?;
                write_message(
                    output,
                    &Message::HelloAck(HelloAckPayload {
                        host_version: env!("CARGO_PKG_VERSION").to_owned(),
                    }),
                )?;
                info!("native messaging handshake completed");
            }
            DecodeOutcome::Message(Message::FullState(snapshot))
                if state.connection() == ConnectionState::Connected =>
            {
                state.apply(AppEvent::FullState {
                    total_unread: snapshot.total_unread,
                    accounts: snapshot
                        .accounts
                        .into_iter()
                        .map(|account| AccountState {
                            id: account.id,
                            name: account.name,
                            unread: account.unread,
                        })
                        .collect(),
                })?;
                info!(
                    total_unread = state.thunderbird().total_unread(),
                    account_count = state.thunderbird().accounts().len(),
                    "received Thunderbird Inbox unread state"
                );
            }
            DecodeOutcome::Message(Message::FullState(_)) => {
                warn!("ignoring fullState received before hello");
            }
            DecodeOutcome::Message(Message::HelloAck(_)) => {
                warn!("ignoring host-only helloAck received from extension");
            }
            DecodeOutcome::Message(Message::RequestFullState(_)) => {
                warn!("ignoring host-only requestFullState received from extension");
            }
        }
    }

    state.apply(AppEvent::Disconnected)?;

    Ok(())
}

fn write_message(output: &mut impl Write, message: &Message) -> Result<(), HostError> {
    let payload = encode(message)?;
    write_frame(output, &payload)?;
    Ok(())
}
