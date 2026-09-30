// SPDX-License-Identifier: GPL-3.0-only

//! Desktop-independent tray presentation and typed user actions.

use crate::config::{LanguageMode, TrayConfig};
use crate::core::TrayState;
use crate::i18n::{Language, Localizer, TrayLabels};

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
    pub status: IndicatorStatus,
    pub labels: TrayLabels,
    pub can_open_thunderbird: bool,
    pub can_refresh: bool,
    pub language_mode: LanguageMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    OpenThunderbird,
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

    pub fn presentation(&self) -> TrayPresentation {
        let language = match self.language_mode {
            LanguageMode::Auto => self.automatic_language,
            LanguageMode::En => Language::English,
            LanguageMode::Ru => Language::Russian,
        };
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
            status,
            labels: Localizer::new(language).tray_labels(self.state, self.config.show_unread_count),
            // The launch/window backend is introduced in Stage 6.
            can_open_thunderbird: false,
            can_refresh: self.state != TrayState::Disconnected,
            language_mode: self.language_mode,
        }
    }
}
