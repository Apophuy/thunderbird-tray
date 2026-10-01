// SPDX-License-Identifier: GPL-3.0-only

//! Application and Native Messaging host entry point.

use std::io::{self, Write};
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use thunderbird_tray::cli::{Cli, Command};
use thunderbird_tray::config::{Config, ConfigSource, LanguageMode, WindowBackend};
use thunderbird_tray::core::TrayState;
use thunderbird_tray::doctor::DoctorReport;
use thunderbird_tray::i18n::{LocaleEnvironment, Localizer, resolve_language};
use thunderbird_tray::kde_wayland::KdeWindowControl;
use thunderbird_tray::lifecycle::{
    AttachedNativeStream, ClaimError, DETACH_SERVICE_ENVIRONMENT, LifecycleClient,
    LifecycleService, WindowActionReport,
};
use thunderbird_tray::native_manifest;
use thunderbird_tray::settings;
use thunderbird_tray::sni::SniService;
use thunderbird_tray::tray::{TrayAction, TrayModel};
use thunderbird_tray::window::{
    BackendAvailability, DesktopEnvironment, OpenOutcome, ProcessLauncher, ThunderbirdLauncher,
    UnsupportedWindowControl, WindowCapabilities, WindowControl, open_thunderbird, reap_children,
    select_backend,
};
use thunderbird_tray::x11::X11WindowControl;
use thunderbird_tray::{HostError, run_host, run_host_with_callbacks, write_message};
use thunderbird_tray_protocol::{Message, RequestFullStatePayload};
use tracing::{error, info, warn};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

fn main() -> Result<()> {
    let locale_environment = LocaleEnvironment::from_process();
    let system_localizer =
        Localizer::new(resolve_language(LanguageMode::Auto, &locale_environment));
    let cli =
        Cli::parse_environment().map_err(|error| anyhow!(system_localizer.cli_error(&error)))?;
    match cli.command {
        Command::Help => return write_output(system_localizer.help()),
        Command::Version => {
            return write_output(concat!(
                env!("CARGO_PKG_NAME"),
                " ",
                env!("CARGO_PKG_VERSION"),
                "\n"
            ));
        }
        Command::InstallNativeManifest => {
            let path = native_manifest::install_default()
                .map_err(|error| anyhow!(system_localizer.native_manifest_error(&error)))?;
            return write_output(&system_localizer.native_manifest_installed(&path));
        }
        Command::UninstallNativeManifest => {
            let (path, removed) = native_manifest::remove_default()
                .map_err(|error| anyhow!(system_localizer.native_manifest_error(&error)))?;
            return write_output(&system_localizer.native_manifest_removed(&path, removed));
        }
        Command::Run | Command::Doctor | Command::Settings | Command::Service => {}
    }
    if cli.command == Command::Service
        && std::env::var_os(DETACH_SERVICE_ENVIRONMENT).as_deref()
            == Some(std::ffi::OsStr::new("1"))
    {
        rustix::process::setsid()
            .map_err(|error| anyhow!(system_localizer.lifecycle_error(&error)))?;
    }
    let config_source = ConfigSource::discover(cli.config.clone())
        .map_err(|error| anyhow!(system_localizer.config_error(&error)))?;
    let config = config_source
        .load()
        .map_err(|error| anyhow!(system_localizer.config_error(&error)))?;
    let language = resolve_language(config.general.language, &locale_environment);
    let localizer = Localizer::new(language);
    let requested_window_backend = cli.window_backend.unwrap_or(config.window.backend);

    if cli.command == Command::Doctor {
        let report =
            DoctorReport::collect(config_source.path(), &config, cli.window_backend, language);
        return write_output(&report.render(localizer));
    }
    if cli.command == Command::Settings {
        let automatic_language = resolve_language(LanguageMode::Auto, &locale_environment);
        return settings::run(config_source, config, automatic_language).map_err(Into::into);
    }

    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_ansi(false)
        .with_target(false)
        .with_max_level(cli.log_level.tracing_level())
        .try_init()
        .map_err(|error| anyhow!("could not initialize stderr logging: {error}"))?;

    if cli.native_launch.is_some() {
        return run_native_launcher(localizer);
    }

    let background_service = cli.command == Command::Service;
    let (attached_tx, attached_rx) = mpsc::channel();
    let (window_report_tx, window_report_rx) = mpsc::channel();
    let (configuration_tx, configuration_rx) = mpsc::channel();
    let lifecycle = match LifecycleService::claim(attached_tx, window_report_tx, configuration_tx) {
        Ok(service) => service,
        Err(ClaimError::AlreadyRunning) if background_service => return Ok(()),
        Err(ClaimError::AlreadyRunning) => return Err(anyhow!(localizer.already_running())),
        Err(source) => return Err(anyhow!(localizer.lifecycle_error(&source))),
    };

    run_application(
        config_source,
        config,
        locale_environment,
        ApplicationContext {
            lifecycle,
            attached_rx,
            window_report_rx,
            configuration_rx,
            requested_window_backend,
            background_service,
        },
    )
}

