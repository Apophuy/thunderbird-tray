// SPDX-License-Identifier: GPL-3.0-only

//! `ksni` adapter for the desktop-independent tray model.

use std::sync::mpsc::Sender;

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{RadioGroup, RadioItem, StandardItem, SubMenu};
use ksni::{Category, MenuItem, Status, ToolTip};

use crate::config::{LanguageMode, TrayConfig};
use crate::core::TrayState;
use crate::tray::{IndicatorStatus, TrayAction, TrayModel};
use crate::tray_icons;

pub struct SniTray {
    model: TrayModel,
    actions: Sender<TrayAction>,
}

impl SniTray {
    pub fn new(model: TrayModel, actions: Sender<TrayAction>) -> Self {
        Self { model, actions }
    }

    fn enqueue(&self, action: TrayAction) {
        let _ = self.actions.send(action);
    }
}

impl ksni::Tray for SniTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "thunderbird-tray".to_owned()
    }

    fn category(&self) -> Category {
        Category::Communications
    }

    fn title(&self) -> String {
        self.model.presentation().labels.title.to_owned()
    }

    fn status(&self) -> Status {
        match self.model.presentation().status {
            IndicatorStatus::Active => Status::Active,
            IndicatorStatus::Passive => Status::Passive,
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let presentation = self.model.presentation();
        tray_icons::pixmaps(presentation.icon, presentation.unread_count)
    }

    fn tool_tip(&self) -> ToolTip {
        let presentation = self.model.presentation();
        ToolTip {
            icon_name: String::new(),
            icon_pixmap: tray_icons::pixmaps(presentation.icon, presentation.unread_count),
            title: presentation.labels.title.to_owned(),
            description: presentation.labels.inbox_status,
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let presentation = self.model.presentation();
        let labels = presentation.labels;
        let selected_language = match presentation.language_mode {
            LanguageMode::Auto => 0,
            LanguageMode::En => 1,
            LanguageMode::Ru => 2,
        };

        vec![
            StandardItem {
                label: labels.open_thunderbird.to_owned(),
                enabled: presentation.can_open_thunderbird,
                icon_name: "thunderbird".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    tray.enqueue(TrayAction::OpenThunderbird);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: labels.hide_thunderbird.to_owned(),
                enabled: presentation.can_hide_thunderbird,
                icon_name: "window-minimize".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    tray.enqueue(TrayAction::HideThunderbird);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: labels.inbox_status,
                enabled: false,
                icon_name: "mail-unread".to_owned(),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: labels.refresh.to_owned(),
                enabled: presentation.can_refresh,
                icon_name: "view-refresh".to_owned(),
                shortcut: vec![vec!["Control".to_owned(), "R".to_owned()]],
                activate: Box::new(|tray: &mut Self| tray.enqueue(TrayAction::Refresh)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: labels.settings.to_owned(),
                icon_name: "preferences-system".to_owned(),
                activate: Box::new(|tray: &mut Self| tray.enqueue(TrayAction::OpenSettings)),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: labels.language.to_owned(),
                icon_name: "preferences-desktop-locale".to_owned(),
                submenu: vec![
                    RadioGroup {
                        selected: selected_language,
                        options: vec![
                            RadioItem {
                                label: labels.automatic.to_owned(),
                                ..Default::default()
                            },
                            RadioItem {
                                label: labels.english.to_owned(),
                                ..Default::default()
                            },
                            RadioItem {
                                label: labels.russian.to_owned(),
                                ..Default::default()
                            },
                        ],
                        select: Box::new(|tray: &mut Self, selected| {
                            let mode = match selected {
                                1 => LanguageMode::En,
                                2 => LanguageMode::Ru,
                                _ => LanguageMode::Auto,
                            };
                            tray.model.set_language_mode(mode);
                            tray.enqueue(TrayAction::SetLanguage(mode));
                        }),
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: labels.quit.to_owned(),
                icon_name: "application-exit".to_owned(),
                shortcut: vec![vec!["Control".to_owned(), "Q".to_owned()]],
                activate: Box::new(|tray: &mut Self| tray.enqueue(TrayAction::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn watcher_online(&self) {
        tracing::info!("StatusNotifierWatcher is online; tray item registered");
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        tracing::warn!(
            ?reason,
            "StatusNotifierWatcher is offline; awaiting recovery"
        );
        true
    }
}

pub struct SniService {
    handle: Handle<SniTray>,
}

impl SniService {
    pub fn spawn(model: TrayModel, actions: Sender<TrayAction>) -> Result<Self, ksni::Error> {
        let handle = SniTray::new(model, actions).spawn()?;
        Ok(Self { handle })
    }

    pub fn set_state(&self, state: TrayState) -> bool {
        self.handle
            .update(move |tray| tray.model.set_state(state))
            .is_some()
    }

    pub fn set_language_mode(&self, mode: LanguageMode) -> bool {
        self.handle
            .update(move |tray| tray.model.set_language_mode(mode))
            .is_some()
    }

    pub fn set_configuration(&self, config: TrayConfig, mode: LanguageMode) -> bool {
        self.handle
            .update(move |tray| tray.model.set_configuration(config, mode))
            .is_some()
    }

    pub fn shutdown(&self) {
        self.handle.shutdown().wait();
    }

    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TrayConfig;
    use crate::i18n::Language;
    use crate::tray::TrayIcon;

    fn tray(state: TrayState) -> (SniTray, std::sync::mpsc::Receiver<TrayAction>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        (
            SniTray::new(
                TrayModel::new(
                    state,
                    TrayConfig::default(),
                    LanguageMode::En,
                    Language::English,
                ),
                sender,
            ),
            receiver,
        )
    }

    #[test]
    fn menu_is_native_ordered_and_callbacks_only_enqueue_actions() {
        let (mut tray, receiver) = tray(TrayState::Unread(2));
        let mut menu = ksni::Tray::menu(&tray);
        assert_eq!(menu.len(), 8);

        let open = match menu.remove(0) {
            MenuItem::Standard(item) => item,
            _ => panic!("Open Thunderbird must be a standard item"),
        };
        assert!(open.enabled);

        let hide = match menu.remove(0) {
            MenuItem::Standard(item) => item,
            _ => panic!("Hide Thunderbird must be a standard item"),
        };
        assert!(!hide.enabled);

        let status = match menu.remove(0) {
            MenuItem::Standard(item) => item,
            _ => panic!("Inbox status must be a standard item"),
        };
        assert!(!status.enabled);
        assert!(status.label.contains("2 unread messages"));

        let refresh = match menu.remove(0) {
            MenuItem::Standard(item) => item,
            _ => panic!("Refresh must be a standard item"),
        };
        (refresh.activate)(&mut tray);
        assert_eq!(receiver.recv().unwrap(), TrayAction::Refresh);

        let quit = match menu.pop().unwrap() {
            MenuItem::Standard(item) => item,
            _ => panic!("Quit must be a standard item"),
        };
        (quit.activate)(&mut tray);
        assert_eq!(receiver.recv().unwrap(), TrayAction::Quit);
    }

    #[test]
    fn language_radio_updates_menu_before_persistence_is_processed() {
        let (mut tray, receiver) = tray(TrayState::NoUnread);
        let mut menu = ksni::Tray::menu(&tray);
        let language = match menu.remove(5) {
            MenuItem::SubMenu(item) => item,
            _ => panic!("Language must be a submenu"),
        };
        let group = match language.submenu.into_iter().next().unwrap() {
            MenuItem::RadioGroup(group) => group,
            _ => panic!("Language choices must be a radio group"),
        };

        (group.select)(&mut tray, 2);

        assert_eq!(tray.model.language_mode(), LanguageMode::Ru);
        assert_eq!(
            receiver.recv().unwrap(),
            TrayAction::SetLanguage(LanguageMode::Ru)
        );
        assert_eq!(ksni::Tray::menu(&tray)[5].as_submenu_label(), Some("_Язык"));
    }

    #[test]
    fn every_visual_state_has_three_well_formed_argb_pixmaps() {
        for state in [
            TrayState::Disconnected,
            TrayState::NoUnread,
            TrayState::Unread(1),
        ] {
            let (tray, _) = tray(state);
            let pixmaps = ksni::Tray::icon_pixmap(&tray);
            assert_eq!(pixmaps.len(), 3);
            for icon in pixmaps {
                assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
                assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] != 0));
            }
        }

        assert_eq!(
            tray(TrayState::NoUnread).0.model.presentation().icon,
            TrayIcon::Connected
        );
    }

    #[test]
    fn watcher_loss_keeps_the_service_alive_for_reregistration() {
        let (tray, _) = tray(TrayState::Disconnected);
        assert!(ksni::Tray::watcher_offline(&tray, ksni::OfflineReason::No));
    }

    trait MenuItemTestExt<T> {
        fn as_submenu_label(&self) -> Option<&str>;
    }

    impl<T> MenuItemTestExt<T> for MenuItem<T> {
        fn as_submenu_label(&self) -> Option<&str> {
            match self {
                MenuItem::SubMenu(item) => Some(item.label.as_str()),
                _ => None,
            }
        }
    }
}
