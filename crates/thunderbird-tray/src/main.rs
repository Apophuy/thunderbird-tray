// SPDX-License-Identifier: GPL-3.0-only

//! Application and Native Messaging host entry point.

use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use thunderbird_tray::cli::{Cli, Command};
use thunderbird_tray::config::{Config, ConfigSource, LanguageMode};
use thunderbird_tray::core::TrayState;
use thunderbird_tray::doctor::DoctorReport;
use thunderbird_tray::i18n::{LocaleEnvironment, Localizer, resolve_language};
use thunderbird_tray::sni::SniService;
use thunderbird_tray::tray::{TrayAction, TrayModel};
use thunderbird_tray::{HostError, run_host_with_callbacks, write_message};
use thunderbird_tray_protocol::{Message, RequestFullStatePayload};
use tracing::{error, info, warn};

fn main() -> Result<()> {
    let locale_environment = LocaleEnvironment::from_process();
    let system_localizer =
        Localizer::new(resolve_language(LanguageMode::Auto, &locale_environment));
    let cli =
        Cli::parse_environment().map_err(|error| anyhow!(system_localizer.cli_error(&error)))?;
    let config_source = ConfigSource::discover(cli.config.clone())
        .map_err(|error| anyhow!(system_localizer.config_error(&error)))?;
    let config = config_source
        .load()
        .map_err(|error| anyhow!(system_localizer.config_error(&error)))?;
    let language = resolve_language(config.general.language, &locale_environment);
    let localizer = Localizer::new(language);

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
        Command::Run => {}
    }

    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_ansi(false)
        .with_target(false)
        .with_max_level(cli.log_level.tracing_level())
        .try_init()
        .map_err(|error| anyhow!("could not initialize stderr logging: {error}"))?;

    run_application(config_source, config, locale_environment, localizer)
}

enum WorkerEvent {
    StateChanged(TrayState),
    HostFinished(Result<(), HostError>),
    WriterFailed(HostError),
}

fn run_application(
    config_source: ConfigSource,
    mut config: Config,
    locale_environment: LocaleEnvironment,
    localizer: Localizer,
) -> Result<()> {
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

    let (outbound_tx, outbound_rx) = mpsc::channel::<Message>();
    let (worker_tx, worker_rx) = mpsc::channel::<WorkerEvent>();

    let writer_events = worker_tx.clone();
    thread::spawn(move || {
        let stdout = io::stdout();
        let mut output = stdout.lock();
        while let Ok(message) = outbound_rx.recv() {
            if let Err(source) = write_message(&mut output, &message) {
                let _ = writer_events.send(WorkerEvent::WriterFailed(source));
                break;
            }
        }
    });

    let host_output = outbound_tx.clone();
    let host_events = worker_tx.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let result = run_host_with_callbacks(
            &mut input,
            |message| {
                host_output
                    .send(message.clone())
                    .map_err(|_| HostError::OutputChannelClosed)
            },
            |state| {
                let _ = host_events.send(WorkerEvent::StateChanged(state.tray()));
            },
        );
        let _ = host_events.send(WorkerEvent::HostFinished(result));
    });
    drop(worker_tx);

    let mut current_state = TrayState::Disconnected;
    let mut worker_channel_open = true;
    loop {
        let worker_event = if worker_channel_open {
            worker_rx.recv_timeout(Duration::from_millis(100))
        } else {
            thread::sleep(Duration::from_millis(100));
            Err(mpsc::RecvTimeoutError::Timeout)
        };
        match worker_event {
            Ok(WorkerEvent::StateChanged(state)) => {
                current_state = state;
                if let Some(tray) = &tray {
                    if !tray.set_state(state) {
                        warn!("StatusNotifierItem service closed while updating state");
                    }
                }
            }
            Ok(WorkerEvent::HostFinished(result)) => {
                current_state = TrayState::Disconnected;
                if let Some(tray) = &tray {
                    let _ = tray.set_state(TrayState::Disconnected);
                } else {
                    return result.map_err(|error| anyhow!(localizer.native_host_error(&error)));
                }
                match result {
                    Ok(()) => info!("Thunderbird connection closed; tray remains disconnected"),
                    Err(source) => {
                        warn!(error = %source, "Thunderbird connection failed; tray remains disconnected");
                    }
                }
            }
            Ok(WorkerEvent::WriterFailed(source)) => {
                current_state = TrayState::Disconnected;
                if let Some(tray) = &tray {
                    let _ = tray.set_state(TrayState::Disconnected);
                    warn!(error = %source, "native messaging output failed; tray remains disconnected");
                } else {
                    return Err(anyhow!(localizer.native_host_error(&source)));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                worker_channel_open = false;
                if tray.is_none() {
                    return Err(anyhow!("application worker channels closed unexpectedly"));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        while let Ok(action) = tray_action_rx.try_recv() {
            match action {
                TrayAction::OpenThunderbird => {
                    warn!("Open Thunderbird is unavailable until a window backend is selected");
                }
                TrayAction::Refresh => {
                    if current_state != TrayState::Disconnected
                        && outbound_tx
                            .send(Message::RequestFullState(RequestFullStatePayload::default()))
                            .is_err()
                    {
                        warn!("could not request a refreshed Thunderbird state");
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
                    drop(outbound_tx);
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
