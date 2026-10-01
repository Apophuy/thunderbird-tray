// SPDX-License-Identifier: GPL-3.0-only

//! KDE Plasma Wayland window control through one-shot KWin scripts.

use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use thiserror::Error;
use zbus::blocking::{Connection, Proxy};
use zbus::names::BusName;

use crate::config::WindowBackend;
use crate::lifecycle::{
    APPLICATION_ID, LIFECYCLE_INTERFACE, LIFECYCLE_OBJECT_PATH, WindowActionReport,
};
use crate::window::{
    ActivationOutcome, DesktopEnvironment, WindowCapabilities, WindowControl, WindowError,
    WindowOperation,
};

const KWIN_SERVICE: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
const SCRIPT_INTERFACE: &str = "org.kde.kwin.Script";
const ACTION_TIMEOUT: Duration = Duration::from_secs(2);
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

pub struct KdeWindowControl {
    connection: Connection,
    reports: Receiver<WindowActionReport>,
    runtime_directory: PathBuf,
}

impl KdeWindowControl {
    pub fn is_available(environment: &DesktopEnvironment) -> Result<bool, KdeError> {
        if !probe_environment(environment) {
            return Ok(false);
        }

        let connection = Connection::session()?;
        if !kwin_scripting_available(&connection)? {
            return Ok(false);
        }
        runtime_directory()?;
        Ok(true)
    }

    pub fn connect(
        environment: &DesktopEnvironment,
        reports: Receiver<WindowActionReport>,
    ) -> Result<Option<Self>, KdeError> {
        if !Self::is_available(environment)? {
            return Ok(None);
        }

        let connection = Connection::session()?;
        let runtime_directory = runtime_directory()?;
        fs::create_dir_all(&runtime_directory).map_err(|source| KdeError::RuntimeDirectory {
            path: runtime_directory.clone(),
            source,
        })?;
        fs::set_permissions(&runtime_directory, fs::Permissions::from_mode(0o700)).map_err(
            |source| KdeError::RuntimeDirectory {
                path: runtime_directory.clone(),
                source,
            },
        )?;

        Ok(Some(Self {
            connection,
            reports,
            runtime_directory,
        }))
    }

    fn perform(&self, action: KdeAction) -> Result<KdeOutcome, KdeError> {
        while self.reports.try_recv().is_ok() {}

        let sequence = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed);
        let request_id = format!("{}-{sequence}", std::process::id());
        let plugin_name = format!("thunderbird-tray-{request_id}");
        let path = self.runtime_directory.join(format!("{plugin_name}.js"));
        write_script(&path, &render_script(action, &request_id))?;

        let scripting = Proxy::new(
            &self.connection,
            KWIN_SERVICE,
            SCRIPTING_PATH,
            SCRIPTING_INTERFACE,
        )?;
        let path_text = path
            .to_str()
            .ok_or_else(|| KdeError::NonUtf8Path(path.clone()))?;
        let script_id: i32 = match scripting.call("loadScript", &(path_text, plugin_name.as_str()))
        {
            Ok(id) => id,
            Err(source) => {
                let _ = fs::remove_file(&path);
                return Err(KdeError::Bus(source));
            }
        };

        let result = (|| {
            let script_path = format!("/Scripting/Script{script_id}");
            let script = Proxy::new(
                &self.connection,
                KWIN_SERVICE,
                script_path.as_str(),
                SCRIPT_INTERFACE,
            )?;
            let _: () = script.call("run", &())?;
            self.wait_for_report(&request_id)
        })();

        match scripting.call::<_, _, bool>("unloadScript", &plugin_name.as_str()) {
            Ok(true) => {}
            Ok(false) => tracing::warn!(plugin_name, "KWin script was already unloaded"),
            Err(source) => {
                tracing::warn!(plugin_name, error = %source, "could not unload KWin script")
            }
        }
        if let Err(source) = fs::remove_file(&path) {
            tracing::warn!(path = %path.display(), error = %source, "could not remove generated KWin script");
        }

        result
    }

    fn wait_for_report(&self, request_id: &str) -> Result<KdeOutcome, KdeError> {
        let deadline = Instant::now() + ACTION_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(KdeError::Timeout);
            }
            let report = self
                .reports
                .recv_timeout(remaining)
                .map_err(|_| KdeError::Timeout)?;
            if report.request_id == request_id {
                return KdeOutcome::parse(&report.outcome);
            }
        }
    }

    fn operation_error(operation: WindowOperation, source: KdeError) -> WindowError {
        WindowError::OperationFailed {
            backend: WindowBackend::KdeWayland,
            operation,
            message: source.to_string(),
        }
    }
}

