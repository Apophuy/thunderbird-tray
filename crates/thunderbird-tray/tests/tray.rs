// SPDX-License-Identifier: GPL-3.0-only

use thunderbird_tray::config::{LanguageMode, TrayConfig};
use thunderbird_tray::core::TrayState;
use thunderbird_tray::i18n::Language;
use thunderbird_tray::tray::{IndicatorStatus, TrayIcon, TrayModel};

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    icon: TrayIcon,
    status: IndicatorStatus,
    inbox: String,
    open: &'static str,
    refresh: &'static str,
    language: &'static str,
    automatic: &'static str,
    english: &'static str,
    russian: &'static str,
    quit: &'static str,
    can_open: bool,
    can_refresh: bool,
}

fn snapshot(state: TrayState, language_mode: LanguageMode) -> Snapshot {
    let model = TrayModel::new(
        state,
        TrayConfig::default(),
        language_mode,
        Language::English,
    );
    let presentation = model.presentation();
    Snapshot {
        icon: presentation.icon,
        status: presentation.status,
        inbox: presentation.labels.inbox_status,
        open: presentation.labels.open_thunderbird,
        refresh: presentation.labels.refresh,
        language: presentation.labels.language,
        automatic: presentation.labels.automatic,
        english: presentation.labels.english,
        russian: presentation.labels.russian,
        quit: presentation.labels.quit,
        can_open: presentation.can_open_thunderbird,
        can_refresh: presentation.can_refresh,
    }
}

fn model(state: TrayState) -> TrayModel {
    TrayModel::new(
        state,
        TrayConfig::default(),
        LanguageMode::Auto,
        Language::English,
    )
}

#[test]
fn disconnected_presentation_never_reuses_unread_data() {
    let mut model = model(TrayState::Unread(27));
    assert_eq!(model.presentation().icon, TrayIcon::Unread);
    assert!(model.presentation().labels.inbox_status.contains("27"));

    model.set_state(TrayState::Disconnected);
    let presentation = model.presentation();
    assert_eq!(presentation.icon, TrayIcon::Disconnected);
    assert_eq!(
        presentation.labels.inbox_status,
        "Thunderbird connection unavailable"
    );
    assert!(!presentation.labels.inbox_status.contains("27"));
    assert!(!presentation.can_refresh);
}

#[test]
fn connected_states_have_distinct_icons_and_refresh_behavior() {
    let mut model = model(TrayState::NoUnread);
    let zero = model.presentation();
    assert_eq!(zero.icon, TrayIcon::Connected);
    assert!(zero.can_refresh);
    assert_eq!(zero.status, IndicatorStatus::Active);

    model.set_state(TrayState::Unread(4));
    let unread = model.presentation();
    assert_eq!(unread.icon, TrayIcon::Unread);
    assert!(unread.labels.inbox_status.contains("4 unread messages"));
}

#[test]
fn hide_when_zero_maps_only_zero_state_to_passive() {
    let config = TrayConfig {
        hide_when_zero: true,
        ..TrayConfig::default()
    };
    let mut model = TrayModel::new(
        TrayState::NoUnread,
        config,
        LanguageMode::En,
        Language::Russian,
    );
    assert_eq!(model.presentation().status, IndicatorStatus::Passive);

    model.set_state(TrayState::Disconnected);
    assert_eq!(model.presentation().status, IndicatorStatus::Active);
    model.set_state(TrayState::Unread(1));
    assert_eq!(model.presentation().status, IndicatorStatus::Active);
}

#[test]
fn language_mode_changes_all_tray_text_immediately() {
    let mut model = model(TrayState::Unread(22));
    assert_eq!(model.presentation().labels.language, "_Language");

    model.set_language_mode(LanguageMode::Ru);
    let russian = model.presentation();
    assert_eq!(russian.labels.language, "_Язык");
    assert_eq!(
        russian.labels.inbox_status,
        "22 непрочитанных письма во Входящих"
    );

    model.set_language_mode(LanguageMode::En);
    assert_eq!(model.presentation().labels.language, "_Language");
}

