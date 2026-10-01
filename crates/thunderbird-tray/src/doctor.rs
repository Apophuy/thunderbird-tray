// SPDX-License-Identifier: GPL-3.0-only

//! Privacy-preserving runtime diagnostics.

use std::env;
use std::fmt::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use zbus::blocking::Connection;
use zbus::names::BusName;

use crate::config::{Config, LanguageMode, WindowBackend};
use crate::i18n::{Language, Localizer};
use crate::kde_wayland::KdeWindowControl;
use crate::lifecycle::{APPLICATION_ID, LifecycleClient};
use crate::window::{BackendAvailability, DesktopEnvironment, SelectionReason, select_backend};
use crate::x11::{X11WindowControl, feature_enabled as x11_feature_enabled};

const KDE_WATCHER: &str = "org.kde.StatusNotifierWatcher";
const FREEDESKTOP_WATCHER: &str = "org.freedesktop.StatusNotifierWatcher";
const KWIN_SERVICE: &str = "org.kde.KWin";
const NATIVE_HOST_NAME: &str = env!("THUNDERBIRD_TRAY_NATIVE_HOST_NAME");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtensionConnection {
    Connected,
    Disconnected,
    ApplicationNotRunning,
    Unknown,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BusDiagnostics {
    available: bool,
    application_running: bool,
    extension_connection: Option<ExtensionConnection>,
    watcher: Option<&'static str>,
    kwin_running: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorReport {
    config_path: PathBuf,
    config_exists: bool,
    requested_backend: WindowBackend,
    selected_backend: WindowBackend,
    selection_reason: SelectionReason,
    language_mode: LanguageMode,
    resolved_language: Language,
    session_type: Option<String>,
    desktop: Option<String>,
    session_bus: bool,
    application_running: bool,
    extension_connection: ExtensionConnection,
    watcher: Option<&'static str>,
    kwin_running: bool,
    x11_feature: bool,
    native_manifest_path: Option<PathBuf>,
    native_manifest_exists: bool,
    thunderbird_executable: bool,
    home: Option<PathBuf>,
}

impl DoctorReport {
    pub fn collect(
        config_path: &Path,
        config: &Config,
        backend_override: Option<WindowBackend>,
        resolved_language: Language,
    ) -> Self {
        let environment = DesktopEnvironment::from_process();
        let bus = collect_bus_diagnostics();
        let kde_wayland = KdeWindowControl::is_available(&environment).unwrap_or(false);
        let x11 = X11WindowControl::connect(&environment)
            .map(|backend| backend.is_some())
            .unwrap_or(false);
        let requested_backend = backend_override.unwrap_or(config.window.backend);
        let selection = select_backend(
            requested_backend,
            &environment,
            BackendAvailability { kde_wayland, x11 },
        );
        let home = env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let native_manifest_path = native_manifest_path(home.as_deref());
        let native_manifest_exists = native_manifest_path.as_deref().is_some_and(Path::is_file);

        Self {
            config_path: config_path.to_owned(),
            config_exists: config_path.is_file(),
            requested_backend,
            selected_backend: selection.selected,
            selection_reason: selection.reason,
            language_mode: config.general.language,
            resolved_language,
            session_type: environment_value(environment.session_type.as_deref()),
            desktop: environment_value(environment.current_desktop.as_deref()),
            session_bus: bus.available,
            application_running: bus.application_running,
            extension_connection: bus
                .extension_connection
                .unwrap_or(ExtensionConnection::Unknown),
            watcher: bus.watcher,
            kwin_running: bus.kwin_running,
            x11_feature: x11_feature_enabled(),
            native_manifest_path,
            native_manifest_exists,
            thunderbird_executable: executable_available(&config.thunderbird.command),
            home,
        }
    }

    pub fn render(&self, localizer: Localizer) -> String {
        let labels = localizer.doctor_labels();
        let yes_no = |value| if value { labels.yes } else { labels.no };
        let available = |value| {
            if value {
                labels.available
            } else {
                labels.unavailable
            }
        };
        let mut output = String::new();
        writeln!(output, "{}", localizer.doctor_title())
            .expect("writing to an in-memory String cannot fail");
        line(
            &mut output,
            labels.config,
            &redacted_path(&self.config_path, self.home.as_deref()),
        );
        line(
            &mut output,
            labels.config_exists,
            yes_no(self.config_exists),
        );
        line(
            &mut output,
            labels.session_type,
            self.session_type.as_deref().unwrap_or(labels.unknown),
        );
        line(
            &mut output,
            labels.desktop,
            self.desktop.as_deref().unwrap_or(labels.unknown),
        );
        line(&mut output, labels.session_bus, available(self.session_bus));
        line(
            &mut output,
            labels.application_service,
            if self.application_running {
                labels.running
            } else {
                labels.not_running
            },
        );
        line(
            &mut output,
            labels.extension_connection,
            match self.extension_connection {
                ExtensionConnection::Connected => labels.connected,
                ExtensionConnection::Disconnected => labels.disconnected,
                ExtensionConnection::ApplicationNotRunning => labels.application_not_running,
                ExtensionConnection::Unknown => labels.unknown,
            },
        );
        line(
            &mut output,
            labels.status_notifier_watcher,
            self.watcher.unwrap_or(labels.unavailable),
        );
        line(
            &mut output,
            labels.kwin_service,
            if self.kwin_running {
                labels.running
            } else {
                labels.not_running
            },
        );
        line(
            &mut output,
            labels.requested_backend,
            self.requested_backend.as_str(),
        );
        line(
            &mut output,
            labels.selected_backend,
            self.selected_backend.as_str(),
        );
        line(
            &mut output,
            labels.selection_reason,
            &localizer.selection_reason(self.selection_reason),
        );
        line(
            &mut output,
            labels.language_mode,
            self.language_mode.as_str(),
        );
        line(
            &mut output,
            labels.resolved_language,
            self.resolved_language.as_str(),
        );
        line(
            &mut output,
            labels.x11_feature,
            if self.x11_feature {
                labels.enabled
            } else {
                labels.disabled
            },
        );
        line(
            &mut output,
            labels.native_manifest,
            &self
                .native_manifest_path
                .as_deref()
                .map(|path| redacted_path(path, self.home.as_deref()))
                .unwrap_or_else(|| labels.unknown.to_owned()),
        );
        line(
            &mut output,
            labels.native_manifest_exists,
            yes_no(self.native_manifest_exists),
        );
        line(
            &mut output,
            labels.thunderbird_executable,
            available(self.thunderbird_executable),
        );

        let mut actions = Vec::new();
        if !self.session_bus {
            actions.push(labels.action_session_bus);
        } else if self.watcher.is_none() {
            actions.push(labels.action_watcher);
        }
        if !self.native_manifest_exists {
            actions.push(labels.action_manifest);
        }
        if !self.thunderbird_executable {
            actions.push(labels.action_executable);
        }
        if self.requested_backend == WindowBackend::X11 && !self.x11_feature {
            actions.push(labels.action_x11_feature);
        }
        if !actions.is_empty() {
            writeln!(output, "{}:", labels.actions)
                .expect("writing to an in-memory String cannot fail");
            for action in actions {
                writeln!(output, "- {action}").expect("writing to an in-memory String cannot fail");
            }
        }
        output
    }
}

fn line(output: &mut String, label: &str, value: &str) {
    writeln!(output, "{label}: {value}").expect("writing to an in-memory String cannot fail");
}

fn collect_bus_diagnostics() -> BusDiagnostics {
    let Ok(connection) = Connection::session() else {
        return BusDiagnostics::default();
    };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
        return BusDiagnostics::default();
    };
    let owner = |name: &'static str| {
        BusName::try_from(name)
            .ok()
            .and_then(|name| proxy.name_has_owner(name).ok())
            .unwrap_or(false)
    };
    let application_running = owner(APPLICATION_ID);
    let extension_connection = if application_running {
        match LifecycleClient::connect().and_then(|client| client.native_session_active()) {
            Ok(true) => ExtensionConnection::Connected,
            Ok(false) => ExtensionConnection::Disconnected,
            Err(_) => ExtensionConnection::Unknown,
        }
    } else {
        ExtensionConnection::ApplicationNotRunning
    };
    let watcher = if owner(KDE_WATCHER) {
        Some(KDE_WATCHER)
    } else if owner(FREEDESKTOP_WATCHER) {
        Some(FREEDESKTOP_WATCHER)
    } else {
        None
    };

    BusDiagnostics {
        available: true,
        application_running,
        extension_connection: Some(extension_connection),
        watcher,
        kwin_running: owner(KWIN_SERVICE),
    }
}

