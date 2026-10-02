// SPDX-License-Identifier: GPL-3.0-only

//! Secure per-user Native Messaging manifest installation.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

pub const NATIVE_HOST_NAME: &str = env!("THUNDERBIRD_TRAY_NATIVE_HOST_NAME");
pub const EXTENSION_ID: &str = env!("THUNDERBIRD_TRAY_EXTENSION_ID");

pub fn install_default() -> Result<PathBuf, ManifestInstallError> {
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(ManifestInstallError::HomeUnavailable)?;
    let binary = env::current_exe().map_err(ManifestInstallError::CurrentExecutable)?;
    let output = manifest_path(&home);
    install(&binary, &output)?;
    Ok(output)
}

pub fn manifest_path(home: &Path) -> PathBuf {
    home.join(".mozilla")
        .join("native-messaging-hosts")
        .join(format!("{NATIVE_HOST_NAME}.json"))
}

pub fn remove_default() -> Result<(PathBuf, bool), ManifestInstallError> {
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(ManifestInstallError::HomeUnavailable)?;
    let output = manifest_path(&home);
    match fs::remove_file(&output) {
        Ok(()) => Ok((output, true)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok((output, false)),
        Err(source) => Err(ManifestInstallError::Remove {
            path: output,
            source,
        }),
    }
}

pub fn install(binary: &Path, output: &Path) -> Result<(), ManifestInstallError> {
    if !binary.is_absolute() {
        return Err(ManifestInstallError::BinaryNotAbsolute(binary.to_owned()));
    }
    let directory = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| ManifestInstallError::InvalidOutput(output.to_owned()))?;
    fs::create_dir_all(directory).map_err(|source| ManifestInstallError::CreateDirectory {
        path: directory.to_owned(),
        source,
    })?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(|source| {
        ManifestInstallError::SetPermissions {
            path: directory.to_owned(),
            source,
        }
    })?;

    let manifest = NativeManifest {
        name: NATIVE_HOST_NAME,
        description: "Native Messaging host for thunderbird-tray",
        path: binary,
        kind: "stdio",
        allowed_extensions: [EXTENSION_ID],
    };
    let contents = serde_json::to_vec_pretty(&manifest)?;
    let temporary = output.with_extension(format!("json.tmp-{}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|source| ManifestInstallError::Write {
                path: temporary.clone(),
                source,
            })?;
        file.write_all(&contents)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| ManifestInstallError::Write {
                path: temporary.clone(),
                source,
            })?;
        fs::rename(&temporary, output).map_err(|source| ManifestInstallError::Replace {
            path: output.to_owned(),
            source,
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[derive(Serialize)]
struct NativeManifest<'a> {
    name: &'a str,
    description: &'a str,
    path: &'a Path,
    #[serde(rename = "type")]
    kind: &'a str,
    allowed_extensions: [&'a str; 1],
}

#[derive(Debug, Error)]
pub enum ManifestInstallError {
    #[error("HOME is unavailable")]
    HomeUnavailable,
    #[error("could not determine the current executable: {0}")]
    CurrentExecutable(#[source] io::Error),
    #[error("native host path is not absolute: {0}")]
    BinaryNotAbsolute(PathBuf),
    #[error("native manifest output has no parent directory: {0}")]
    InvalidOutput(PathBuf),
    #[error("could not create native manifest directory {path}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("could not set permissions on {path}: {source}")]
    SetPermissions { path: PathBuf, source: io::Error },
    #[error("could not serialize native manifest: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("could not write native manifest temporary file {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("could not replace native manifest {path}: {source}")]
    Replace { path: PathBuf, source: io::Error },
    #[error("could not remove native manifest {path}: {source}")]
    Remove { path: PathBuf, source: io::Error },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_uses_absolute_binary_and_stable_identifiers() {
        let root = std::env::temp_dir().join(format!(
            "thunderbird-tray-manifest-test-{}",
            std::process::id()
        ));
        let output = manifest_path(&root);
        install(
            Path::new("/opt/thunderbird-tray/bin/thunderbird-tray"),
            &output,
        )
        .unwrap();
        let contents = fs::read_to_string(&output).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(manifest["name"], NATIVE_HOST_NAME);
        assert_eq!(
            manifest["path"],
            "/opt/thunderbird-tray/bin/thunderbird-tray"
        );
        assert_eq!(manifest["allowed_extensions"][0], EXTENSION_ID);
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn relative_binary_is_rejected_before_writing() {
        assert!(matches!(
            install(
                Path::new("bin/thunderbird-tray"),
                Path::new("/tmp/host.json")
            ),
            Err(ManifestInstallError::BinaryNotAbsolute(_))
        ));
    }
}
