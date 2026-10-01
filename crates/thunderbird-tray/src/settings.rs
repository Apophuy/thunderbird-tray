// SPDX-License-Identifier: GPL-3.0-only

//! Native settings window backed by the same strict TOML configuration.

use std::cell::Cell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};
use thiserror::Error;

use crate::autostart::{AutostartEntry, AutostartError};
use crate::config::{Config, ConfigError, ConfigSource, LanguageMode, ThemeMode, WindowBackend};
use crate::i18n::Language;
use crate::lifecycle::LifecycleClient;

slint::slint! {
    import { Button, CheckBox, ComboBox, LineEdit, Palette, TabWidget } from "std-widgets.slint";

    export component SettingsWindow inherits Window {
        title: root.window-title;
        icon: root.app-icon;
        preferred-width: 680px;
        preferred-height: 540px;
        min-width: 560px;
        min-height: 460px;

        in property <string> window-title;
        in property <image> app-icon;
        in property <string> general-tab;
        in property <string> tray-tab;
        in property <string> thunderbird-tab;
        in property <string> autostart-label;
        in property <string> start-thunderbird-label;
        in property <string> start-minimized-label;
        in property <string> notifications-label;
        in property <string> language-label;
        in property <string> theme-label;
        in property <string> unread-count-label;
        in property <string> hide-zero-label;
        in property <string> close-limit-title;
        in property <string> close-limit-text;
        in property <string> command-label;
        in property <string> backend-label;
        in property <string> apply-label;
        in property <string> cancel-label;
        in property <[string]> language-options;
        in property <[string]> theme-options;
        in property <[string]> backend-options;

        in-out property <bool> autostart-enabled;
        in-out property <bool> start-thunderbird;
        in-out property <bool> start-minimized;
        in-out property <bool> notifications-enabled;
        in-out property <bool> show-unread-count;
        in-out property <bool> hide-when-zero;
        in-out property <int> language-index;
        in-out property <int> theme-index;
        in-out property <int> backend-index;
        in-out property <string> thunderbird-command;
        in-out property <string> status-message;

        callback apply();
        callback cancel();

        changed theme-index => {
            Palette.color-scheme = root.theme-index == 1 ? ColorScheme.light
                : root.theme-index == 2 ? ColorScheme.dark
                : ColorScheme.unknown;
        }

        VerticalLayout {
            padding: 18px;
            spacing: 14px;

            TabWidget {
                vertical-stretch: 1;

                Tab {
                    title: root.general-tab;
                    VerticalLayout {
                        padding: 20px;
                        spacing: 14px;
                        CheckBox {
                            text: root.autostart-label;
                            checked <=> root.autostart-enabled;
                        }
                        CheckBox {
                            text: root.start-thunderbird-label;
                            checked <=> root.start-thunderbird;
                        }
                        CheckBox {
                            text: root.start-minimized-label;
                            enabled: root.start-thunderbird;
                            checked <=> root.start-minimized;
                        }
                        CheckBox {
                            text: root.notifications-label;
                            checked <=> root.notifications-enabled;
                        }
                        Text { text: root.language-label; }
                        ComboBox {
                            model: root.language-options;
                            current-index <=> root.language-index;
                        }
                        Text { text: root.theme-label; }
                        ComboBox {
                            model: root.theme-options;
                            current-index <=> root.theme-index;
                        }
                        Rectangle { vertical-stretch: 1; }
                    }
                }

                Tab {
                    title: root.tray-tab;
                    VerticalLayout {
                        padding: 20px;
                        spacing: 14px;
                        CheckBox {
                            text: root.unread-count-label;
                            checked <=> root.show-unread-count;
                        }
                        CheckBox {
                            text: root.hide-zero-label;
                            checked <=> root.hide-when-zero;
                        }
                        Text {
                            text: root.close-limit-title;
                            font-weight: 700;
                        }
                        Text {
                            text: root.close-limit-text;
                            wrap: word-wrap;
                        }
                        Rectangle { vertical-stretch: 1; }
                    }
                }

                Tab {
                    title: root.thunderbird-tab;
                    VerticalLayout {
                        padding: 20px;
                        spacing: 12px;
                        Text { text: root.command-label; }
                        LineEdit { text <=> root.thunderbird-command; }
                        Text { text: root.backend-label; }
                        ComboBox {
                            model: root.backend-options;
                            current-index <=> root.backend-index;
                        }
                        Rectangle { vertical-stretch: 1; }
                    }
                }
            }

            Text {
                text: root.status-message;
                color: #b91c1c;
                wrap: word-wrap;
                visible: root.status-message != "";
            }

            HorizontalLayout {
                alignment: end;
                spacing: 10px;
                Button {
                    text: root.cancel-label;
                    clicked => { root.cancel(); }
                }
                Button {
                    text: root.apply-label;
                    primary: true;
                    clicked => { root.apply(); }
                }
            }
        }
    }
}