fn environment_value(value: Option<&std::ffi::OsStr>) -> Option<String> {
    value.map(|value| {
        value
            .to_string_lossy()
            .chars()
            .take(120)
            .map(|character| {
                if character.is_control() {
                    '\u{fffd}'
                } else {
                    character
                }
            })
            .collect()
    })
}

fn native_manifest_path(home: Option<&Path>) -> Option<PathBuf> {
    let file_name = format!("{NATIVE_HOST_NAME}.json");
    let user = home.map(|home| {
        home.join(".mozilla")
            .join("native-messaging-hosts")
            .join(&file_name)
    });
    user.as_ref()
        .filter(|path| path.is_file())
        .cloned()
        .or_else(|| {
            ["/usr/lib", "/usr/lib64"]
                .into_iter()
                .map(|root| {
                    Path::new(root)
                        .join("mozilla")
                        .join("native-messaging-hosts")
                        .join(&file_name)
                })
                .find(|path| path.is_file())
        })
        .or(user)
}

fn executable_available(command: &str) -> bool {
    if command.contains('/') {
        return is_executable(Path::new(command));
    }
    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|directory| is_executable(&directory.join(command)))
    })
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn redacted_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if let Ok(relative) = path.strip_prefix(home) {
            return if relative.as_os_str().is_empty() {
                "~".to_owned()
            } else {
                format!("~/{}", relative.display())
            };
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn paths_under_home_are_redacted() {
        assert_eq!(
            redacted_path(
                Path::new("/home/alice/.config/thunderbird-tray/config.toml"),
                Some(Path::new("/home/alice"))
            ),
            "~/.config/thunderbird-tray/config.toml"
        );
        assert_eq!(
            redacted_path(Path::new("/etc/thunderbird-tray.toml"), None),
            "/etc/thunderbird-tray.toml"
        );
    }

    #[test]
    fn environment_values_are_single_line_and_bounded() {
        let value = std::ffi::OsStr::new("GNOME\nmalicious");
        let rendered = environment_value(Some(value)).unwrap();
        assert!(!rendered.contains('\n'));
        assert!(rendered.chars().count() <= 120);
        assert_eq!(environment_value(None), None);
    }

    #[test]
    fn report_is_localized_actionable_and_excludes_mail_values() {
        let report = DoctorReport {
            config_path: PathBuf::from("/home/alice/.config/thunderbird-tray/config.toml"),
            config_exists: false,
            requested_backend: WindowBackend::X11,
            selected_backend: WindowBackend::None,
            selection_reason: SelectionReason::RequestedBackendUnavailable(WindowBackend::X11),
            language_mode: LanguageMode::Ru,
            resolved_language: Language::Russian,
            session_type: Some("x11".to_owned()),
            desktop: Some("KDE".to_owned()),
            session_bus: true,
            application_running: true,
            extension_connection: ExtensionConnection::Connected,
            watcher: None,
            kwin_running: false,
            x11_feature: false,
            native_manifest_path: Some(PathBuf::from(
                "/home/alice/.mozilla/native-messaging-hosts/host.json",
            )),
            native_manifest_exists: false,
            thunderbird_executable: false,
            home: Some(PathBuf::from("/home/alice")),
        };
        let rendered = report.render(Localizer::new(Language::Russian));

        assert!(rendered.contains("Выбранный оконный бэкенд: none"));
        assert!(rendered.contains("Соединение с расширением Thunderbird: подключено"));
        assert!(rendered.contains("--features x11"));
        assert!(rendered.contains("~/.mozilla/native-messaging-hosts/host.json"));
        assert!(!rendered.contains("/home/alice"));
        assert!(!rendered.contains("unread"));
        assert!(!rendered.contains("account"));
    }
}
