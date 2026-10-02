// SPDX-License-Identifier: GPL-3.0-only

//! Native settings window backed by the same strict TOML configuration.

use std::cell::{Cell, RefCell};
use std::io;
use std::process::{Command, Stdio};
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};
use thiserror::Error;

use crate::autostart::{AutostartEntry, AutostartError};
use crate::config::{Config, ConfigError, ConfigSource, LanguageMode, ThemeMode, WindowBackend};
use crate::i18n::{Language, language_for_mode};
use crate::lifecycle::LifecycleClient;

const AUTHOR_EMAIL: &str = "apophuy@hotmail.com";

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
        in property <string> about-tab;
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
        in property <string> about-name;
        in property <string> about-version;
        in property <string> about-author;
        in property <string> about-license;
        in property <string> contact-author-label;
        in property <string> apply-label;
        in property <string> done-label;
        in property <string> cancel-label;
        in property <bool> russian-interface;
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
        in-out property <bool> status-is-error;
        in-out property <bool> apply-enabled;

        callback apply(bool);
        callback cancel();
        callback edited();
        callback email-author();

        changed autostart-enabled => { root.edited(); }
        changed start-thunderbird => { root.edited(); }
        changed start-minimized => { root.edited(); }
        changed notifications-enabled => { root.edited(); }
        changed show-unread-count => { root.edited(); }
        changed hide-when-zero => { root.edited(); }
        changed language-index => { root.edited(); }
        changed backend-index => { root.edited(); }
        changed thunderbird-command => { root.edited(); }

        changed theme-index => {
            Palette.color-scheme = root.theme-index == 1 ? ColorScheme.light
                : root.theme-index == 2 ? ColorScheme.dark
                : ColorScheme.unknown;
            root.edited();
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
                            visible: !root.russian-interface;
                            model: ["Automatic", "English", "Русский"];
                            current-index <=> root.language-index;
                        }
                        ComboBox {
                            visible: root.russian-interface;
                            model: ["Автоматически", "English", "Русский"];
                            current-index <=> root.language-index;
                        }
                        Text { text: root.theme-label; }
                        ComboBox {
                            visible: !root.russian-interface;
                            model: ["System", "Light", "Dark"];
                            current-index <=> root.theme-index;
                        }
                        ComboBox {
                            visible: root.russian-interface;
                            model: ["Системная", "Светлая", "Тёмная"];
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
                            visible: !root.russian-interface;
                            model: ["Automatic", "KDE Plasma Wayland", "X11", "None"];
                            current-index <=> root.backend-index;
                        }
                        ComboBox {
                            visible: root.russian-interface;
                            model: ["Автоматически", "KDE Plasma Wayland", "X11", "Нет"];
                            current-index <=> root.backend-index;
                        }
                        Rectangle { vertical-stretch: 1; }
                    }
                }

                Tab {
                    title: root.about-tab;
                    VerticalLayout {
                        padding: 24px;
                        spacing: 12px;
                        Rectangle { vertical-stretch: 1; }
                        HorizontalLayout {
                            alignment: center;
                            Image {
                                source: root.app-icon;
                                width: 96px;
                                height: 96px;
                                image-fit: contain;
                            }
                        }
                        Text {
                            text: root.about-name;
                            horizontal-alignment: center;
                            font-size: 24px;
                            font-weight: 700;
                        }
                        Text {
                            text: root.about-version;
                            horizontal-alignment: center;
                        }
                        Text {
                            text: root.about-author;
                            horizontal-alignment: center;
                        }
                        HorizontalLayout {
                            alignment: center;
                            Button {
                                text: root.contact-author-label;
                                icon: @image-url("assets/mail.svg");
                                colorize-icon: true;
                                clicked => { root.email-author(); }
                            }
                        }
                        Text {
                            text: root.about-license;
                            horizontal-alignment: center;
                        }
                        Rectangle { vertical-stretch: 1; }
                    }
                }
            }

            Text {
                text: root.status-message;
                color: root.status-is-error ? #b91c1c : #15803d;
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
                    enabled: root.apply-enabled;
                    clicked => { root.apply(false); }
                }
                Button {
                    text: root.done-label;
                    primary: true;
                    clicked => { root.apply(true); }
                }
            }
        }
    }
}