pub fn run(
    config_source: ConfigSource,
    config: Config,
    language: Language,
) -> Result<(), SettingsError> {
    let autostart = AutostartEntry::discover()?;
    let strings = SettingsStrings::new(language);
    let window = SettingsWindow::new()?;

    if let Some(icon) = crate::tray_icons::application_image() {
        window.set_app_icon(icon);
    }

    window.set_window_title(strings.window_title.into());
    window.set_general_tab(strings.general_tab.into());
    window.set_tray_tab(strings.tray_tab.into());
    window.set_thunderbird_tab(strings.thunderbird_tab.into());
    window.set_autostart_label(strings.autostart.into());
    window.set_start_thunderbird_label(strings.start_thunderbird.into());
    window.set_start_minimized_label(strings.start_minimized.into());
    window.set_notifications_label(strings.notifications.into());
    window.set_language_label(strings.language.into());
    window.set_theme_label(strings.theme.into());
    window.set_unread_count_label(strings.unread_count.into());
    window.set_hide_zero_label(strings.hide_zero.into());
    window.set_close_limit_title(strings.close_limit_title.into());
    window.set_close_limit_text(strings.close_limit_text.into());
    window.set_command_label(strings.command.into());
    window.set_backend_label(strings.backend.into());
    window.set_apply_label(strings.apply.into());
    window.set_cancel_label(strings.cancel.into());
    window.set_language_options(
        Rc::new(slint::VecModel::from(vec![
            SharedString::from(strings.automatic),
            SharedString::from("English"),
            SharedString::from("Русский"),
        ]))
        .into(),
    );
    window.set_theme_options(
        Rc::new(slint::VecModel::from(vec![
            SharedString::from(strings.theme_system),
            SharedString::from(strings.theme_light),
            SharedString::from(strings.theme_dark),
        ]))
        .into(),
    );
    window.set_backend_options(
        Rc::new(slint::VecModel::from(vec![
            SharedString::from(strings.backend_auto),
            SharedString::from("KDE Plasma Wayland"),
            SharedString::from("X11"),
            SharedString::from(strings.backend_none),
        ]))
        .into(),
    );

    window.set_autostart_enabled(autostart.is_enabled());
    window.set_start_thunderbird(config.general.start_thunderbird);
    window.set_start_minimized(config.general.start_minimized);
    window.set_notifications_enabled(config.general.notifications);
    window.set_show_unread_count(config.tray.show_unread_count);
    window.set_hide_when_zero(config.tray.hide_when_zero);
    window.set_language_index(language_index(config.general.language));
    window.set_theme_index(theme_index(config.general.theme));
    window.set_backend_index(backend_index(config.window.backend));
    window.set_thunderbird_command(config.thunderbird.command.clone().into());

    let cancelled = Rc::new(Cell::new(false));
    let cancel_flag = Rc::clone(&cancelled);
    window.on_cancel(move || {
        cancel_flag.set(true);
        let _ = slint::quit_event_loop();
    });

    let weak = window.as_weak();
    let saved_config = config.clone();
    window.on_apply(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut updated = saved_config.clone();
        updated.general.start_thunderbird = window.get_start_thunderbird();
        updated.general.start_minimized =
            window.get_start_thunderbird() && window.get_start_minimized();
        updated.general.notifications = window.get_notifications_enabled();
        updated.general.language = language_from_index(window.get_language_index());
        updated.general.theme = theme_from_index(window.get_theme_index());
        updated.tray.show_unread_count = window.get_show_unread_count();
        updated.tray.hide_when_zero = window.get_hide_when_zero();
        updated.thunderbird.command = window.get_thunderbird_command().to_string();
        updated.window.backend = backend_from_index(window.get_backend_index());

        let previous_autostart = autostart.is_enabled();
        let requested_autostart = window.get_autostart_enabled();
        let result = autostart
            .set_enabled(requested_autostart)
            .map_err(SettingsError::Autostart)
            .and_then(|()| config_source.save(&updated).map_err(SettingsError::Config));
        if let Err(error) = result {
            if previous_autostart != requested_autostart {
                let _ = autostart.set_enabled(previous_autostart);
            }
            window.set_status_message(format!("{}: {error}", strings.save_failed).into());
            return;
        }

        if let Ok(client) = LifecycleClient::connect() {
            if client.service_is_running().unwrap_or(false) {
                let _ = client.notify_configuration_changed();
            }
        }
        let _ = slint::quit_event_loop();
    });

    window.run()?;
    let _ = cancelled;
    Ok(())
}

fn language_index(language: LanguageMode) -> i32 {
    match language {
        LanguageMode::Auto => 0,
        LanguageMode::En => 1,
        LanguageMode::Ru => 2,
    }
}

fn language_from_index(index: i32) -> LanguageMode {
    match index {
        1 => LanguageMode::En,
        2 => LanguageMode::Ru,
        _ => LanguageMode::Auto,
    }
}

fn theme_index(theme: ThemeMode) -> i32 {
    match theme {
        ThemeMode::System => 0,
        ThemeMode::Light => 1,
        ThemeMode::Dark => 2,
    }
}

