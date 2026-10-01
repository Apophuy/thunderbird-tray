// SPDX-License-Identifier: GPL-3.0-only

//! Application and Native Messaging host entry point.

use std::io::{self, Write};
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use thunderbird_tray::cli::{Cli, Command};
use thunderbird_tray::config::{Config, ConfigSource, LanguageMode, WindowBackend};
use thunderbird_tray::core::TrayState;
use thunderbird_tray::doctor::DoctorReport;
use thunderbird_tray::i18n::{LocaleEnvironment, Localizer, resolve_language};
use thunderbird_tray::kde_wayland::KdeWindowControl;
use thunderbird_tray::lifecycle::{
    AttachedNativeStream, ClaimError, LifecycleClient, LifecycleService, WindowActionReport,
};
use thunderbird_tray::sni::SniService;
use thunderbird_tray::tray::{TrayAction, TrayModel};
use thunderbird_tray::window::{
    BackendAvailability, DesktopEnvironment, OpenOutcome, ProcessLauncher,
    UnsupportedWindowControl, WindowControl, open_thunderbird, reap_children, select_backend,
};
use thunderbird_tray::{HostError, run_host, run_host_with_callbacks, write_message};
use thunderbird_tray_protocol::{Message, RequestFullStatePayload};
use tracing::{error, info, warn};

const SERVICE_START_TIMEOUT: Duration = Duration::from_secs(3);
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DETACH_SERVICE_ENVIRONMENT: &str = "THUNDERBIRD_TRAY_DETACH_SERVICE";

fn main() -> Result<()> {
    let locale_environment = LocaleEnvironment::from_process();
    let system_localizer =
        Localizer::new(resolve_language(LanguageMode::Auto, &locale_environment));
    let cli =
        Cli::parse_environment().map_err(|error| anyhow!(system_localizer.cli_error(&error)))?;
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

    match cli.command {
        Command::Help => return write_output(localizer.help()),
        Command::Version => {
            return write_output(concat!(
                env!("CARGO_PKG_NAME"),
                " ",
                env!("CARGO_PKG_VERSION"),
                "\n"
            ));
        }
        Command::Doctor => {
            let report =
                DoctorReport::collect(config_source.path(), &config, cli.window_backend, language);
            return write_output(&report.render(localizer));
        }
        Command::Run | Command::Service => {}
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
    let lifecycle = match LifecycleService::claim(attached_tx, window_report_tx) {
        Ok(service) => service,
        Err(ClaimError::AlreadyRunning) if background_service => return Ok(()),
        Err(ClaimError::AlreadyRunning) => return Err(anyhow!(localizer.already_running())),
        Err(source) => return Err(anyhow!(localizer.lifecycle_error(&source))),
    };

    run_application(
        config_source,
        config,
        locale_environment,
        lifecycle,
        attached_rx,
        window_report_rx,
        requested_window_backend,
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

    if !client
        .service_is_running()
        .map_err(|error| anyhow!(localizer.lifecycle_error(&error)))?
    {
        let executable =
            std::env::current_exe().map_err(|error| anyhow!(localizer.lifecycle_error(&error)))?;
        ProcessCommand::new(executable)
            .arg("--service")
            .env(DETACH_SERVICE_ENVIRONMENT, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| anyhow!(localizer.lifecycle_error(&error)))?;

        let deadline = Instant::now() + SERVICE_START_TIMEOUT;
        while Instant::now() < deadline {
            if client
                .service_is_running()
                .map_err(|error| anyhow!(localizer.lifecycle_error(&error)))?
            {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        if !client
            .service_is_running()
            .map_err(|error| anyhow!(localizer.lifecycle_error(&error)))?
        {
            return Err(anyhow!(localizer.service_start_timeout()));
        }
    }

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
    lifecycle: LifecycleService,
    attached_rx: mpsc::Receiver<AttachedNativeStream>,
    window_report_rx: mpsc::Receiver<WindowActionReport>,
    requested_window_backend: WindowBackend,
) -> Result<()> {
    let desktop_environment = DesktopEnvironment::from_process();
    let kde_window_control = match KdeWindowControl::connect(&desktop_environment, window_report_rx)
    {
        Ok(backend) => backend,
        Err(source) => {
            warn!(error = %source, "KDE Wayland window-control probe failed");
            None
        }
    };
    let backend_selection = select_backend(
        requested_window_backend,
        &desktop_environment,
        BackendAvailability {
            kde_wayland: kde_window_control.is_some(),
            x11: false,
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
            _ => Box::new(UnsupportedWindowControl::new(backend_selection)),
        };
    let launcher = ProcessLauncher;
    let mut launcher_children = Vec::new();

    let automatic_language = resolve_language(LanguageMode::Auto, &locale_environment);
    let model = TrayModel::new(
        TrayState::Disconnected,
        config.tray,
        config.general.language,
        automatic_language,
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
    }
}

fn write_output(contents: &str) -> Result<()> {
    io::stdout()
        .lock()
        .write_all(contents.as_bytes())
        .map_err(|error| anyhow!("could not write command output: {error}"))
}
