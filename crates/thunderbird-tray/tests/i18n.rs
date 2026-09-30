// SPDX-License-Identifier: GPL-3.0-only

use std::ffi::OsString;
use std::path::Path;

use thunderbird_tray::config::{Config, LanguageMode, WindowBackend};
use thunderbird_tray::doctor::DoctorReport;
use thunderbird_tray::i18n::{Language, LocaleEnvironment, Localizer, resolve_language};

fn environment(locale: &str) -> LocaleEnvironment {
    LocaleEnvironment {
        lang: Some(OsString::from(locale)),
        ..LocaleEnvironment::default()
    }
}

#[test]
fn automatic_language_uses_russian_locale_with_english_fallback() {
    assert_eq!(
        resolve_language(LanguageMode::Auto, &environment("ru_RU.UTF-8")),
        Language::Russian
    );
    assert_eq!(
        resolve_language(LanguageMode::Auto, &environment("de_DE.UTF-8")),
        Language::English
    );
    assert_eq!(
        resolve_language(LanguageMode::Auto, &LocaleEnvironment::default()),
        Language::English
    );
}

#[test]
fn explicit_language_overrides_the_environment() {
    assert_eq!(
        resolve_language(LanguageMode::En, &environment("ru_RU.UTF-8")),
        Language::English
    );
    assert_eq!(
        resolve_language(LanguageMode::Ru, &environment("en_US.UTF-8")),
        Language::Russian
    );
}

#[test]
fn locale_precedence_and_language_fallback_list_are_supported() {
    let environment = LocaleEnvironment {
        lc_all: Some(OsString::from("en_US.UTF-8")),
        language: Some(OsString::from("ru:en")),
        lang: Some(OsString::from("ru_RU.UTF-8")),
        ..LocaleEnvironment::default()
    };
    assert_eq!(
        resolve_language(LanguageMode::Auto, &environment),
        Language::English
    );

    let environment = LocaleEnvironment {
        lc_all: Some(OsString::from("de_DE.UTF-8")),
        lang: Some(OsString::from("ru_RU.UTF-8")),
        ..LocaleEnvironment::default()
    };
    assert_eq!(
        resolve_language(LanguageMode::Auto, &environment),
        Language::English
    );

    let environment = LocaleEnvironment {
        language: Some(OsString::from("de:ru:en")),
        ..LocaleEnvironment::default()
    };
    assert_eq!(
        resolve_language(LanguageMode::Auto, &environment),
        Language::Russian
    );
}

#[test]
fn help_and_doctor_are_localized_without_mail_data() {
    let mut config = Config::default();
    config.general.language = LanguageMode::Ru;
    config.window.backend = WindowBackend::None;
    config.thunderbird.command = "/opt/thunderbird/thunderbird".to_owned();
    let localizer = Localizer::new(Language::Russian);
    let report = DoctorReport::collect(
        Path::new("/tmp/config.toml"),
        &config,
        None,
        Language::Russian,
    )
    .render(localizer);

    assert!(localizer.help().contains("Использование"));
    assert!(report.contains("Диагностика thunderbird-tray"));
    assert!(report.contains("Оконный бэкенд: none"));
    assert!(report.contains("Выбранный язык: ru"));
    assert!(localizer.already_running().contains("уже запущен"));
    assert!(localizer.lifecycle_error(&"D-Bus").contains("D-Bus"));
    assert!(localizer.service_start_timeout().contains("не запустилась"));
    assert!(!report.contains("unread"));
    assert!(!report.contains("account"));
}
