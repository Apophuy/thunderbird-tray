// SPDX-License-Identifier: GPL-3.0-only

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use thunderbird_tray::config::{Config, ConfigError, ConfigSource, LanguageMode, WindowBackend};

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let sequence = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "thunderbird-tray-config-{}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn defaults_are_sane_and_stable() {
    let config = Config::default();
    assert!(!config.general.start_thunderbird);
    assert!(config.general.notifications);
    assert_eq!(config.general.language, LanguageMode::Auto);
    assert!(config.tray.show_unread_count);
    assert!(!config.tray.hide_when_zero);
    assert_eq!(config.thunderbird.command, "thunderbird");
    assert!(config.thunderbird.arguments.is_empty());
    assert_eq!(config.window.backend, WindowBackend::Auto);
}

#[test]
fn xdg_config_home_wins_and_relative_values_fall_back_to_home() {
    let source = ConfigSource::from_environment(
        None,
        Some(OsStr::new("/tmp/xdg")),
        Some(OsStr::new("/home/test")),
    )
    .unwrap();
    assert_eq!(
        source.path(),
        PathBuf::from("/tmp/xdg/thunderbird-tray/config.toml")
    );

    let source = ConfigSource::from_environment(
        None,
        Some(OsStr::new("relative")),
        Some(OsStr::new("/home/test")),
    )
    .unwrap();
    assert_eq!(
        source.path(),
        PathBuf::from("/home/test/.config/thunderbird-tray/config.toml")
    );
}

#[test]
fn missing_implicit_config_uses_defaults_but_missing_explicit_config_fails() {
    let directory = temporary_directory();
    let implicit = ConfigSource::from_environment(None, Some(directory.as_os_str()), None).unwrap();
    assert_eq!(implicit.load().unwrap(), Config::default());

    let explicit =
        ConfigSource::from_environment(Some(directory.join("missing.toml")), None, None).unwrap();
    assert!(matches!(explicit.load(), Err(ConfigError::Read { .. })));
}

#[test]
fn partial_config_uses_defaults_and_parses_supported_values() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    fs::write(
        &path,
        r#"
[general]
language = "ru"

[thunderbird]
command = "/opt/thunderbird/thunderbird"
arguments = ["--new-instance"]

[window]
backend = "kde-wayland"
"#,
    )
    .unwrap();
    let source = ConfigSource::from_environment(Some(path), None, None).unwrap();
    let config = source.load().unwrap();
    assert_eq!(config.general.language, LanguageMode::Ru);
    assert!(config.general.notifications);
    assert_eq!(config.window.backend, WindowBackend::KdeWayland);
    assert_eq!(config.thunderbird.command, "/opt/thunderbird/thunderbird");
    assert_eq!(config.thunderbird.arguments, ["--new-instance"]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unknown_keys_and_backend_names_are_rejected() {
    for contents in [
        "unexpected = true\n",
        "[window]\nbackend = \"magic\"\n",
        "[tray]\nunknown = true\n",
    ] {
        let directory = temporary_directory();
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("config.toml");
        fs::write(&path, contents).unwrap();
        let source = ConfigSource::from_environment(Some(path), None, None).unwrap();
        assert!(matches!(source.load(), Err(ConfigError::Parse { .. })));
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn language_override_is_persisted_and_loaded() {
    let directory = temporary_directory();
    let path = directory.join("config.toml");
    let source = ConfigSource::from_environment(Some(path), None, None).unwrap();
    let mut config = Config::default();
    config.set_language(LanguageMode::Ru);
    source.save(&config).unwrap();

    assert_eq!(source.load().unwrap().general.language, LanguageMode::Ru);
    fs::remove_dir_all(directory).unwrap();
}
