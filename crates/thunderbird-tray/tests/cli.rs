// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use thunderbird_tray::cli::{Cli, CliError, Command, LogLevel};
use thunderbird_tray::config::WindowBackend;

#[test]
fn defaults_start_the_native_host() {
    assert_eq!(
        Cli::parse_from(["thunderbird-tray"]).unwrap(),
        Cli::default()
    );
}

#[test]
fn parses_doctor_and_all_global_options() {
    let cli = Cli::parse_from([
        "thunderbird-tray",
        "--config",
        "/tmp/test.toml",
        "--window-backend",
        "kde-wayland",
        "--log-level",
        "debug",
        "doctor",
    ])
    .unwrap();

    assert_eq!(cli.command, Command::Doctor);
    assert_eq!(cli.config, Some(PathBuf::from("/tmp/test.toml")));
    assert_eq!(cli.window_backend, Some(WindowBackend::KdeWayland));
    assert_eq!(cli.log_level, LogLevel::Debug);
}

#[test]
fn help_and_version_short_options_are_supported() {
    assert_eq!(
        Cli::parse_from(["thunderbird-tray", "-h"]).unwrap().command,
        Command::Help
    );
    assert_eq!(
        Cli::parse_from(["thunderbird-tray", "-V"]).unwrap().command,
        Command::Version
    );
}

#[test]
fn invalid_cli_values_are_typed_errors() {
    assert!(matches!(
        Cli::parse_from(["thunderbird-tray", "--config"]),
        Err(CliError::MissingValue("--config"))
    ));
    assert!(matches!(
        Cli::parse_from(["thunderbird-tray", "--window-backend", "magic"]),
        Err(CliError::InvalidWindowBackend(value)) if value == "magic"
    ));
    assert!(matches!(
        Cli::parse_from(["thunderbird-tray", "--log-level", "verbose"]),
        Err(CliError::InvalidLogLevel(value)) if value == "verbose"
    ));
    assert!(matches!(
        Cli::parse_from(["thunderbird-tray", "doctor", "--help"]),
        Err(CliError::MultipleCommands)
    ));
}

#[test]
fn thunderbird_native_messaging_arguments_select_host_mode() {
    let cli = Cli::parse_from([
        "thunderbird-tray",
        "/home/test/.mozilla/native-messaging-hosts/io.github.apophuy.thunderbird_tray.json",
        "{6fd82ebe-fdb5-40f5-b272-04585e8ba661}",
    ])
    .unwrap();

    assert_eq!(cli.command, Command::Run);
    let launch = cli.native_launch.unwrap();
    assert!(launch.manifest_path.is_absolute());
    assert_eq!(
        launch.extension_id,
        "{6fd82ebe-fdb5-40f5-b272-04585e8ba661}"
    );
}

#[test]
fn internal_service_mode_is_distinct_from_native_launch() {
    let cli = Cli::parse_from(["thunderbird-tray", "--service"]).unwrap();
    assert_eq!(cli.command, Command::Service);
    assert!(cli.native_launch.is_none());
}
