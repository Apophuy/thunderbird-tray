// SPDX-License-Identifier: GPL-3.0-only

use std::cell::Cell;
use std::ffi::{OsStr, OsString};

use thunderbird_tray::config::{ThunderbirdConfig, WindowBackend};
use thunderbird_tray::window::{
    ActivationOutcome, BackendAvailability, DesktopEnvironment, LaunchError, OpenOutcome,
    ProcessLauncher, SelectionReason, ThunderbirdLauncher, ToggleThunderbirdOutcome,
    ToggleVisibilityOutcome, UnsupportedWindowControl, WindowCapabilities, WindowControl,
    WindowError, WindowOperation, open_thunderbird, select_backend, toggle_thunderbird,
};

fn environment(values: &[(&str, &str)]) -> DesktopEnvironment {
    DesktopEnvironment::from_lookup(|name| {
        values
            .iter()
            .find_map(|(key, value)| (*key == name).then(|| OsString::from(value)))
    })
}

#[test]
fn backend_selection_combines_explicit_choice_hints_and_availability() {
    let plasma = environment(&[
        ("XDG_SESSION_TYPE", "wayland"),
        ("XDG_CURRENT_DESKTOP", "KDE"),
        ("WAYLAND_DISPLAY", "wayland-0"),
    ]);
    assert!(plasma.is_kde_wayland());
    assert_eq!(
        select_backend(
            WindowBackend::Auto,
            &plasma,
            BackendAvailability {
                kde_wayland: true,
                x11: false,
            },
        )
        .selected,
        WindowBackend::KdeWayland
    );

    let unavailable = select_backend(
        WindowBackend::KdeWayland,
        &plasma,
        BackendAvailability::default(),
    );
    assert_eq!(unavailable.selected, WindowBackend::None);
    assert_eq!(
        unavailable.reason,
        SelectionReason::RequestedBackendUnavailable(WindowBackend::KdeWayland)
    );

    let x11 = environment(&[("XDG_SESSION_TYPE", "x11"), ("DISPLAY", ":0")]);
    assert!(x11.is_x11());
    assert_eq!(
        select_backend(
            WindowBackend::Auto,
            &x11,
            BackendAvailability {
                kde_wayland: false,
                x11: true,
            },
        )
        .selected,
        WindowBackend::X11
    );
}

#[test]
fn unsupported_wayland_desktops_never_inherit_kde_or_x11_capabilities() {
    for desktop in ["GNOME", "sway", "Hyprland", "wlroots"] {
        let environment = environment(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", desktop),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DISPLAY", ":1"),
        ]);
        let selection = select_backend(
            WindowBackend::Auto,
            &environment,
            BackendAvailability {
                kde_wayland: true,
                x11: true,
            },
        );
        assert_eq!(selection.selected, WindowBackend::None, "{desktop}");
        assert_eq!(selection.reason, SelectionReason::NoSupportedDesktop);
    }
}

#[test]
fn unsupported_backend_reports_each_missing_capability() {
    let selection = select_backend(
        WindowBackend::None,
        &DesktopEnvironment::default(),
        BackendAvailability::default(),
    );
    let backend = UnsupportedWindowControl::new(selection);

    assert_eq!(backend.backend(), WindowBackend::None);
    assert_eq!(backend.capabilities(), WindowCapabilities::default());
    assert!(matches!(
        backend.detect(),
        Err(WindowError::Unsupported {
            operation: WindowOperation::Detect,
            ..
        })
    ));
    assert!(matches!(
        backend.activate(),
        Err(WindowError::Unsupported {
            operation: WindowOperation::Activate,
            ..
        })
    ));
    assert!(matches!(
        backend.hide(),
        Err(WindowError::Unsupported {
            operation: WindowOperation::Hide,
            ..
        })
    ));
    assert!(matches!(
        backend.show(),
        Err(WindowError::Unsupported {
            operation: WindowOperation::Show,
            ..
        })
    ));
    assert!(matches!(
        backend.toggle_visibility(),
        Err(WindowError::Unsupported {
            operation: WindowOperation::Toggle,
            ..
        })
    ));
}

struct MockBackend {
    capabilities: WindowCapabilities,
    activation: ActivationOutcome,
    toggle: ToggleVisibilityOutcome,
}

impl WindowControl for MockBackend {
    fn backend(&self) -> WindowBackend {
        WindowBackend::None
    }

    fn capabilities(&self) -> WindowCapabilities {
        self.capabilities
    }

    fn detect(&self) -> Result<bool, WindowError> {
        Ok(true)
    }

