// SPDX-License-Identifier: GPL-3.0-only

//! Application and Native Messaging host entry point.

use std::io::{self, Write};

use anyhow::{Result, anyhow};
use thunderbird_tray::cli::{Cli, Command};
use thunderbird_tray::config::{ConfigSource, LanguageMode};
use thunderbird_tray::doctor::DoctorReport;
use thunderbird_tray::i18n::{LocaleEnvironment, Localizer, resolve_language};
use thunderbird_tray::run_host;

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

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    run_host(&mut input, &mut output).map_err(|error| anyhow!(localizer.native_host_error(&error)))
}

fn write_output(contents: &str) -> Result<()> {
    io::stdout()
        .lock()
        .write_all(contents.as_bytes())
        .map_err(|error| anyhow!("could not write command output: {error}"))
}
