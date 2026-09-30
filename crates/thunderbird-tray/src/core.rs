// SPDX-License-Identifier: GPL-3.0-only

//! Desktop- and transport-independent application state.

use std::collections::HashSet;

use thiserror::Error;

/// Lifecycle of the Thunderbird extension connection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConnectionState {
    /// No extension is connected.
    #[default]
    Disconnected,
    /// A connection attempt is in progress.
    Connecting,
    /// The protocol handshake completed.
    Connected,
}

/// State consumed by the future tray boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TrayState {
    /// Unread data is unavailable and must not be presented as current.
    #[default]
    Disconnected,
    /// Thunderbird is connected and the current Inbox unread count is zero.
    NoUnread,
    /// Thunderbird is connected and has unread Inbox messages.
    Unread(u32),
}

/// Unread state for one Thunderbird account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountState {
    pub id: String,
    pub name: String,
    pub unread: u32,
}

/// Current complete Thunderbird snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThunderbirdState {
    total_unread: u32,
    accounts: Vec<AccountState>,
}

impl ThunderbirdState {
    pub fn total_unread(&self) -> u32 {
        self.total_unread
    }

    pub fn accounts(&self) -> &[AccountState] {
        &self.accounts
    }
}

/// Complete application state updated only through [`AppEvent`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppState {
    connection: ConnectionState,
    thunderbird: ThunderbirdState,
    tray: TrayState,
}

impl AppState {
    pub fn connection(&self) -> ConnectionState {
        self.connection
    }

    pub fn thunderbird(&self) -> &ThunderbirdState {
        &self.thunderbird
    }

    pub fn tray(&self) -> TrayState {
        self.tray
    }

    pub fn apply(&mut self, event: AppEvent) -> Result<(), StateError> {
        match event {
            AppEvent::ConnectionStarted => {
                self.clear_unread();
                self.connection = ConnectionState::Connecting;
            }
            AppEvent::Connected => {
                self.clear_unread();
                self.connection = ConnectionState::Connected;
            }
            AppEvent::FullState {
                total_unread,
                accounts,
            } => self.apply_full_state(total_unread, accounts)?,
            AppEvent::Disconnected => {
                self.clear_unread();
                self.connection = ConnectionState::Disconnected;
            }
        }
        Ok(())
    }

    fn apply_full_state(
        &mut self,
        total_unread: u32,
        accounts: Vec<AccountState>,
    ) -> Result<(), StateError> {
        if self.connection != ConnectionState::Connected {
            return Err(StateError::SnapshotWhileDisconnected);
        }

        let mut account_ids = HashSet::with_capacity(accounts.len());
        let mut calculated = 0_u64;
        for account in &accounts {
            if !account_ids.insert(account.id.as_str()) {
                return Err(StateError::DuplicateAccountId(account.id.clone()));
            }
            calculated += u64::from(account.unread);
        }
        if calculated != u64::from(total_unread) {
            return Err(StateError::InconsistentUnreadTotal {
                declared: total_unread,
                calculated,
            });
        }

        self.thunderbird = ThunderbirdState {
            total_unread,
            accounts,
        };
        self.tray = if total_unread == 0 {
            TrayState::NoUnread
        } else {
            TrayState::Unread(total_unread)
        };
        Ok(())
    }

    fn clear_unread(&mut self) {
        self.thunderbird = ThunderbirdState::default();
        self.tray = TrayState::Disconnected;
    }
}

/// Explicit input to the application state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppEvent {
    ConnectionStarted,
    Connected,
    FullState {
        total_unread: u32,
        accounts: Vec<AccountState>,
    },
    Disconnected,
}

/// Rejected state transition or invalid complete snapshot.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StateError {
    #[error("received unread state while Thunderbird is disconnected")]
    SnapshotWhileDisconnected,
    #[error("duplicate Thunderbird account ID: {0}")]
    DuplicateAccountId(String),
    #[error(
        "declared unread total {declared} does not match calculated account total {calculated}"
    )]
    InconsistentUnreadTotal { declared: u32, calculated: u64 },
}