fn kwin_scripting_available(connection: &Connection) -> Result<bool, KdeError> {
    let dbus = zbus::blocking::fdo::DBusProxy::new(connection)?;
    let name = BusName::try_from(KWIN_SERVICE)?;
    if !dbus.name_has_owner(name)? {
        return Ok(false);
    }

    let scripting = Proxy::new(
        connection,
        KWIN_SERVICE,
        SCRIPTING_PATH,
        SCRIPTING_INTERFACE,
    )?;
    let _: bool = scripting.call("isScriptLoaded", &"thunderbird-tray-probe")?;
    Ok(true)
}

impl WindowControl for KdeWindowControl {
    fn backend(&self) -> WindowBackend {
        WindowBackend::KdeWayland
    }

    fn capabilities(&self) -> WindowCapabilities {
        advertised_capabilities()
    }

    fn detect(&self) -> Result<bool, WindowError> {
        self.perform(KdeAction::Detect)
            .and_then(|outcome| match outcome {
                KdeOutcome::Found => Ok(true),
                KdeOutcome::NoWindow => Ok(false),
                other => Err(KdeError::UnexpectedOutcome(other.as_str().to_owned())),
            })
            .map_err(|source| Self::operation_error(WindowOperation::Detect, source))
    }

    fn activate(&self) -> Result<ActivationOutcome, WindowError> {
        self.perform(KdeAction::Activate)
            .and_then(|outcome| match outcome {
                KdeOutcome::Activated => Ok(ActivationOutcome::Activated),
                KdeOutcome::NoWindow => Ok(ActivationOutcome::NoWindow),
                other => Err(KdeError::UnexpectedOutcome(other.as_str().to_owned())),
            })
            .map_err(|source| Self::operation_error(WindowOperation::Activate, source))
    }

    fn hide(&self) -> Result<(), WindowError> {
        self.perform(KdeAction::Hide)
            .and_then(|outcome| match outcome {
                KdeOutcome::Hidden => Ok(()),
                other => Err(KdeError::UnexpectedOutcome(other.as_str().to_owned())),
            })
            .map_err(|source| Self::operation_error(WindowOperation::Hide, source))
    }

    fn show(&self) -> Result<(), WindowError> {
        self.perform(KdeAction::Show)
            .and_then(|outcome| match outcome {
                KdeOutcome::Shown => Ok(()),
                other => Err(KdeError::UnexpectedOutcome(other.as_str().to_owned())),
            })
            .map_err(|source| Self::operation_error(WindowOperation::Show, source))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KdeAction {
    Detect,
    Activate,
    Hide,
    Show,
}

impl KdeAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Activate => "activate",
            Self::Hide => "hide",
            Self::Show => "show",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KdeOutcome {
    Found,
    Activated,
    Hidden,
    Shown,
    NoWindow,
    Unsupported,
}

impl KdeOutcome {
    fn parse(value: &str) -> Result<Self, KdeError> {
        match value {
            "found" => Ok(Self::Found),
            "activated" => Ok(Self::Activated),
            "hidden" => Ok(Self::Hidden),
            "shown" => Ok(Self::Shown),
            "noWindow" => Ok(Self::NoWindow),
            "unsupported" => Ok(Self::Unsupported),
            other => Err(KdeError::UnexpectedOutcome(other.to_owned())),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::Activated => "activated",
            Self::Hidden => "hidden",
            Self::Shown => "shown",
            Self::NoWindow => "noWindow",
            Self::Unsupported => "unsupported",
        }
    }
}

fn runtime_directory() -> Result<PathBuf, KdeError> {
    runtime_directory_from(env::var_os("XDG_RUNTIME_DIR"))
        .ok_or(KdeError::RuntimeDirectoryUnavailable)
}

fn runtime_directory_from(value: Option<OsString>) -> Option<PathBuf> {
    value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| path.join("thunderbird-tray"))
}

fn probe_environment(environment: &DesktopEnvironment) -> bool {
    environment.is_kde_wayland()
}

fn advertised_capabilities() -> WindowCapabilities {
    WindowCapabilities {
        detect: true,
        activate: true,
        hide: true,
        show: true,
    }
}

fn write_script(path: &Path, contents: &str) -> Result<(), KdeError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| KdeError::ScriptWrite {
            path: path.to_owned(),
            source,
        })?;
    file.write_all(contents.as_bytes())
        .map_err(|source| KdeError::ScriptWrite {
            path: path.to_owned(),
            source,
        })
}