#[test]
fn russian_unread_plural_forms_are_correct() {
    let mut model = TrayModel::new(
        TrayState::Unread(1),
        TrayConfig::default(),
        LanguageMode::Ru,
        Language::English,
    );
    assert_eq!(
        model.presentation().labels.inbox_status,
        "1 непрочитанное письмо во Входящих"
    );
    model.set_state(TrayState::Unread(3));
    assert_eq!(
        model.presentation().labels.inbox_status,
        "3 непрочитанных письма во Входящих"
    );
    model.set_state(TrayState::Unread(11));
    assert_eq!(
        model.presentation().labels.inbox_status,
        "11 непрочитанных писем во Входящих"
    );
}

#[test]
fn hidden_count_keeps_unread_meaning_without_a_number() {
    let config = TrayConfig {
        show_unread_count: false,
        ..TrayConfig::default()
    };
    let model = TrayModel::new(
        TrayState::Unread(99),
        config,
        LanguageMode::En,
        Language::English,
    );
    assert_eq!(
        model.presentation().labels.inbox_status,
        "Unread messages in Inbox"
    );
}

#[test]
fn english_snapshots_cover_every_state_and_menu_action() {
    assert_eq!(
        snapshot(TrayState::Disconnected, LanguageMode::En),
        Snapshot {
            icon: TrayIcon::Disconnected,
            status: IndicatorStatus::Active,
            inbox: "Thunderbird connection unavailable".into(),
            open: "_Open Thunderbird",
            refresh: "_Refresh Inbox status",
            language: "_Language",
            automatic: "_Automatic",
            english: "_English",
            russian: "_Русский",
            quit: "_Quit",
            can_open: false,
            can_refresh: false,
        }
    );
    assert_eq!(
        snapshot(TrayState::NoUnread, LanguageMode::En),
        Snapshot {
            icon: TrayIcon::Connected,
            status: IndicatorStatus::Active,
            inbox: "Inbox is up to date — no unread messages".into(),
            open: "_Open Thunderbird",
            refresh: "_Refresh Inbox status",
            language: "_Language",
            automatic: "_Automatic",
            english: "_English",
            russian: "_Русский",
            quit: "_Quit",
            can_open: false,
            can_refresh: true,
        }
    );
    assert_eq!(
        snapshot(TrayState::Unread(2), LanguageMode::En),
        Snapshot {
            icon: TrayIcon::Unread,
            status: IndicatorStatus::Active,
            inbox: "2 unread messages in Inbox".into(),
            open: "_Open Thunderbird",
            refresh: "_Refresh Inbox status",
            language: "_Language",
            automatic: "_Automatic",
            english: "_English",
            russian: "_Русский",
            quit: "_Quit",
            can_open: false,
            can_refresh: true,
        }
    );
}

#[test]
fn russian_snapshots_cover_every_state_and_menu_action() {
    assert_eq!(
        snapshot(TrayState::Disconnected, LanguageMode::Ru),
        Snapshot {
            icon: TrayIcon::Disconnected,
            status: IndicatorStatus::Active,
            inbox: "Нет соединения с Thunderbird".into(),
            open: "_Открыть Thunderbird",
            refresh: "_Обновить состояние Входящих",
            language: "_Язык",
            automatic: "_Автоматически",
            english: "_English",
            russian: "_Русский",
            quit: "_Выйти",
            can_open: false,
            can_refresh: false,
        }
    );
    assert_eq!(
        snapshot(TrayState::NoUnread, LanguageMode::Ru),
        Snapshot {
            icon: TrayIcon::Connected,
            status: IndicatorStatus::Active,
            inbox: "Входящие прочитаны — новых писем нет".into(),
            open: "_Открыть Thunderbird",
            refresh: "_Обновить состояние Входящих",
            language: "_Язык",
            automatic: "_Автоматически",
            english: "_English",
            russian: "_Русский",
            quit: "_Выйти",
            can_open: false,
            can_refresh: true,
        }
    );
    assert_eq!(
        snapshot(TrayState::Unread(5), LanguageMode::Ru),
        Snapshot {
            icon: TrayIcon::Unread,
            status: IndicatorStatus::Active,
            inbox: "5 непрочитанных писем во Входящих".into(),
            open: "_Открыть Thunderbird",
            refresh: "_Обновить состояние Входящих",
            language: "_Язык",
            automatic: "_Автоматически",
            english: "_English",
            russian: "_Русский",
            quit: "_Выйти",
            can_open: false,
            can_refresh: true,
        }
    );
}