fn theme_from_index(index: i32) -> ThemeMode {
    match index {
        1 => ThemeMode::Light,
        2 => ThemeMode::Dark,
        _ => ThemeMode::System,
    }
}

fn backend_index(backend: WindowBackend) -> i32 {
    match backend {
        WindowBackend::Auto => 0,
        WindowBackend::KdeWayland => 1,
        WindowBackend::X11 => 2,
        WindowBackend::None => 3,
    }
}

fn backend_from_index(index: i32) -> WindowBackend {
    match index {
        1 => WindowBackend::KdeWayland,
        2 => WindowBackend::X11,
        3 => WindowBackend::None,
        _ => WindowBackend::Auto,
    }
}

#[derive(Clone, Copy)]
struct SettingsStrings {
    window_title: &'static str,
    general_tab: &'static str,
    tray_tab: &'static str,
    thunderbird_tab: &'static str,
    autostart: &'static str,
    start_thunderbird: &'static str,
    start_minimized: &'static str,
    notifications: &'static str,
    language: &'static str,
    theme: &'static str,
    unread_count: &'static str,
    hide_zero: &'static str,
    close_limit_title: &'static str,
    close_limit_text: &'static str,
    command: &'static str,
    backend: &'static str,
    apply: &'static str,
    cancel: &'static str,
    automatic: &'static str,
    theme_system: &'static str,
    theme_light: &'static str,
    theme_dark: &'static str,
    backend_auto: &'static str,
    backend_none: &'static str,
    save_failed: &'static str,
}

impl SettingsStrings {
    fn new(language: Language) -> Self {
        match language {
            Language::English => Self {
                window_title: "thunderbird-tray Settings",
                general_tab: "General",
                tray_tab: "Tray",
                thunderbird_tab: "Thunderbird",
                autostart: "Start thunderbird-tray when I sign in",
                start_thunderbird: "Start Thunderbird with thunderbird-tray",
                start_minimized: "Start Thunderbird hidden in the tray",
                notifications: "Enable notifications",
                language: "Interface language",
                theme: "Appearance",
                unread_count: "Show the unread count on the tray badge",
                hide_zero: "Allow Plasma to hide the tray icon when there are no unread messages",
                close_limit_title: "Close button behavior",
                close_limit_text: "Thunderbird 156 and KWin 6 do not expose a supported pre-close hook on Linux. Use Hide Thunderbird to tray; the window is removed from the KDE task manager while mail monitoring continues.",
                command: "Thunderbird executable",
                backend: "Window-control backend",
                apply: "Apply",
                cancel: "Cancel",
                automatic: "Automatic",
                theme_system: "System",
                theme_light: "Light",
                theme_dark: "Dark",
                backend_auto: "Automatic",
                backend_none: "None",
                save_failed: "Could not save settings",
            },
            Language::Russian => Self {
                window_title: "Настройки thunderbird-tray",
                general_tab: "Общие",
                tray_tab: "Трей",
                thunderbird_tab: "Thunderbird",
                autostart: "Запускать thunderbird-tray при входе в систему",
                start_thunderbird: "Запускать Thunderbird вместе с thunderbird-tray",
                start_minimized: "Запускать Thunderbird скрытым в трее",
                notifications: "Включить уведомления",
                language: "Язык интерфейса",
                theme: "Оформление",
                unread_count: "Показывать число непрочитанных писем на значке",
                hide_zero: "Разрешить Plasma скрывать значок, когда непрочитанных писем нет",
                close_limit_title: "Поведение кнопки закрытия",
                close_limit_text: "Thunderbird 156 и KWin 6 не предоставляют поддерживаемого перехвата кнопки закрытия в Linux. Используйте «Скрыть Thunderbird в трей»: окно исчезнет из панели KDE, а проверка почты продолжится.",
                command: "Исполняемый файл Thunderbird",
                backend: "Бэкенд управления окном",
                apply: "Применить",
                cancel: "Отмена",
                automatic: "Автоматически",
                theme_system: "Системная",
                theme_light: "Светлая",
                theme_dark: "Тёмная",
                backend_auto: "Автоматически",
                backend_none: "Нет",
                save_failed: "Не удалось сохранить настройки",
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error(transparent)]
    Autostart(#[from] AutostartError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("settings window failed: {0}")]
    Platform(#[from] slint::PlatformError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_box_indices_round_trip_stable_config_values() {
        for language in [LanguageMode::Auto, LanguageMode::En, LanguageMode::Ru] {
            assert_eq!(language_from_index(language_index(language)), language);
        }
        for backend in [
            WindowBackend::Auto,
            WindowBackend::KdeWayland,
            WindowBackend::X11,
            WindowBackend::None,
        ] {
            assert_eq!(backend_from_index(backend_index(backend)), backend);
        }
        for theme in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            assert_eq!(theme_from_index(theme_index(theme)), theme);
        }
    }
}