pub fn run(
    config_source: ConfigSource,
    config: Config,
    automatic_language: Language,
) -> Result<(), SettingsError> {
    let autostart = AutostartEntry::discover()?;
    let window = SettingsWindow::new()?;
    let language = language_for_mode(config.general.language, automatic_language);
    let strings = apply_language(&window, language);

    if let Some(icon) = crate::tray_icons::application_image() {
        window.set_app_icon(icon);
    }

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
    window.set_apply_enabled(false);

    if let Err(error) = ensure_tray_service(&config_source) {
        window.set_status_is_error(true);
        window.set_status_message(format!("{}: {error}", strings.apply_failed).into());
    }

    let saved_values = Rc::new(RefCell::new(SettingsValues::from_window(&window)));
    let preview_language = Rc::new(Cell::new(language));
    install_edit_handler(
        &window,
        Rc::clone(&saved_values),
        preview_language,
        automatic_language,
    );

    let weak = window.as_weak();
    window.on_email_author(move || {
        if let Err(error) = open_author_email() {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let language = language_for_mode(
                language_from_index(window.get_language_index()),
                automatic_language,
            );
            let strings = SettingsStrings::new(language);
            window.set_status_is_error(true);
            window.set_status_message(format!("{}: {error}", strings.email_failed).into());
        }
    });

    window.on_cancel(move || {
        let _ = slint::quit_event_loop();
    });

    let weak = window.as_weak();
    let saved_config = config.clone();
    let applied_saved_values = Rc::clone(&saved_values);
    window.on_apply(move |close_after_success| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let current_values = SettingsValues::from_window(&window);
        if current_values == *applied_saved_values.borrow() {
            if close_after_success {
                let _ = slint::quit_event_loop();
            }
            return;
        }
        window.set_status_message(SharedString::default());
        let strings = SettingsStrings::new(language_for_mode(
            current_values.language,
            automatic_language,
        ));
        let mut updated = saved_config.clone();
        current_values.write_to_config(&mut updated);

        let previous_autostart = autostart.is_enabled();
        let requested_autostart = current_values.autostart_enabled;
        let result = autostart
            .set_enabled(requested_autostart)
            .map_err(SettingsError::Autostart)
            .and_then(|()| config_source.save(&updated).map_err(SettingsError::Config));
        if let Err(error) = result {
            if previous_autostart != requested_autostart {
                let _ = autostart.set_enabled(previous_autostart);
            }
            window.set_status_is_error(true);
            window.set_status_message(format!("{}: {error}", strings.save_failed).into());
            return;
        }

        let service_result = LifecycleClient::connect().and_then(|client| {
            client.ensure_service_running(Some(config_source.path()))?;
            client.notify_configuration_changed()
        });
        if let Err(error) = service_result {
            window.set_status_is_error(true);
            window.set_status_message(format!("{}: {error}", strings.apply_failed).into());
            return;
        }

        window.set_status_is_error(false);
        window.set_status_message(strings.applied.into());
        *applied_saved_values.borrow_mut() = current_values;
        window.set_apply_enabled(false);
        if close_after_success {
            let _ = slint::quit_event_loop();
        }
    });

    window.run()?;
    Ok(())
}

fn install_edit_handler(
    window: &SettingsWindow,
    saved_values: Rc<RefCell<SettingsValues>>,
    preview_language: Rc<Cell<Language>>,
    automatic_language: Language,
) {
    let weak = window.as_weak();
    window.on_edited(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let current_values = SettingsValues::from_window(&window);
        let language = language_for_mode(current_values.language, automatic_language);
        if language != preview_language.get() {
            apply_language(&window, language);
            preview_language.set(language);
        }
        let changed = current_values != *saved_values.borrow();
        window.set_apply_enabled(changed);
        window.set_status_message(SharedString::default());
    });
}

fn apply_language(window: &SettingsWindow, language: Language) -> SettingsStrings {
    let strings = SettingsStrings::new(language);
    window.set_russian_interface(language == Language::Russian);
    window.set_window_title(strings.window_title.into());
    window.set_general_tab(strings.general_tab.into());
    window.set_tray_tab(strings.tray_tab.into());
    window.set_thunderbird_tab(strings.thunderbird_tab.into());
    window.set_about_tab(strings.about_tab.into());
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
    window.set_about_name("thunderbird-tray".into());
    window.set_about_version(about_version(strings).into());
    window.set_about_author(about_author(strings).into());
    window.set_about_license(strings.license.into());
    window.set_contact_author_label(strings.contact_author.into());
    window.set_apply_label(strings.apply.into());
    window.set_done_label(strings.done.into());
    window.set_cancel_label(strings.cancel.into());
    strings
}

fn about_version(strings: SettingsStrings) -> String {
    format!("{} {}", strings.version, env!("CARGO_PKG_VERSION"))
}

fn about_author(strings: SettingsStrings) -> String {
    format!("{} Apophuy", strings.author)
}

