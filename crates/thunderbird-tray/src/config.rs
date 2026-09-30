// SPDX-License-Identifier: GPL-3.0-only

//! Strict application configuration and XDG path discovery.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const CONFIG_DIRECTORY: &str = "thunderbird-tray";
const CONFIG_FILE: &str = "config.toml";

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub general: GeneralConfig,
    pub tray: TrayConfig,
    pub thunderbird: ThunderbirdConfig,
    pub window: WindowConfig,
}

impl Config {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.thunderbird.command.trim().is_empty() {
            return Err(ConfigError::EmptyThunderbirdCommand);
        }
        Ok(())
    }

    pub fn set_language(&mut self, language: LanguageMode) {
        self.general.language = language;
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GeneralConfig {
    pub start_thunderbird: bool,
    pub notifications: bool,
    pub language: LanguageMode,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_thunderbird: false,
            notifications: true,
            language: LanguageMode::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TrayConfig {
    pub show_unread_count: bool,
    pub hide_when_zero: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            show_unread_count: true,
            hide_when_zero: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThunderbirdConfig {
    pub command: String,
    pub arguments: Vec<String>,
}

impl Default for ThunderbirdConfig {
    fn default() -> Self {
        Self {
            command: "thunderbird".to_owned(),
            arguments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowConfig {
    pub backend: WindowBackend,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            backend: WindowBackend::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowBackend {
    #[default]
    Auto,
    KdeWayland,
    X11,
    None,
}

impl WindowBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "kde-wayland" => Some(Self::KdeWayland),
            "x11" => Some(Self::X11),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::KdeWayland => "kde-wayland",
            Self::X11 => "x11",
            Self::None => "none",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageMode {
    #[default]
    Auto,
    En,
    Ru,
}

impl LanguageMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::En => "en",
            Self::Ru => "ru",
        }
    }
}

/// Resolved configuration path and whether the user supplied it explicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSource {
    path: PathBuf,
    required: bool,
}

impl ConfigSource {
    pub fn discover(explicit: Option<PathBuf>) -> Result<Self, ConfigError> {
        Self::from_environment(
            explicit,
            env::var_os("XDG_CONFIG_HOME").as_deref(),
            env::var_os("HOME").as_deref(),
        )
    }

    pub fn from_environment(
        explicit: Option<PathBuf>,
        xdg_config_home: Option<&OsStr>,
        home: Option<&OsStr>,
    ) -> Result<Self, ConfigError> {
        if let Some(path) = explicit {
            return Ok(Self {
                path,
                required: true,
            });
        }

        let base = xdg_config_home
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| home.map(PathBuf::from).map(|path| path.join(".config")))
            .ok_or(ConfigError::ConfigHomeUnavailable)?;
        Ok(Self {
            path: base.join(CONFIG_DIRECTORY).join(CONFIG_FILE),
            required: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Config, ConfigError> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound && !self.required => {
                return Ok(Config::default());
            }
            Err(source) => {
                return Err(ConfigError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        };
        let config: Config = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: self.path.clone(),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, config: &Config) -> Result<(), ConfigError> {
        config.validate()?;
        let contents = toml::to_string_pretty(config).map_err(ConfigError::Serialize)?;
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| ConfigError::CreateDirectory {
                path: parent.to_owned(),
                source,
            })?;
        }
        fs::write(&self.path, contents).map_err(|source| ConfigError::Write {
            path: self.path.clone(),
            source,
        })
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("neither XDG_CONFIG_HOME nor HOME provides a configuration directory")]
    ConfigHomeUnavailable,
    #[error("could not read configuration {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("invalid configuration {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("Thunderbird command must not be empty")]
    EmptyThunderbirdCommand,
    #[error("could not serialize configuration: {0}")]
    Serialize(toml::ser::Error),
    #[error("could not create configuration directory {path}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("could not write configuration {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
}