fn run_native_launcher(localizer: Localizer) -> Result<()> {
    let client = match LifecycleClient::connect() {
        Ok(client) => client,
        Err(source) => {
            warn!(error = %source, "session D-Bus unavailable; using direct native host fallback");
            return run_direct_native_host(localizer);
        }
    };

    client
        .ensure_service_running(None)
        .map_err(|error| anyhow!(localizer.lifecycle_error(&error)))?;

    client
        .attach_standard_streams()
        .map_err(|error| anyhow!(localizer.lifecycle_error(&error)))?;
    info!("forwarded Native Messaging stream to the primary process");
    Ok(())
}

fn run_direct_native_host(localizer: Localizer) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    run_host(&mut input, &mut output).map_err(|error| anyhow!(localizer.native_host_error(&error)))
}

enum WorkerEvent {
    StateChanged {
        session_id: u64,
        state: TrayState,
    },
    HostFinished {
        session_id: u64,
        result: Result<(), HostError>,
    },
    WriterFailed {
        session_id: u64,
        source: HostError,
    },
}

struct ActiveSession {
    id: u64,
    outbound: mpsc::Sender<Message>,
    _completion_guard: std::os::unix::net::UnixStream,
}

struct ApplicationContext {
    lifecycle: LifecycleService,
    attached_rx: mpsc::Receiver<AttachedNativeStream>,
    window_report_rx: mpsc::Receiver<WindowActionReport>,
    configuration_rx: mpsc::Receiver<()>,
    requested_window_backend: WindowBackend,
    background_service: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupBehavior {
    Disabled,
    Open,
    OpenHidden,
}

fn startup_behavior(
    background_service: bool,
    config: &Config,
    capabilities: WindowCapabilities,
) -> StartupBehavior {
    if !background_service || !config.general.start_thunderbird {
        StartupBehavior::Disabled
    } else if config.general.start_minimized && capabilities.detect && capabilities.hide {
        StartupBehavior::OpenHidden
    } else {
        StartupBehavior::Open
    }
}

fn start_host_session(
    stream: AttachedNativeStream,
    session_id: u64,
    worker_tx: &mpsc::Sender<WorkerEvent>,
) -> ActiveSession {
    let AttachedNativeStream {
        input,
        output,
        completion_guard,
    } = stream;
    let (outbound_tx, outbound_rx) = mpsc::channel::<Message>();
    let writer_events = worker_tx.clone();
    thread::spawn(move || {
        let mut output = output;
        while let Ok(message) = outbound_rx.recv() {
            if let Err(source) = write_message(&mut output, &message) {
                let _ = writer_events.send(WorkerEvent::WriterFailed { session_id, source });
                break;
            }
        }
    });

    let host_output = outbound_tx.clone();
    let host_events = worker_tx.clone();
    thread::spawn(move || {
        let mut input = input;
        let result = run_host_with_callbacks(
            &mut input,
            |message| {
                host_output
                    .send(message.clone())
                    .map_err(|_| HostError::OutputChannelClosed)
            },
            |state| {
                let _ = host_events.send(WorkerEvent::StateChanged {
                    session_id,
                    state: state.tray(),
                });
            },
        );
        let _ = host_events.send(WorkerEvent::HostFinished { session_id, result });
    });

    ActiveSession {
        id: session_id,
        outbound: outbound_tx,
        _completion_guard: completion_guard,
    }
}

fn run_application(
    config_source: ConfigSource,
    mut config: Config,
    locale_environment: LocaleEnvironment,
    context: ApplicationContext,
) -> Result<()> {
    let ApplicationContext {
        lifecycle,
        attached_rx,
        window_report_rx,
        configuration_rx,
        requested_window_backend,
        background_service,
    } = context;
    let desktop_environment = DesktopEnvironment::from_process();
    let kde_window_control = match KdeWindowControl::connect(&desktop_environment, window_report_rx)
    {
        Ok(backend) => backend,
        Err(source) => {
            warn!(error = %source, "KDE Wayland window-control probe failed");
            None
        }
    };
    let x11_window_control = match X11WindowControl::connect(&desktop_environment) {
        Ok(backend) => backend,
        Err(source) => {
            warn!(error = %source, "X11 window-control probe failed");
            None
        }
    };
    let backend_selection = select_backend(
        requested_window_backend,
        &desktop_environment,
        BackendAvailability {
            kde_wayland: kde_window_control.is_some(),
            x11: x11_window_control.is_some(),
        },
    );
    info!(
        requested = backend_selection.requested.as_str(),
        selected = backend_selection.selected.as_str(),
        reason = %backend_selection.reason,
        "selected window backend"
    );
    let window_control: Box<dyn WindowControl> =
        match (backend_selection.selected, kde_window_control) {
            (WindowBackend::KdeWayland, Some(backend)) => Box::new(backend),
            _ => match (backend_selection.selected, x11_window_control) {
                (WindowBackend::X11, Some(backend)) => Box::new(backend),
                _ => Box::new(UnsupportedWindowControl::new(backend_selection)),
            },
        };
    let launcher = ProcessLauncher;
    let mut launcher_children = Vec::new();

    let automatic_language = resolve_language(LanguageMode::Auto, &locale_environment);
    let mut model = TrayModel::new(
        TrayState::Disconnected,
        config.tray,
        config.general.language,
        automatic_language,
    );
    model.set_can_hide_thunderbird(
        backend_selection.selected == WindowBackend::KdeWayland
            && window_control.capabilities().hide,
    );
    let (tray_action_tx, tray_action_rx) = mpsc::channel();
    let tray = match SniService::spawn(model, tray_action_tx) {
        Ok(service) => Some(service),
        Err(source) => {
            warn!(error = %source, "could not register StatusNotifierItem");
            None
        }
    };

    let (worker_tx, worker_rx) = mpsc::channel::<WorkerEvent>();
    let mut next_session_id = 1_u64;
    let mut active_session: Option<ActiveSession> = None;
    let mut current_state = TrayState::Disconnected;
    let mut startup_hide_pending = false;

    match startup_behavior(background_service, &config, window_control.capabilities()) {
        StartupBehavior::Disabled => {}
        StartupBehavior::OpenHidden => match window_control.detect() {
            Ok(true) => match window_control.hide() {
                Ok(()) => info!("hid the existing Thunderbird window for session startup"),
                Err(source) => {
                    error!(error = %source, "could not hide Thunderbird for session startup")
                }
            },
            Ok(false) => match launcher.launch(&config.thunderbird) {
                Ok(child) => {
                    info!(
                        pid = child.id(),
                        "started Thunderbird for the login session"
                    );
                    startup_hide_pending = true;
                    launcher_children.push(child);
                }
                Err(source) => {
                    error!(error = %source, "could not start Thunderbird for the login session")
                }
            },
            Err(source) => {
                error!(error = %source, "could not detect Thunderbird for session startup")
            }
        },
        StartupBehavior::Open => {
            match open_thunderbird(window_control.as_ref(), &launcher, &config.thunderbird) {
                Ok(OpenOutcome::ActivatedExisting) => {
                    info!("activated Thunderbird for the login session")
                }
                Ok(OpenOutcome::Launched(child)) => {
                    info!(
                        pid = child.id(),
                        "started Thunderbird for the login session"
                    );
                    if config.general.start_minimized {
                        warn!("start minimized is unavailable with the selected window backend");
                    }
                    launcher_children.push(child);
                }
                Err(source) => {
                    error!(error = %source, "could not start Thunderbird for the login session")
                }
            }
        }
    }

    loop {
        reap_children(&mut launcher_children);

        while let Ok(stream) = attached_rx.try_recv() {
            if active_session.is_some() {
                warn!("ignoring an unexpected second Native Messaging stream");
                continue;
            }
            let session = start_host_session(stream, next_session_id, &worker_tx);
            info!(
                session_id = next_session_id,
                "attached Native Messaging stream"
            );
            next_session_id = next_session_id.wrapping_add(1);
            active_session = Some(session);
        }

        match worker_rx.recv_timeout(EVENT_POLL_INTERVAL) {
            Ok(WorkerEvent::StateChanged { session_id, state })
                if active_session
                    .as_ref()
                    .is_some_and(|session| session.id == session_id) =>
            {
                current_state = state;
                if startup_hide_pending {
                    match window_control.hide() {
                        Ok(()) => {
                            startup_hide_pending = false;
                            info!("hid the startup Thunderbird window in the tray");
                        }
                        Err(source) => {
                            warn!(error = %source, "could not yet hide the startup Thunderbird window");
                        }
                    }
                }
                if let Some(tray) = &tray {
                    if !tray.set_state(state) {
                        warn!("StatusNotifierItem service closed while updating state");
                    }
                }
            }
            Ok(WorkerEvent::HostFinished { session_id, result })
                if active_session
                    .as_ref()
                    .is_some_and(|session| session.id == session_id) =>
            {
                active_session = None;
                lifecycle.session_finished();
                current_state = TrayState::Disconnected;
                if let Some(tray) = &tray {
                    let _ = tray.set_state(TrayState::Disconnected);
                }
                match result {
                    Ok(()) => {
                        info!(
                            session_id,
                            "Thunderbird connection closed; awaiting reconnect"
                        );
                    }
                    Err(source) => {
                        warn!(session_id, error = %source, "Thunderbird connection failed; awaiting reconnect");
                    }
                }
            }
            Ok(WorkerEvent::WriterFailed { session_id, source })
                if active_session
                    .as_ref()
                    .is_some_and(|session| session.id == session_id) =>
            {
                current_state = TrayState::Disconnected;
                if let Some(tray) = &tray {
                    let _ = tray.set_state(TrayState::Disconnected);
                }
                warn!(session_id, error = %source, "native messaging output failed");
            }
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(anyhow!(
                    "Native Messaging worker channel closed unexpectedly"
                ));
            }
        }

        while let Ok(action) = tray_action_rx.try_recv() {
            match action {
                TrayAction::OpenThunderbird => {
                    match open_thunderbird(window_control.as_ref(), &launcher, &config.thunderbird)
                    {
                        Ok(OpenOutcome::ActivatedExisting) => {
                            info!("activated an existing Thunderbird window");
                        }
                        Ok(OpenOutcome::Launched(child)) => {
                            info!(pid = child.id(), "started Thunderbird");
                            launcher_children.push(child);
                        }
                        Err(source) => {
                            error!(error = %source, "could not open Thunderbird");
                        }
                    }
                }
                TrayAction::HideThunderbird => {
                    if let Err(source) = window_control.hide() {
                        error!(error = %source, "could not hide Thunderbird to tray");
                    } else {
                        info!("hid Thunderbird window from the KDE task manager");
                    }
                }
                TrayAction::OpenSettings => {
                    match std::env::current_exe().and_then(|executable| {
                        ProcessCommand::new(executable)
                            .arg("settings")
                            .arg("--config")
                            .arg(config_source.path())
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::inherit())
                            .spawn()
                    }) {
                        Ok(child) => launcher_children.push(child),
                        Err(source) => error!(error = %source, "could not open settings window"),
                    }
                }
                TrayAction::Refresh => {
                    if current_state != TrayState::Disconnected {
                        if let Some(session) = &active_session {
                            if session
                                .outbound
                                .send(Message::RequestFullState(RequestFullStatePayload::default()))
                                .is_err()
                            {
                                warn!("could not request a refreshed Thunderbird state");
                            }
                        }
                    }
                }
                TrayAction::SetLanguage(mode) => {
                    config.set_language(mode);
                    if let Err(source) = config_source.save(&config) {
                        error!(error = %source, "could not persist tray language preference");
                    } else {
                        info!(
                            language_mode = mode.as_str(),
                            "updated tray language preference"
                        );
                    }
                    if let Some(tray) = &tray {
                        let _ = tray.set_language_mode(mode);
                    }
                }
                TrayAction::Quit => {
                    if let Some(tray) = &tray {
                        tray.shutdown();
                    }
                    return Ok(());
                }
            }
        }

