// SPDX-License-Identifier: GPL-3.0-only

use thunderbird_tray::core::{
    AccountState, AppEvent, AppState, ConnectionState, StateError, TrayState,
};

fn account(id: &str, unread: u32) -> AccountState {
    AccountState {
        id: id.to_owned(),
        name: format!("Account {id}"),
        unread,
    }
}

#[test]
fn connection_and_snapshot_events_drive_derived_tray_state() {
    let mut state = AppState::default();
    assert_eq!(state.connection(), ConnectionState::Disconnected);
    assert_eq!(state.tray(), TrayState::Disconnected);

    state.apply(AppEvent::ConnectionStarted).unwrap();
    assert_eq!(state.connection(), ConnectionState::Connecting);
    assert_eq!(state.tray(), TrayState::Disconnected);

    state.apply(AppEvent::Connected).unwrap();
    state
        .apply(AppEvent::FullState {
            total_unread: 5,
            accounts: vec![account("a", 2), account("b", 3)],
        })
        .unwrap();
    assert_eq!(state.connection(), ConnectionState::Connected);
    assert_eq!(state.tray(), TrayState::Unread(5));
    assert_eq!(state.thunderbird().total_unread(), 5);

    state.apply(AppEvent::Disconnected).unwrap();
    assert_eq!(state.connection(), ConnectionState::Disconnected);
    assert_eq!(state.tray(), TrayState::Disconnected);
    assert_eq!(state.thunderbird().total_unread(), 0);
    assert!(state.thunderbird().accounts().is_empty());
}

#[test]
fn zero_snapshot_is_distinct_from_disconnected_state() {
    let mut state = AppState::default();
    state.apply(AppEvent::Connected).unwrap();
    state
        .apply(AppEvent::FullState {
            total_unread: 0,
            accounts: vec![account("a", 0)],
        })
        .unwrap();
    assert_eq!(state.tray(), TrayState::NoUnread);
}

#[test]
fn invalid_snapshots_are_rejected_without_replacing_current_state() {
    let mut state = AppState::default();
    assert_eq!(
        state
            .apply(AppEvent::FullState {
                total_unread: 0,
                accounts: Vec::new(),
            })
            .unwrap_err(),
        StateError::SnapshotWhileDisconnected
    );

    state.apply(AppEvent::Connected).unwrap();
    state
        .apply(AppEvent::FullState {
            total_unread: 1,
            accounts: vec![account("a", 1)],
        })
        .unwrap();
    let previous = state.clone();

    assert_eq!(
        state
            .apply(AppEvent::FullState {
                total_unread: 2,
                accounts: vec![account("a", 1)],
            })
            .unwrap_err(),
        StateError::InconsistentUnreadTotal {
            declared: 2,
            calculated: 1,
        }
    );
    assert_eq!(state, previous);

    assert_eq!(
        state
            .apply(AppEvent::FullState {
                total_unread: 2,
                accounts: vec![account("a", 1), account("a", 1)],
            })
            .unwrap_err(),
        StateError::DuplicateAccountId("a".to_owned())
    );
    assert_eq!(state, previous);
}