    fn activate(&self) -> Result<ActivationOutcome, WindowError> {
        Ok(self.activation)
    }

    fn hide(&self) -> Result<(), WindowError> {
        Ok(())
    }

    fn show(&self) -> Result<(), WindowError> {
        Ok(())
    }

    fn toggle_visibility(&self) -> Result<ToggleVisibilityOutcome, WindowError> {
        Ok(self.toggle)
    }
}

#[derive(Default)]
struct MockLauncher {
    launches: Cell<usize>,
}

impl ThunderbirdLauncher for MockLauncher {
    type Handle = usize;

    fn launch(&self, _config: &ThunderbirdConfig) -> Result<Self::Handle, LaunchError> {
        let launch = self.launches.get() + 1;
        self.launches.set(launch);
        Ok(launch)
    }
}

#[test]
fn open_activates_when_possible_and_launches_when_no_window_control_exists() {
    let config = ThunderbirdConfig::default();
    let launcher = MockLauncher::default();
    let capable = MockBackend {
        capabilities: WindowCapabilities {
            activate: true,
            ..WindowCapabilities::default()
        },
        activation: ActivationOutcome::Activated,
        toggle: ToggleVisibilityOutcome::Shown,
    };
    assert!(matches!(
        open_thunderbird(&capable, &launcher, &config).unwrap(),
        OpenOutcome::ActivatedExisting
    ));
    assert_eq!(launcher.launches.get(), 0);

    let no_window = MockBackend {
        capabilities: capable.capabilities,
        activation: ActivationOutcome::NoWindow,
        toggle: ToggleVisibilityOutcome::NoWindow,
    };
    assert!(matches!(
        open_thunderbird(&no_window, &launcher, &config).unwrap(),
        OpenOutcome::Launched(1)
    ));

    let selection = select_backend(
        WindowBackend::None,
        &DesktopEnvironment::default(),
        BackendAvailability::default(),
    );
    let unsupported = UnsupportedWindowControl::new(selection);
    assert!(matches!(
        open_thunderbird(&unsupported, &launcher, &config).unwrap(),
        OpenOutcome::Launched(2)
    ));
    assert_eq!(launcher.launches.get(), 2);
}

#[test]
fn double_activation_toggles_a_window_or_launches_when_none_exists() {
    let config = ThunderbirdConfig::default();
    let launcher = MockLauncher::default();
    let capabilities = WindowCapabilities {
        activate: true,
        hide: true,
        show: true,
        ..WindowCapabilities::default()
    };

    let visible = MockBackend {
        capabilities,
        activation: ActivationOutcome::Activated,
        toggle: ToggleVisibilityOutcome::Hidden,
    };
    assert!(matches!(
        toggle_thunderbird(&visible, &launcher, &config).unwrap(),
        ToggleThunderbirdOutcome::Hidden
    ));

    let hidden = MockBackend {
        capabilities,
        activation: ActivationOutcome::Activated,
        toggle: ToggleVisibilityOutcome::Shown,
    };
    assert!(matches!(
        toggle_thunderbird(&hidden, &launcher, &config).unwrap(),
        ToggleThunderbirdOutcome::Shown
    ));

    let absent = MockBackend {
        capabilities,
        activation: ActivationOutcome::NoWindow,
        toggle: ToggleVisibilityOutcome::NoWindow,
    };
    assert!(matches!(
        toggle_thunderbird(&absent, &launcher, &config).unwrap(),
        ToggleThunderbirdOutcome::Launched(1)
    ));
    assert_eq!(launcher.launches.get(), 1);
}

#[test]
fn process_launcher_preserves_program_and_argument_boundaries_without_a_shell() {
    let config = ThunderbirdConfig {
        command: "/opt/Thunderbird App/thunderbird".to_owned(),
        arguments: vec![
            "--profile".to_owned(),
            "Personal; touch /tmp/not-a-command".to_owned(),
        ],
    };
    let command = ProcessLauncher::command(&config);
    assert_eq!(
        command.get_program(),
        OsStr::new("/opt/Thunderbird App/thunderbird")
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        vec![
            OsStr::new("--profile"),
            OsStr::new("Personal; touch /tmp/not-a-command")
        ]
    );
}

#[test]
fn process_launcher_returns_a_typed_spawn_failure() {
    let config = ThunderbirdConfig {
        command: "/definitely/missing/thunderbird-tray-test".to_owned(),
        arguments: Vec::new(),
    };
    assert!(matches!(
        ProcessLauncher.launch(&config),
        Err(LaunchError::Spawn { .. })
    ));
}