        while configuration_rx.try_recv().is_ok() {
            match config_source.load() {
                Ok(updated) => {
                    config = updated;
                    if let Some(tray) = &tray {
                        let _ = tray.set_configuration(config.tray, config.general.language);
                    }
                    info!("reloaded application settings");
                }
                Err(source) => error!(error = %source, "could not reload application settings"),
            }
        }
    }
}

fn write_output(contents: &str) -> Result<()> {
    io::stdout()
        .lock()
        .write_all(contents.as_bytes())
        .map_err(|error| anyhow!("could not write command output: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_is_opt_in_and_only_hides_with_required_capabilities() {
        let mut config = Config::default();
        let capable = WindowCapabilities {
            detect: true,
            hide: true,
            ..WindowCapabilities::default()
        };

        assert_eq!(
            startup_behavior(true, &config, capable),
            StartupBehavior::Disabled
        );
        config.general.start_thunderbird = true;
        assert_eq!(
            startup_behavior(false, &config, capable),
            StartupBehavior::Disabled
        );
        assert_eq!(
            startup_behavior(true, &config, capable),
            StartupBehavior::Open
        );

        config.general.start_minimized = true;
        assert_eq!(
            startup_behavior(true, &config, WindowCapabilities::default()),
            StartupBehavior::Open
        );
        assert_eq!(
            startup_behavior(true, &config, capable),
            StartupBehavior::OpenHidden
        );
    }
}
