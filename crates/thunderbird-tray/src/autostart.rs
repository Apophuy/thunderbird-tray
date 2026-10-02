// SPDX-License-Identifier: GPL-3.0-only

//! Per-user XDG autostart integration controlled from the settings window.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

const AUTOSTART_FILE: &str = "io.github.apophuy.thunderbird-tray.desktop";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutostartEntry {
    path: PathBuf,
    executable: PathBuf,
}

impl AutostartEntry {
    pub fn discover() -> Result<Self, AutostartError> {
        let executable = env::current_exe().map_err(AutostartError::CurrentExecutable)?;
        Self::from_environment(
            executable,
            env::var_os("XDG_CONFIG_HOME").as_deref(),
            env::var_os("HOME").as_deref(),
        )
    }

    pub fn from_environment(
        executable: PathBuf,
        xdg_config_home: Option<&OsStr>,
        home: Option<&OsStr>,
    ) -> Result<Self, AutostartError> {
        let base = xdg_config_home
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| home.map(PathBuf::from).map(|path| path.join(".config")))
            .ok_or(AutostartError::ConfigHomeUnavailable)?;
        if !executable.is_absolute() {
            return Err(AutostartError::ExecutableNotAbsolute(executable));
        }
        Ok(Self {
            path: base.join("autostart").join(AUTOSTART_FILE),
            executable,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_enabled(&self) -> bool {
        self.path.is_file()
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<(), AutostartError> {
        if enabled {
            let parent = self
                .path
                .parent()
                .ok_or_else(|| AutostartError::InvalidPath(self.path.clone()))?;
            fs::create_dir_all(parent).map_err(|source| AutostartError::CreateDirectory {
                path: parent.to_owned(),
                source,
            })?;
            fs::write(&self.path, self.desktop_entry()?).map_err(|source| AutostartError::Write {
                path: self.path.clone(),
                source,
            })
        } else {
            match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(source) => Err(AutostartError::Remove {
                    path: self.path.clone(),
                    source,
                }),
            }
        }
    }

    fn desktop_entry(&self) -> Result<String, AutostartError> {
        let executable = self
            .executable
            .to_str()
            .ok_or_else(|| AutostartError::NonUtf8Executable(self.executable.clone()))?;
        if executable.contains(['\n', '\r']) {
            return Err(AutostartError::InvalidExecutable(self.executable.clone()));
        }
        let escaped = executable
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$");
        Ok(format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Version=1.0\n\
             Name=thunderbird-tray\n\
             Name[ru]=thunderbird-tray\n\
             Comment=Keep Thunderbird available from the system tray\n\
             Comment[ru]=Запускать Thunderbird вместе с системным треем\n\
             Exec=\"{escaped}\" --service\n\
             Icon=io.github.apophuy.thunderbird-tray\n\
             Terminal=false\n\
             X-KDE-autostart-after=panel\n"
        ))
    }
}

#[derive(Debug, Error)]
pub enum AutostartError {
    #[error("neither XDG_CONFIG_HOME nor HOME provides a configuration directory")]
    ConfigHomeUnavailable,
    #[error("could not resolve the current executable: {0}")]
    CurrentExecutable(io::Error),
    #[error("autostart executable is not an absolute path: {0}")]
    ExecutableNotAbsolute(PathBuf),
    #[error("autostart executable path is not UTF-8: {0}")]
    NonUtf8Executable(PathBuf),
    #[error("autostart executable path contains a line break: {0}")]
    InvalidExecutable(PathBuf),
    #[error("invalid autostart path: {0}")]
    InvalidPath(PathBuf),
    #[error("could not create autostart directory {path}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("could not write autostart entry {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("could not remove autostart entry {path}: {source}")]
    Remove { path: PathBuf, source: io::Error },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_path_and_desktop_entry_are_deterministic() {
        let entry = AutostartEntry::from_environment(
            PathBuf::from("/opt/thunderbird tray/bin/thunderbird-tray"),
            Some(OsStr::new("/tmp/config")),
            None,
        )
        .unwrap();
        assert_eq!(
            entry.path(),
            Path::new("/tmp/config/autostart/io.github.apophuy.thunderbird-tray.desktop")
        );
        let desktop = entry.desktop_entry().unwrap();
        assert!(desktop.contains("Type=Application"));
        assert!(desktop.contains("Exec=\"/opt/thunderbird tray/bin/thunderbird-tray\" --service"));
        assert!(desktop.contains("Terminal=false"));
        assert!(desktop.contains("Icon=io.github.apophuy.thunderbird-tray"));
    }

    #[test]
    fn enable_and_disable_are_idempotent() {
        let temporary =
            std::env::temp_dir().join(format!("thunderbird-tray-autostart-{}", std::process::id()));
        let entry = AutostartEntry::from_environment(
            PathBuf::from("/opt/thunderbird-tray/bin/thunderbird-tray"),
            Some(temporary.as_os_str()),
            None,
        )
        .unwrap();
        entry.set_enabled(false).unwrap();
        entry.set_enabled(true).unwrap();
        assert!(entry.is_enabled());
        entry.set_enabled(true).unwrap();
        entry.set_enabled(false).unwrap();
        entry.set_enabled(false).unwrap();
        assert!(!entry.is_enabled());
        let _ = fs::remove_dir_all(temporary);
    }
}
