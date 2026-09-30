// SPDX-License-Identifier: GPL-3.0-only

//! Privacy-preserving diagnostic report foundation.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::config::{Config, LanguageMode, WindowBackend};
use crate::i18n::{Language, Localizer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorReport {
    config_path: PathBuf,
    config_exists: bool,
    backend: WindowBackend,
    language_mode: LanguageMode,
    resolved_language: Language,
    thunderbird_command: String,
}

impl DoctorReport {
    pub fn collect(
        config_path: &Path,
        config: &Config,
        backend_override: Option<WindowBackend>,
        resolved_language: Language,
    ) -> Self {
        Self {
            config_path: config_path.to_owned(),
            config_exists: config_path.is_file(),
            backend: backend_override.unwrap_or(config.window.backend),
            language_mode: config.general.language,
            resolved_language,
            thunderbird_command: config.thunderbird.command.clone(),
        }
    }

    pub fn render(&self, localizer: Localizer) -> String {
        let labels = localizer.doctor_labels();
        let exists = if self.config_exists {
            labels.yes
        } else {
            labels.no
        };
        let mut output = String::new();
        writeln!(output, "{}", localizer.doctor_title())
            .expect("writing to an in-memory String cannot fail");
        writeln!(output, "{}: {}", labels.config, self.config_path.display())
            .expect("writing to an in-memory String cannot fail");
        writeln!(output, "{}: {exists}", labels.config_exists)
            .expect("writing to an in-memory String cannot fail");
        writeln!(output, "{}: {}", labels.backend, self.backend.as_str())
            .expect("writing to an in-memory String cannot fail");
        writeln!(
            output,
            "{}: {}",
            labels.language_mode,
            self.language_mode.as_str()
        )
        .expect("writing to an in-memory String cannot fail");
        writeln!(
            output,
            "{}: {}",
            labels.resolved_language,
            self.resolved_language.as_str()
        )
        .expect("writing to an in-memory String cannot fail");
        writeln!(
            output,
            "{}: {}",
            labels.thunderbird_command, self.thunderbird_command
        )
        .expect("writing to an in-memory String cannot fail");
        output
    }
}
