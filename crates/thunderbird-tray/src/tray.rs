// SPDX-License-Identifier: GPL-3.0-only

//! Desktop-independent tray presentation and typed user actions.

use crate::config::{LanguageMode, TrayConfig};
use crate::core::TrayState;
use crate::i18n::{Language, Localizer, TrayLabels, language_for_mode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayIcon {
    Connected,
    Unread,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndicatorStatus {
    Active,
    Passive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayPresentation {
    pub icon: TrayIcon,
    pub unread_count: Option<u32>,
    pub status: IndicatorStatus,
    pub labels: TrayLabels,
    pub can_open_thunderbird: bool,
    pub can_hide_thunderbird: bool,
    pub can_refresh: bool,
    pub language_mode: LanguageMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    OpenThunderbird,
    ToggleThunderbird,
    HideThunderbird,
    OpenSettings,
    Refresh,
    SetLanguage(LanguageMode),
    Quit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayModel {
    state: TrayState,
    config: TrayConfig,
    language_mode: LanguageMode,
    automatic_language: Language,
    can_hide_thunderbird: bool,
}

impl TrayModel {
    pub fn new(
        state: TrayState,
        config: TrayConfig,
        language_mode: LanguageMode,
        automatic_language: Language,
    ) -> Self {
        Self {
            state,
            config,
            language_mode,
            automatic_language,
            can_hide_thunderbird: false,
        }
    }

    pub fn state(&self) -> TrayState {
        self.state
    }

    pub fn set_state(&mut self, state: TrayState) {
        self.state = state;
    }

    pub fn language_mode(&self) -> LanguageMode {
        self.language_mode
    }

    pub fn set_language_mode(&mut self, language_mode: LanguageMode) {
        self.language_mode = language_mode;
    }

    pub fn set_can_hide_thunderbird(&mut self, can_hide: bool) {
        self.can_hide_thunderbird = can_hide;
    }

    pub fn set_configuration(&mut self, config: TrayConfig, language_mode: LanguageMode) {
        self.config = config;
        self.language_mode = language_mode;
    }

    pub fn presentation(&self) -> TrayPresentation {
        let language = language_for_mode(self.language_mode, self.automatic_language);
        let icon = match self.state {
            TrayState::Disconnected => TrayIcon::Disconnected,
            TrayState::NoUnread => TrayIcon::Connected,
            TrayState::Unread(_) => TrayIcon::Unread,
        };
        let status = if self.state == TrayState::NoUnread && self.config.hide_when_zero {
            IndicatorStatus::Passive
        } else {
            IndicatorStatus::Active
        };

        TrayPresentation {
            icon,
            unread_count: match (self.state, self.config.show_unread_count) {
                (TrayState::Unread(count), true) => Some(count),
                _ => None,
            },
            status,
            labels: Localizer::new(language).tray_labels(self.state, self.config.show_unread_count),
            can_open_thunderbird: true,
            can_hide_thunderbird: self.can_hide_thunderbird,
            can_refresh: self.state != TrayState::Disconnected,
            language_mode: self.language_mode,
        }
    }
}
