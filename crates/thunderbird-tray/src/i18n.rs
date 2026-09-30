// SPDX-License-Identifier: GPL-3.0-only

//! English/Russian localization boundary for user-facing application text.

use std::env;
use std::ffi::OsString;

use crate::cli::{CliError, display_os};
use crate::config::{ConfigError, LanguageMode};
use crate::core::TrayState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    English,
    Russian,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
        }
    }
}

/// Locale variables captured at the process boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocaleEnvironment {
    pub lc_all: Option<OsString>,
    pub lc_messages: Option<OsString>,
    pub language: Option<OsString>,
    pub lang: Option<OsString>,
}

impl LocaleEnvironment {
    pub fn from_process() -> Self {
        Self {
            lc_all: env::var_os("LC_ALL"),
            lc_messages: env::var_os("LC_MESSAGES"),
            language: env::var_os("LANGUAGE"),
            lang: env::var_os("LANG"),
        }
    }
}

pub fn resolve_language(mode: LanguageMode, environment: &LocaleEnvironment) -> Language {
    match mode {
        LanguageMode::En => Language::English,
        LanguageMode::Ru => Language::Russian,
        LanguageMode::Auto => {
            let selected_locale = [
                environment.lc_all.as_ref(),
                environment.language.as_ref(),
                environment.lc_messages.as_ref(),
                environment.lang.as_ref(),
            ]
            .into_iter()
            .flatten()
            .find(|value| !value.is_empty());
            selected_locale
                .and_then(|value| supported_language(&value.to_string_lossy()))
                .unwrap_or(Language::English)
        }
    }
}