fn author_email_command() -> Command {
    let mut command = Command::new("xdg-email");
    command
        .arg(AUTHOR_EMAIL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn open_author_email() -> io::Result<()> {
    let mut child = author_email_command().spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn ensure_tray_service(
    config_source: &ConfigSource,
) -> Result<(), crate::lifecycle::LifecycleError> {
    let client = LifecycleClient::connect()?;
    client.ensure_service_running(Some(config_source.path()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SettingsValues {
    autostart_enabled: bool,
    start_thunderbird: bool,
    start_minimized: bool,
    notifications_enabled: bool,
    show_unread_count: bool,
    hide_when_zero: bool,
    language: LanguageMode,
    theme: ThemeMode,
    backend: WindowBackend,
    thunderbird_command: String,
}

impl SettingsValues {
    fn from_window(window: &SettingsWindow) -> Self {
        let start_thunderbird = window.get_start_thunderbird();
        Self {
            autostart_enabled: window.get_autostart_enabled(),
            start_thunderbird,
            start_minimized: start_thunderbird && window.get_start_minimized(),
            notifications_enabled: window.get_notifications_enabled(),
            show_unread_count: window.get_show_unread_count(),
            hide_when_zero: window.get_hide_when_zero(),
            language: language_from_index(window.get_language_index()),
            theme: theme_from_index(window.get_theme_index()),
            backend: backend_from_index(window.get_backend_index()),
            thunderbird_command: window.get_thunderbird_command().to_string(),
        }
    }

    #[cfg(test)]
    fn from_config(config: &Config, autostart_enabled: bool) -> Self {
        Self {
            autostart_enabled,
            start_thunderbird: config.general.start_thunderbird,
            start_minimized: config.general.start_thunderbird && config.general.start_minimized,
            notifications_enabled: config.general.notifications,
            show_unread_count: config.tray.show_unread_count,
            hide_when_zero: config.tray.hide_when_zero,
            language: config.general.language,
            theme: config.general.theme,
            backend: config.window.backend,
            thunderbird_command: config.thunderbird.command.clone(),
        }
    }

    fn write_to_config(&self, config: &mut Config) {
        config.general.start_thunderbird = self.start_thunderbird;
        config.general.start_minimized = self.start_minimized;
        config.general.notifications = self.notifications_enabled;
        config.general.language = self.language;
        config.general.theme = self.theme;
        config.tray.show_unread_count = self.show_unread_count;
        config.tray.hide_when_zero = self.hide_when_zero;
        config
            .thunderbird
            .command
            .clone_from(&self.thunderbird_command);
        config.window.backend = self.backend;
    }
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
    about_tab: &'static str,
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
    version: &'static str,
    author: &'static str,
    contact_author: &'static str,
    license: &'static str,
    apply: &'static str,
    done: &'static str,
    cancel: &'static str,
    save_failed: &'static str,
    apply_failed: &'static str,
    email_failed: &'static str,
    applied: &'static str,
}

impl SettingsStrings {
    fn new(language: Language) -> Self {
        match language {
            Language::English => Self {
                window_title: "thunderbird-tray Settings",
                general_tab: "General",
                tray_tab: "Tray",
                thunderbird_tab: "Thunderbird",
                about_tab: "About",
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
                version: "Version",
                author: "Author:",
                contact_author: "Contact the author",
                license: "License: GPL-3.0-only",
                apply: "Apply",
                done: "Done",
                cancel: "Cancel",
                save_failed: "Could not save settings",
                apply_failed: "Settings were saved, but could not be applied",
                email_failed: "Could not open the mail application",
                applied: "Settings applied. thunderbird-tray is running.",
            },
            Language::Russian => Self {
                window_title: "Настройки thunderbird-tray",
                general_tab: "Общие",
                tray_tab: "Трей",
                thunderbird_tab: "Thunderbird",
                about_tab: "О приложении",
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
                version: "Версия",
                author: "Автор:",
                contact_author: "Написать автору",
                license: "Лицензия: GPL-3.0-only",
                apply: "Применить",
                done: "Готово",
                cancel: "Отмена",
                save_failed: "Не удалось сохранить настройки",
                apply_failed: "Настройки сохранены, но применить их не удалось",
                email_failed: "Не удалось открыть почтовое приложение",
                applied: "Настройки применены. thunderbird-tray запущен.",
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
    use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
    use slint::platform::{Platform, PlatformError, WindowAdapter};

    thread_local! {
        static TEST_WINDOW: Rc<MinimalSoftwareWindow> =
            MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    }

    struct TestPlatform;

    impl Platform for TestPlatform {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(TEST_WINDOW.with(Rc::clone))
        }
    }

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

    #[test]
    fn settings_actions_have_distinct_localized_labels() {
        let english = SettingsStrings::new(Language::English);
        assert_eq!(
            (english.cancel, english.apply, english.done),
            ("Cancel", "Apply", "Done")
        );

        let russian = SettingsStrings::new(Language::Russian);
        assert_eq!(
            (russian.cancel, russian.apply, russian.done),
            ("Отмена", "Применить", "Готово")
        );
    }

    #[test]
    fn settings_language_follows_manual_and_automatic_modes() {
        let automatic =
            SettingsStrings::new(language_for_mode(LanguageMode::Auto, Language::Russian));
        let english = SettingsStrings::new(language_for_mode(LanguageMode::En, Language::Russian));
        let russian = SettingsStrings::new(language_for_mode(LanguageMode::Ru, Language::English));

        assert_eq!(automatic.language, "Язык интерфейса");
        assert_eq!(english.language, "Interface language");
        assert_eq!(russian.language, "Язык интерфейса");
    }

    #[test]
    fn language_selection_survives_apply_and_settings_reopen() {
        let automatic_language = Language::Russian;
        let mut config = Config::default();
        let mut selected = SettingsValues::from_config(&config, false);
        selected.language = language_from_index(language_index(LanguageMode::En));
        selected.theme = ThemeMode::Dark;
        selected.backend = WindowBackend::KdeWayland;
        selected.write_to_config(&mut config);
        assert_eq!(config.general.language, LanguageMode::En);

        let reopened_language = language_for_mode(config.general.language, automatic_language);
        assert_eq!(reopened_language, Language::English);
        assert_eq!(language_index(config.general.language), 1);
        assert_eq!(
            SettingsStrings::new(reopened_language).language,
            "Interface language"
        );

        slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
        let window = SettingsWindow::new().unwrap();
        apply_language(&window, reopened_language);
        window.set_language_index(language_index(config.general.language));
        window.set_theme_index(theme_index(config.general.theme));
        window.set_backend_index(backend_index(config.window.backend));
        window.show().unwrap();
        TEST_WINDOW.with(|test_window| {
            let size = slint::PhysicalSize::new(680, 540);
            test_window.set_size(size);
            let mut pixels =
                slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(size.width, size.height);
            assert!(test_window.draw_if_needed(|renderer| {
                renderer.render(pixels.make_mut_slice(), size.width as usize);
            }));
        });
        assert_eq!(
            window.get_language_index(),
            language_index(LanguageMode::En)
        );
        assert_eq!(window.get_theme_index(), theme_index(ThemeMode::Dark));
        assert_eq!(
            window.get_backend_index(),
            backend_index(WindowBackend::KdeWayland)
        );

        apply_language(&window, Language::Russian);
        TEST_WINDOW.with(|test_window| {
            let size = slint::PhysicalSize::new(680, 540);
            let mut pixels =
                slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(size.width, size.height);
            assert!(test_window.draw_if_needed(|renderer| {
                renderer.render(pixels.make_mut_slice(), size.width as usize);
            }));
        });
        assert!(window.get_russian_interface());
        assert_eq!(
            window.get_language_index(),
            language_index(LanguageMode::En)
        );
        assert_eq!(window.get_theme_index(), theme_index(ThemeMode::Dark));
        assert_eq!(
            window.get_backend_index(),
            backend_index(WindowBackend::KdeWayland)
        );
    }

    #[test]
    fn about_details_use_package_version_without_exposing_email() {
        let strings = SettingsStrings::new(Language::English);
        let author = about_author(strings);
        let mail_icon =
            slint::Image::load_from_svg_data(include_bytes!("../assets/mail.svg")).unwrap();

        assert_eq!(author, "Author: Apophuy");
        assert_eq!(
            about_version(strings),
            format!("Version {}", env!("CARGO_PKG_VERSION"))
        );
        assert!(!author.contains(AUTHOR_EMAIL));
        assert_eq!(mail_icon.size(), [24, 24].into());
    }

    #[test]
    fn mail_button_uses_xdg_email_without_a_shell() {
        let command = author_email_command();
        assert_eq!(command.get_program(), "xdg-email");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [std::ffi::OsStr::new(AUTHOR_EMAIL)]
        );
    }

    #[test]
    fn settings_snapshot_detects_real_changes_and_writes_them_back() {
        let config = Config::default();
        let saved = SettingsValues::from_config(&config, false);
        assert_eq!(saved, SettingsValues::from_config(&config, false));

        let mut changed = saved.clone();
        changed.show_unread_count = !changed.show_unread_count;
        changed.autostart_enabled = true;
        assert_ne!(changed, saved);

        let mut updated = config;
        changed.write_to_config(&mut updated);
        assert_eq!(updated.tray.show_unread_count, changed.show_unread_count);
        assert_eq!(updated.general.start_thunderbird, changed.start_thunderbird);
    }

    #[test]
    fn hidden_start_is_ignored_while_thunderbird_startup_is_disabled() {
        let mut config = Config::default();
        config.general.start_thunderbird = false;
        config.general.start_minimized = true;

        let values = SettingsValues::from_config(&config, false);
        assert!(!values.start_minimized);
        values.write_to_config(&mut config);
        assert!(!config.general.start_minimized);
    }
}