fn render_script(action: KdeAction, request_id: &str) -> String {
    format!(
        r#"// SPDX-License-Identifier: GPL-3.0-only
(function () {{
    const action = "{}";
    const requestId = "{}";
    const windows = workspace.stackingOrder;
    let target = null;
    for (let index = windows.length - 1; index >= 0; --index) {{
        const window = windows[index];
        const desktopFile = String(window.desktopFileName || "").toLowerCase();
        const resourceClass = String(window.resourceClass || "").toLowerCase();
        if (desktopFile === "thunderbird" ||
            desktopFile.endsWith("/thunderbird.desktop") ||
            resourceClass === "thunderbird" ||
            resourceClass === "thunderbird-default") {{
            target = window;
            break;
        }}
    }}

    let outcome = "noWindow";
    if (target !== null) {{
        if (action === "detect") {{
            outcome = "found";
        }} else if (action === "activate") {{
            target.skipTaskbar = false;
            target.minimized = false;
            workspace.activeWindow = target;
            outcome = !target.skipTaskbar && !target.minimized && workspace.activeWindow === target
                ? "activated" : "unsupported";
        }} else if (action === "show") {{
            target.skipTaskbar = false;
            target.minimized = false;
            workspace.activeWindow = target;
            outcome = !target.skipTaskbar && !target.minimized ? "shown" : "unsupported";
        }} else if (action === "hide" && target.minimizable) {{
            target.skipTaskbar = true;
            target.minimized = true;
            outcome = target.skipTaskbar && target.minimized ? "hidden" : "unsupported";
        }} else {{
            outcome = "unsupported";
        }}
    }}

    callDBus("{}", "{}", "{}", "ReportWindowAction",
             requestId, outcome);
}})();
"#,
        action.as_str(),
        request_id,
        APPLICATION_ID,
        LIFECYCLE_OBJECT_PATH,
        LIFECYCLE_INTERFACE,
    )
}

#[derive(Debug, Error)]
pub enum KdeError {
    #[error("session D-Bus operation failed: {0}")]
    Bus(#[from] zbus::Error),
    #[error("session D-Bus request failed: {0}")]
    Fdo(#[from] zbus::fdo::Error),
    #[error("invalid KWin bus name: {0}")]
    InvalidBusName(#[from] zbus::names::Error),
    #[error("XDG_RUNTIME_DIR is unavailable for generated KWin scripts")]
    RuntimeDirectoryUnavailable,
    #[error("could not prepare KWin script directory {path}: {source}")]
    RuntimeDirectory { path: PathBuf, source: io::Error },
    #[error("could not write generated KWin script {path}: {source}")]
    ScriptWrite { path: PathBuf, source: io::Error },
    #[error("generated KWin script path is not UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error("KWin script did not report a result in time")]
    Timeout,
    #[error("KWin script returned unexpected outcome {0:?}")]
    UnexpectedOutcome(String),
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    fn desktop_environment(values: &[(&str, &str)]) -> DesktopEnvironment {
        DesktopEnvironment::from_lookup(|name| {
            values
                .iter()
                .find_map(|(key, value)| (*key == name).then(|| OsString::from(value)))
        })
    }

    #[test]
    fn probe_requires_plasma_wayland_and_an_absolute_runtime_directory() {
        let plasma_wayland = desktop_environment(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", "KDE"),
        ]);
        let plasma_x11 =
            desktop_environment(&[("XDG_SESSION_TYPE", "x11"), ("XDG_CURRENT_DESKTOP", "KDE")]);

        assert!(probe_environment(&plasma_wayland));
        assert!(!probe_environment(&plasma_x11));
        assert_eq!(
            runtime_directory_from(Some(OsString::from("/run/user/1000"))),
            Some(PathBuf::from("/run/user/1000/thunderbird-tray"))
        );
        assert_eq!(
            runtime_directory_from(Some(OsString::from("relative"))),
            None
        );
        assert_eq!(runtime_directory_from(None), None);
    }

    #[test]
    fn a_successful_probe_advertises_each_implemented_operation() {
        assert_eq!(
            advertised_capabilities(),
            WindowCapabilities {
                detect: true,
                activate: true,
                hide: true,
                show: true,
            }
        );
    }

    #[test]
    fn generated_script_uses_only_documented_window_properties_and_fixed_dbus_target() {
        let script = render_script(KdeAction::Activate, "123-4");
        assert!(script.contains("workspace.stackingOrder"));
        assert!(script.contains("window.desktopFileName"));
        assert!(script.contains("window.resourceClass"));
        assert!(script.contains("target.minimized = false"));
        assert!(script.contains("target.skipTaskbar = false"));
        assert!(script.contains("workspace.activeWindow = target"));
        assert!(script.contains(APPLICATION_ID));
        assert!(script.contains("ReportWindowAction"));
        assert!(script.contains("123-4"));

        let hide_script = render_script(KdeAction::Hide, "123-5");
        assert!(hide_script.contains("target.skipTaskbar = true"));
        assert!(hide_script.contains("target.minimized = true"));
        assert!(hide_script.contains("target.skipTaskbar && target.minimized"));
    }

    #[test]
    fn reports_are_parsed_strictly() {
        assert_eq!(KdeOutcome::parse("found").unwrap(), KdeOutcome::Found);
        assert_eq!(KdeOutcome::parse("noWindow").unwrap(), KdeOutcome::NoWindow);
        assert!(matches!(
            KdeOutcome::parse("pretend-success"),
            Err(KdeError::UnexpectedOutcome(_))
        ));
    }
}
