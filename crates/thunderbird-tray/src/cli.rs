// SPDX-License-Identifier: GPL-3.0-only

//! Small, dependency-free command-line parser for the stable MVP surface.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::Level;

use crate::config::WindowBackend;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Command {
    #[default]
    Run,
    Doctor,
    Settings,
    Help,
    Version,
    InstallNativeManifest,
    UninstallNativeManifest,
    /// Internal long-lived process started by a Native Messaging launcher.
    Service,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    pub fn tracing_level(self) -> Level {
        match self {
            Self::Trace => Level::TRACE,
            Self::Debug => Level::DEBUG,
            Self::Info => Level::INFO,
            Self::Warn => Level::WARN,
            Self::Error => Level::ERROR,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cli {
    pub command: Command,
    pub config: Option<PathBuf>,
    pub window_backend: Option<WindowBackend>,
    pub log_level: LogLevel,
    pub native_launch: Option<NativeMessagingLaunch>,
}

/// Arguments supplied by Thunderbird when it starts a Native Messaging host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeMessagingLaunch {
    pub manifest_path: PathBuf,
    pub extension_id: OsString,
}

impl Cli {
    pub fn parse_environment() -> Result<Self, CliError> {
        Self::parse_from(env::args_os())
    }

    pub fn parse_from<I, T>(arguments: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut arguments = arguments.into_iter().map(Into::into);
        let _program = arguments.next();
        let arguments: Vec<OsString> = arguments.collect();
        if let Some(native_launch) = native_messaging_launch(&arguments) {
            return Ok(Self {
                native_launch: Some(native_launch),
                ..Self::default()
            });
        }

        let mut arguments = arguments.into_iter();
        let mut cli = Self::default();

        while let Some(argument) = arguments.next() {
            match argument.to_str() {
                Some("--config") => {
                    cli.config = Some(PathBuf::from(next_value(&mut arguments, "--config")?));
                }
                Some("--window-backend") => {
                    let value = utf8_value(
                        next_value(&mut arguments, "--window-backend")?,
                        "--window-backend",
                    )?;
                    cli.window_backend = Some(
                        WindowBackend::parse(&value)
                            .ok_or_else(|| CliError::InvalidWindowBackend(value.clone()))?,
                    );
                }
                Some("--log-level") => {
                    let value =
                        utf8_value(next_value(&mut arguments, "--log-level")?, "--log-level")?;
                    cli.log_level = LogLevel::parse(&value)
                        .ok_or_else(|| CliError::InvalidLogLevel(value.clone()))?;
                }
                Some("--help" | "-h") => set_command(&mut cli, Command::Help)?,
                Some("--version" | "-V") => set_command(&mut cli, Command::Version)?,
                Some("--service") => set_command(&mut cli, Command::Service)?,
                Some("doctor") => set_command(&mut cli, Command::Doctor)?,
                Some("settings") => set_command(&mut cli, Command::Settings)?,
                Some("install-native-manifest") => {
                    set_command(&mut cli, Command::InstallNativeManifest)?
                }
                Some("uninstall-native-manifest") => {
                    set_command(&mut cli, Command::UninstallNativeManifest)?
                }
                _ => return Err(CliError::UnknownArgument(argument)),
            }
        }

        Ok(cli)
    }
}

fn native_messaging_launch(arguments: &[OsString]) -> Option<NativeMessagingLaunch> {
    if arguments.len() != 2 || !Path::new(&arguments[0]).is_absolute() || arguments[1].is_empty() {
        return None;
    }
    Some(NativeMessagingLaunch {
        manifest_path: PathBuf::from(&arguments[0]),
        extension_id: arguments[1].clone(),
    })
}

fn next_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &'static str,
) -> Result<OsString, CliError> {
    arguments.next().ok_or(CliError::MissingValue(option))
}

fn utf8_value(value: OsString, option: &'static str) -> Result<String, CliError> {
    value
        .into_string()
        .map_err(|_| CliError::NonUtf8Value(option))
}

fn set_command(cli: &mut Cli, command: Command) -> Result<(), CliError> {
    if cli.command != Command::Run {
        return Err(CliError::MultipleCommands);
    }
    cli.command = command;
    Ok(())
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("missing value for {0}")]
    MissingValue(&'static str),
    #[error("value for {0} is not valid UTF-8")]
    NonUtf8Value(&'static str),
    #[error("unknown window backend: {0}")]
    InvalidWindowBackend(String),
    #[error("unknown log level: {0}")]
    InvalidLogLevel(String),
    #[error("unknown argument: {}", .0.to_string_lossy())]
    UnknownArgument(OsString),
    #[error("only one command may be selected")]
    MultipleCommands,
}

pub fn display_os(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}
