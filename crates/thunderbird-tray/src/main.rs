// SPDX-License-Identifier: GPL-3.0-only

//! Application and Native Messaging host entry point.

use std::io;

use anyhow::{Context, Result, anyhow};
use thunderbird_tray::run_host;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_ansi(false)
        .with_target(false)
        .try_init()
        .map_err(|error| anyhow!("could not initialize stderr logging: {error}"))?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    run_host(&mut input, &mut output).context("native messaging host failed")
}