fn supported_language(value: &str) -> Option<Language> {
    for locale in value.split(':') {
        let normalized = locale.trim().to_ascii_lowercase();
        let language = normalized
            .split(['_', '-', '.', '@'])
            .next()
            .unwrap_or_default();
        match language {
            "ru" => return Some(Language::Russian),
            "en" | "c" | "posix" => return Some(Language::English),
            _ => {}
        }
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Localizer {
    language: Language,
}

impl Localizer {
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    pub fn language(self) -> Language {
        self.language
    }

    pub fn help(self) -> &'static str {
        match self.language {
            Language::English => {
                "thunderbird-tray — Linux tray companion for Thunderbird\n\n\
Usage: thunderbird-tray [OPTIONS] [doctor]\n\n\
Options:\n  --config <PATH>\n  --window-backend <auto|kde-wayland|x11|none>\n  \
--log-level <trace|debug|info|warn|error>\n  -h, --help\n  -V, --version\n"
            }
            Language::Russian => {
                "thunderbird-tray — дополнение Thunderbird для системного трея Linux\n\n\
Использование: thunderbird-tray [ПАРАМЕТРЫ] [doctor]\n\n\
Параметры:\n  --config <ПУТЬ>\n  --window-backend <auto|kde-wayland|x11|none>\n  \
--log-level <trace|debug|info|warn|error>\n  -h, --help\n  -V, --version\n"
            }
        }
    }

    pub fn cli_error(self, error: &CliError) -> String {
        match (self.language, error) {
            (Language::English, CliError::MissingValue(option)) => {
                format!("missing value for {option}")
            }
            (Language::Russian, CliError::MissingValue(option)) => {
                format!("не указано значение для {option}")
            }
            (Language::English, CliError::NonUtf8Value(option)) => {
                format!("value for {option} is not valid UTF-8")
            }
            (Language::Russian, CliError::NonUtf8Value(option)) => {
                format!("значение {option} содержит недопустимые символы UTF-8")
            }
            (Language::English, CliError::InvalidWindowBackend(value)) => {
                format!("unknown window backend: {value}")
            }
            (Language::Russian, CliError::InvalidWindowBackend(value)) => {
                format!("неизвестный оконный бэкенд: {value}")
            }
            (Language::English, CliError::InvalidLogLevel(value)) => {
                format!("unknown log level: {value}")
            }
            (Language::Russian, CliError::InvalidLogLevel(value)) => {
                format!("неизвестный уровень журналирования: {value}")
            }
            (Language::English, CliError::UnknownArgument(value)) => {
                format!("unknown argument: {}", display_os(value))
            }
            (Language::Russian, CliError::UnknownArgument(value)) => {
                format!("неизвестный аргумент: {}", display_os(value))
            }
            (Language::English, CliError::MultipleCommands) => {
                "only one command may be selected".to_owned()
            }
            (Language::Russian, CliError::MultipleCommands) => {
                "можно выбрать только одну команду".to_owned()
            }
        }
    }

    pub fn config_error(self, error: &ConfigError) -> String {
        match (self.language, error) {
            (Language::English, ConfigError::ConfigHomeUnavailable) => {
                "could not determine the configuration directory".to_owned()
            }
            (Language::Russian, ConfigError::ConfigHomeUnavailable) => {
                "не удалось определить каталог конфигурации".to_owned()
            }
            (Language::English, ConfigError::Read { path, source }) => {
                format!("could not read configuration {}: {source}", path.display())
            }
            (Language::Russian, ConfigError::Read { path, source }) => {
                format!(
                    "не удалось прочитать конфигурацию {}: {source}",
                    path.display()
                )
            }
            (Language::English, ConfigError::Parse { path, source }) => {
                format!("invalid configuration {}: {source}", path.display())
            }
            (Language::Russian, ConfigError::Parse { path, source }) => {
                format!("ошибка в конфигурации {}: {source}", path.display())
            }
            (Language::English, ConfigError::EmptyThunderbirdCommand) => {
                "Thunderbird command must not be empty".to_owned()
            }
            (Language::Russian, ConfigError::EmptyThunderbirdCommand) => {
                "команда запуска Thunderbird не должна быть пустой".to_owned()
            }
            (Language::English, ConfigError::Serialize(source)) => {
                format!("could not serialize configuration: {source}")
            }
            (Language::Russian, ConfigError::Serialize(source)) => {
                format!("не удалось сохранить конфигурацию: {source}")
            }
            (Language::English, ConfigError::CreateDirectory { path, source }) => {
                format!(
                    "could not create configuration directory {}: {source}",
                    path.display()
                )
            }
            (Language::Russian, ConfigError::CreateDirectory { path, source }) => {
                format!(
                    "не удалось создать каталог конфигурации {}: {source}",
                    path.display()
                )
            }
            (Language::English, ConfigError::Write { path, source }) => {
                format!("could not write configuration {}: {source}", path.display())
            }
            (Language::Russian, ConfigError::Write { path, source }) => {
                format!(
                    "не удалось записать конфигурацию {}: {source}",
                    path.display()
                )
            }
        }
    }

    pub fn native_host_error(self, error: &impl std::fmt::Display) -> String {
        match self.language {
            Language::English => format!("native messaging host failed: {error}"),
            Language::Russian => format!("ошибка Native Messaging: {error}"),
        }
    }

    pub fn tray_labels(self, state: TrayState, show_unread_count: bool) -> TrayLabels {
        let inbox_status = match (self.language, state, show_unread_count) {
            (Language::English, TrayState::Disconnected, _) => {
                "Thunderbird connection unavailable".to_owned()
            }
            (Language::Russian, TrayState::Disconnected, _) => {
                "Нет соединения с Thunderbird".to_owned()
            }
            (Language::English, TrayState::NoUnread, _) => {
                "Inbox is up to date — no unread messages".to_owned()
            }
            (Language::Russian, TrayState::NoUnread, _) => {
                "Входящие прочитаны — новых писем нет".to_owned()
            }
            (Language::English, TrayState::Unread(count), true) => {
                format!("{} unread {} in Inbox", count, english_message_word(count))
            }
            (Language::Russian, TrayState::Unread(count), true) => {
                format!("{} {} во Входящих", count, russian_unread_phrase(count))
            }
            (Language::English, TrayState::Unread(_), false) => {
                "Unread messages in Inbox".to_owned()
            }
            (Language::Russian, TrayState::Unread(_), false) => {
                "Непрочитанные письма во Входящих".to_owned()
            }
        };

        match self.language {
            Language::English => TrayLabels {
                title: "thunderbird-tray",
                inbox_status,
                open_thunderbird: "_Open Thunderbird",
                refresh: "_Refresh Inbox status",
                language: "_Language",
                automatic: "_Automatic",
                english: "_English",
                russian: "_Русский",
                quit: "_Quit",
            },
            Language::Russian => TrayLabels {
                title: "thunderbird-tray",
                inbox_status,
                open_thunderbird: "_Открыть Thunderbird",
                refresh: "_Обновить состояние Входящих",
                language: "_Язык",
                automatic: "_Автоматически",
                english: "_English",
                russian: "_Русский",
                quit: "_Выйти",
            },
        }
    }

    pub fn doctor_title(self) -> &'static str {
        match self.language {
            Language::English => "thunderbird-tray diagnostics",
            Language::Russian => "Диагностика thunderbird-tray",
        }
    }

    pub fn doctor_labels(self) -> DoctorLabels {
        match self.language {
            Language::English => DoctorLabels {
                config: "Configuration",
                config_exists: "Configuration exists",
                backend: "Window backend",
                language_mode: "Language mode",
                resolved_language: "Resolved language",
                thunderbird_command: "Thunderbird command",
                yes: "yes",
                no: "no",
            },
            Language::Russian => DoctorLabels {
                config: "Конфигурация",
                config_exists: "Конфигурация существует",
                backend: "Оконный бэкенд",
                language_mode: "Режим языка",
                resolved_language: "Выбранный язык",
                thunderbird_command: "Команда Thunderbird",
                yes: "да",
                no: "нет",
            },
        }
    }
}

fn english_message_word(count: u32) -> &'static str {
    if count == 1 { "message" } else { "messages" }
}

fn russian_unread_phrase(count: u32) -> &'static str {
    let last_two = count % 100;
    let last = count % 10;
    if last == 1 && last_two != 11 {
        "непрочитанное письмо"
    } else if (2..=4).contains(&last) && !(12..=14).contains(&last_two) {
        "непрочитанных письма"
    } else {
        "непрочитанных писем"
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayLabels {
    pub title: &'static str,
    pub inbox_status: String,
    pub open_thunderbird: &'static str,
    pub refresh: &'static str,
    pub language: &'static str,
    pub automatic: &'static str,
    pub english: &'static str,
    pub russian: &'static str,
    pub quit: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DoctorLabels {
    pub config: &'static str,
    pub config_exists: &'static str,
    pub backend: &'static str,
    pub language_mode: &'static str,
    pub resolved_language: &'static str,
    pub thunderbird_command: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
}
