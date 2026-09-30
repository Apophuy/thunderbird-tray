// SPDX-License-Identifier: GPL-3.0-only

//! Capability-based window control and shell-free Thunderbird launching.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::process::{Child, Command, Stdio};

use thiserror::Error;

use crate::config::{ThunderbirdConfig, WindowBackend as WindowBackendChoice};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesktopEnvironment {
    pub session_type: Option<OsString>,
    pub current_desktop: Option<OsString>,
    pub wayland_display: Option<OsString>,
    pub display: Option<OsString>,
}

impl DesktopEnvironment {
    pub fn from_process() -> Self {
        Self::from_lookup(|name| env::var_os(name))
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<OsString>) -> Self {
        Self {
            session_type: nonempty(lookup("XDG_SESSION_TYPE")),
            current_desktop: nonempty(lookup("XDG_CURRENT_DESKTOP")),
            wayland_display: nonempty(lookup("WAYLAND_DISPLAY")),
            display: nonempty(lookup("DISPLAY")),
        }
    }

    pub fn is_kde_wayland(&self) -> bool {
        value_eq(&self.session_type, "wayland")
            && self
                .current_desktop
                .as_deref()
                .is_some_and(|value| desktop_list_contains(value, "kde"))
    }

    pub fn is_x11(&self) -> bool {
        value_eq(&self.session_type, "x11")
            || (self.session_type.is_none()
                && self.wayland_display.is_none()
                && self.display.is_some())
    }
}

fn nonempty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

fn value_eq(value: &Option<OsString>, expected: &str) -> bool {
    value
        .as_deref()
        .and_then(OsStr::to_str)
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn desktop_list_contains(value: &OsStr, expected: &str) -> bool {
    value.to_string_lossy().split(':').any(|desktop| {
        desktop.eq_ignore_ascii_case(expected)
            || (expected == "kde" && desktop.eq_ignore_ascii_case("plasma"))
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BackendAvailability {
    pub kde_wayland: bool,
    pub x11: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionReason {
    Explicit,
    KdeWaylandDetected,
    X11Detected,
    RequestedBackendUnavailable(WindowBackendChoice),
    NoSupportedDesktop,
}

impl fmt::Display for SelectionReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Explicit => formatter.write_str("explicit configuration"),
            Self::KdeWaylandDetected => formatter.write_str("KDE Plasma Wayland detected"),
            Self::X11Detected => formatter.write_str("X11 session detected"),
            Self::RequestedBackendUnavailable(backend) => {
                write!(formatter, "{} backend is unavailable", backend.as_str())
            }
            Self::NoSupportedDesktop => {
                formatter.write_str("no supported window-control backend detected")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendSelection {
    pub requested: WindowBackendChoice,
    pub selected: WindowBackendChoice,
    pub reason: SelectionReason,
}

pub fn select_backend(
    requested: WindowBackendChoice,
    environment: &DesktopEnvironment,
    availability: BackendAvailability,
) -> BackendSelection {
    let (selected, reason) = match requested {
        WindowBackendChoice::None => (WindowBackendChoice::None, SelectionReason::Explicit),
        WindowBackendChoice::KdeWayland if availability.kde_wayland => {
            (WindowBackendChoice::KdeWayland, SelectionReason::Explicit)
        }
        WindowBackendChoice::X11 if availability.x11 => {
            (WindowBackendChoice::X11, SelectionReason::Explicit)
        }
        WindowBackendChoice::KdeWayland | WindowBackendChoice::X11 => (
            WindowBackendChoice::None,
            SelectionReason::RequestedBackendUnavailable(requested),
        ),
        WindowBackendChoice::Auto if environment.is_kde_wayland() && availability.kde_wayland => (
            WindowBackendChoice::KdeWayland,
            SelectionReason::KdeWaylandDetected,
        ),
        WindowBackendChoice::Auto if environment.is_x11() && availability.x11 => {
            (WindowBackendChoice::X11, SelectionReason::X11Detected)
        }
        WindowBackendChoice::Auto if environment.is_kde_wayland() => (
            WindowBackendChoice::None,
            SelectionReason::RequestedBackendUnavailable(WindowBackendChoice::KdeWayland),
        ),
        WindowBackendChoice::Auto if environment.is_x11() => (
            WindowBackendChoice::None,
            SelectionReason::RequestedBackendUnavailable(WindowBackendChoice::X11),
        ),
        WindowBackendChoice::Auto => (
            WindowBackendChoice::None,
            SelectionReason::NoSupportedDesktop,
        ),
    };

    BackendSelection {
        requested,
        selected,
        reason,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowCapabilities {
    pub activate: bool,
    pub hide: bool,
    pub show: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowOperation {
    Activate,
    Hide,
    Show,
}

impl fmt::Display for WindowOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Activate => "activate",
            Self::Hide => "hide",
            Self::Show => "show",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationOutcome {
    Activated,
    NoWindow,
}

pub trait WindowControl {
    fn backend(&self) -> WindowBackendChoice;
    fn capabilities(&self) -> WindowCapabilities;
    fn activate(&self) -> Result<ActivationOutcome, WindowError>;
    fn hide(&self) -> Result<(), WindowError>;
    fn show(&self) -> Result<(), WindowError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedWindowControl {
    selection: BackendSelection,
}

impl UnsupportedWindowControl {
    pub fn new(selection: BackendSelection) -> Self {
        Self { selection }
    }

    fn unsupported(&self, operation: WindowOperation) -> WindowError {
        WindowError::Unsupported {
            backend: self.selection.selected,
            operation,
            reason: self.selection.reason.to_string(),
        }
    }
}

impl WindowControl for UnsupportedWindowControl {
    fn backend(&self) -> WindowBackendChoice {
        self.selection.selected
    }

    fn capabilities(&self) -> WindowCapabilities {
        WindowCapabilities::default()
    }

    fn activate(&self) -> Result<ActivationOutcome, WindowError> {
        Err(self.unsupported(WindowOperation::Activate))
    }

    fn hide(&self) -> Result<(), WindowError> {
        Err(self.unsupported(WindowOperation::Hide))
    }

    fn show(&self) -> Result<(), WindowError> {
        Err(self.unsupported(WindowOperation::Show))
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WindowError {
    #[error("{operation} is unsupported by the {backend} backend: {reason}")]
    Unsupported {
        backend: WindowBackendChoice,
        operation: WindowOperation,
        reason: String,
    },
    #[error("{operation} failed in the {backend} backend: {message}")]
    OperationFailed {
        backend: WindowBackendChoice,
        operation: WindowOperation,
        message: String,
    },
}

impl fmt::Display for WindowBackendChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub trait ThunderbirdLauncher {
    type Handle;

    fn launch(&self, config: &ThunderbirdConfig) -> Result<Self::Handle, LaunchError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessLauncher;

impl ProcessLauncher {
    pub fn command(config: &ThunderbirdConfig) -> Command {
        let mut command = Command::new(&config.command);
        command
            .args(&config.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        command
    }
}

impl ThunderbirdLauncher for ProcessLauncher {
    type Handle = Child;

    fn launch(&self, config: &ThunderbirdConfig) -> Result<Self::Handle, LaunchError> {
        Self::command(config)
            .spawn()
            .map_err(|source| LaunchError::Spawn {
                command: config.command.clone(),
                source,
            })
    }
}

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("could not start Thunderbird command {command:?}: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub enum OpenOutcome<Handle> {
    ActivatedExisting,
    Launched(Handle),
}

pub fn open_thunderbird<Backend, Launcher>(
    backend: &Backend,
    launcher: &Launcher,
    config: &ThunderbirdConfig,
) -> Result<OpenOutcome<Launcher::Handle>, OpenError>
where
    Backend: WindowControl + ?Sized,
    Launcher: ThunderbirdLauncher,
{
    if backend.capabilities().activate {
        match backend.activate()? {
            ActivationOutcome::Activated => return Ok(OpenOutcome::ActivatedExisting),
            ActivationOutcome::NoWindow => {}
        }
    }

    launcher
        .launch(config)
        .map(OpenOutcome::Launched)
        .map_err(OpenError::Launch)
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error(transparent)]
    Window(#[from] WindowError),
    #[error(transparent)]
    Launch(#[from] LaunchError),
}

pub fn reap_children(children: &mut Vec<Child>) {
    children.retain_mut(|child| match child.try_wait() {
        Ok(None) => true,
        Ok(Some(status)) => {
            tracing::debug!(pid = child.id(), %status, "Thunderbird launcher child exited");
            false
        }
        Err(source) => {
            tracing::warn!(pid = child.id(), error = %source, "could not reap Thunderbird launcher child");
            false
        }
    });
}
