// SPDX-License-Identifier: GPL-3.0-only

//! Optional X11/EWMH window control.

use crate::window::DesktopEnvironment;

pub const fn feature_enabled() -> bool {
    cfg!(feature = "x11")
}

#[cfg(feature = "x11")]
mod implementation {
    use std::thread;
    use std::time::{Duration, Instant};

    use thiserror::Error;
    use x11rb::CURRENT_TIME;
    use x11rb::connection::Connection;
    use x11rb::errors::{ConnectError, ConnectionError, ParseError, ReplyError};
    use x11rb::properties::WmClass;
    use x11rb::protocol::xproto::{
        Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, Window,
    };
    use x11rb::rust_connection::RustConnection;

    use super::DesktopEnvironment;
    use crate::config::WindowBackend;
    use crate::window::{
        ActivationOutcome, ToggleVisibilityOutcome, WindowCapabilities, WindowControl, WindowError,
        WindowOperation,
    };

    const OPERATION_TIMEOUT: Duration = Duration::from_millis(750);
    const POLL_INTERVAL: Duration = Duration::from_millis(25);
    const ICCCM_ICONIC_STATE: u32 = 3;
    const EWMH_SOURCE_PAGER: u32 = 2;

    pub struct X11WindowControl {
        connection: RustConnection,
        root: Window,
        atoms: Atoms,
    }

    impl X11WindowControl {
        pub fn connect(environment: &DesktopEnvironment) -> Result<Option<Self>, X11Error> {
            if !environment.is_x11() {
                return Ok(None);
            }
            let display = environment
                .display
                .as_deref()
                .and_then(|value| value.to_str())
                .ok_or(X11Error::DisplayUnavailable)?;
            let (connection, screen_index) = RustConnection::connect(Some(display))?;
            let root = connection
                .setup()
                .roots
                .get(screen_index)
                .ok_or(X11Error::MissingScreen(screen_index))?
                .root;
            let atoms = Atoms::intern(&connection)?;
            atoms.require_ewmh(&connection, root)?;
            Ok(Some(Self {
                connection,
                root,
                atoms,
            }))
        }

        fn thunderbird_window(&self) -> Result<Option<Window>, X11Error> {
            let reply = self
                .connection
                .get_property(
                    false,
                    self.root,
                    self.atoms.net_client_list_stacking,
                    AtomEnum::WINDOW,
                    0,
                    u32::MAX,
                )?
                .reply()?;
            let windows = reply
                .value32()
                .ok_or(X11Error::InvalidProperty("_NET_CLIENT_LIST_STACKING"))?;
            for window in windows.collect::<Vec<_>>().into_iter().rev() {
                let class = match WmClass::get(&self.connection, window)?.reply() {
                    Ok(class) => class,
                    Err(ReplyError::X11Error(_)) => continue,
                    Err(source) => return Err(source.into()),
                };
                if class.as_ref().is_some_and(is_thunderbird_class) {
                    return Ok(Some(window));
                }
            }
            Ok(None)
        }

        fn activate_window(&self, window: Window) -> Result<(), X11Error> {
            let event = ClientMessageEvent::new(
                32,
                window,
                self.atoms.net_active_window,
                [EWMH_SOURCE_PAGER, CURRENT_TIME, 0, 0, 0],
            );
            self.connection
                .send_event(
                    false,
                    self.root,
                    EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                    event,
                )?
                .check()?;
            self.connection.flush()?;
            self.wait_until("activation", || {
                self.active_window().map(|active| active == Some(window))
            })
        }

        fn active_window(&self) -> Result<Option<Window>, X11Error> {
            let reply = self
                .connection
                .get_property(
                    false,
                    self.root,
                    self.atoms.net_active_window,
                    AtomEnum::WINDOW,
                    0,
                    1,
                )?
                .reply()?;
            Ok(reply.value32().and_then(|mut values| values.next()))
        }

        fn set_iconic(&self, window: Window) -> Result<(), X11Error> {
            let event = ClientMessageEvent::new(
                32,
                window,
                self.atoms.wm_change_state,
                [ICCCM_ICONIC_STATE, 0, 0, 0, 0],
            );
            self.connection
                .send_event(
                    false,
                    self.root,
                    EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                    event,
                )?
                .check()?;
            self.connection.flush()?;
            self.wait_until("minimize", || self.is_hidden(window))
        }

        fn restore(&self, window: Window) -> Result<(), X11Error> {
            self.connection.map_window(window)?.check()?;
            self.connection.flush()?;
            self.wait_until("restore", || self.is_hidden(window).map(|hidden| !hidden))?;
            self.activate_window(window)
        }

        fn is_hidden(&self, window: Window) -> Result<bool, X11Error> {
            let reply = self
                .connection
                .get_property(
                    false,
                    window,
                    self.atoms.net_wm_state,
                    AtomEnum::ATOM,
                    0,
                    u32::MAX,
                )?
                .reply()?;
            Ok(reply.value32().is_some_and(|mut values| {
                values.any(|atom| atom == self.atoms.net_wm_state_hidden)
            }))
        }

        fn wait_until(
            &self,
            operation: &'static str,
            mut condition: impl FnMut() -> Result<bool, X11Error>,
        ) -> Result<(), X11Error> {
            let deadline = Instant::now() + OPERATION_TIMEOUT;
            loop {
                if condition()? {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(X11Error::OperationTimeout(operation));
                }
                thread::sleep(POLL_INTERVAL);
            }
        }

        fn operation_error(operation: WindowOperation, source: X11Error) -> WindowError {
            WindowError::OperationFailed {
                backend: WindowBackend::X11,
                operation,
                message: source.to_string(),
            }
        }
    }

    impl WindowControl for X11WindowControl {
        fn backend(&self) -> WindowBackend {
            WindowBackend::X11
        }

        fn capabilities(&self) -> WindowCapabilities {
            WindowCapabilities {
                detect: true,
                activate: true,
                hide: true,
                show: true,
            }
        }

        fn detect(&self) -> Result<bool, WindowError> {
            self.thunderbird_window()
                .map(|window| window.is_some())
                .map_err(|source| Self::operation_error(WindowOperation::Detect, source))
        }

        fn activate(&self) -> Result<ActivationOutcome, WindowError> {
            let Some(window) = self
                .thunderbird_window()
                .map_err(|source| Self::operation_error(WindowOperation::Activate, source))?
            else {
                return Ok(ActivationOutcome::NoWindow);
            };
            self.restore(window)
                .map(|()| ActivationOutcome::Activated)
                .map_err(|source| Self::operation_error(WindowOperation::Activate, source))
        }

        fn hide(&self) -> Result<(), WindowError> {
            let window = self
                .thunderbird_window()
                .map_err(|source| Self::operation_error(WindowOperation::Hide, source))?
                .ok_or_else(|| {
                    Self::operation_error(WindowOperation::Hide, X11Error::NoThunderbirdWindow)
                })?;
            self.set_iconic(window)
                .map_err(|source| Self::operation_error(WindowOperation::Hide, source))
        }

        fn show(&self) -> Result<(), WindowError> {
            let window = self
                .thunderbird_window()
                .map_err(|source| Self::operation_error(WindowOperation::Show, source))?
                .ok_or_else(|| {
                    Self::operation_error(WindowOperation::Show, X11Error::NoThunderbirdWindow)
                })?;
            self.restore(window)
                .map_err(|source| Self::operation_error(WindowOperation::Show, source))
        }

        fn toggle_visibility(&self) -> Result<ToggleVisibilityOutcome, WindowError> {
            let Some(window) = self
                .thunderbird_window()
                .map_err(|source| Self::operation_error(WindowOperation::Toggle, source))?
            else {
                return Ok(ToggleVisibilityOutcome::NoWindow);
            };
            let hidden = self
                .is_hidden(window)
                .map_err(|source| Self::operation_error(WindowOperation::Toggle, source))?;
            if hidden {
                self.restore(window)
                    .map(|()| ToggleVisibilityOutcome::Shown)
                    .map_err(|source| Self::operation_error(WindowOperation::Toggle, source))
            } else {
                self.set_iconic(window)
                    .map(|()| ToggleVisibilityOutcome::Hidden)
                    .map_err(|source| Self::operation_error(WindowOperation::Toggle, source))
            }
        }
    }

    struct Atoms {
        net_supported: Atom,
        net_client_list_stacking: Atom,
        net_active_window: Atom,
        net_wm_state: Atom,
        net_wm_state_hidden: Atom,
        wm_change_state: Atom,
    }

    impl Atoms {
        fn intern(connection: &RustConnection) -> Result<Self, X11Error> {
            Ok(Self {
                net_supported: intern(connection, b"_NET_SUPPORTED")?,
                net_client_list_stacking: intern(connection, b"_NET_CLIENT_LIST_STACKING")?,
                net_active_window: intern(connection, b"_NET_ACTIVE_WINDOW")?,
                net_wm_state: intern(connection, b"_NET_WM_STATE")?,
                net_wm_state_hidden: intern(connection, b"_NET_WM_STATE_HIDDEN")?,
                wm_change_state: intern(connection, b"WM_CHANGE_STATE")?,
            })
        }

        fn require_ewmh(&self, connection: &RustConnection, root: Window) -> Result<(), X11Error> {
            let reply = connection
                .get_property(false, root, self.net_supported, AtomEnum::ATOM, 0, u32::MAX)?
                .reply()?;
            let supported = reply
                .value32()
                .ok_or(X11Error::InvalidProperty("_NET_SUPPORTED"))?
                .collect::<Vec<_>>();
            for (atom, name) in [
                (self.net_client_list_stacking, "_NET_CLIENT_LIST_STACKING"),
                (self.net_active_window, "_NET_ACTIVE_WINDOW"),
                (self.net_wm_state, "_NET_WM_STATE"),
                (self.net_wm_state_hidden, "_NET_WM_STATE_HIDDEN"),
            ] {
                if !supported.contains(&atom) {
                    return Err(X11Error::UnsupportedEwmh(name));
                }
            }
            Ok(())
        }
    }

    fn intern(connection: &RustConnection, name: &[u8]) -> Result<Atom, X11Error> {
        Ok(connection.intern_atom(false, name)?.reply()?.atom)
    }

    fn is_thunderbird_class(class: &WmClass) -> bool {
        [class.class(), class.instance()].into_iter().any(|value| {
            value.eq_ignore_ascii_case(b"thunderbird")
                || value.eq_ignore_ascii_case(b"thunderbird-default")
        })
    }

    #[derive(Debug, Error)]
    pub enum X11Error {
        #[error("DISPLAY is unavailable or is not valid UTF-8")]
        DisplayUnavailable,
        #[error("X11 connection failed: {0}")]
        Connect(#[from] ConnectError),
        #[error("X11 connection operation failed: {0}")]
        Connection(#[from] ConnectionError),
        #[error("X11 server rejected a request: {0}")]
        Reply(#[from] ReplyError),
        #[error("X11 property could not be parsed: {0}")]
        Parse(#[from] ParseError),
        #[error("X11 display does not contain screen {0}")]
        MissingScreen(usize),
        #[error("X11 property {0} is missing or has an unexpected type")]
        InvalidProperty(&'static str),
        #[error("the X11 window manager does not advertise {0}")]
        UnsupportedEwmh(&'static str),
        #[error("no Thunderbird X11 window was found")]
        NoThunderbirdWindow,
        #[error("the X11 window manager did not confirm {0} in time")]
        OperationTimeout(&'static str),
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn thunderbird_wm_class_matching_is_narrow_and_case_insensitive() {
            fn class(instance: &[u8], class: &[u8]) -> WmClass {
                let mut value = instance.to_vec();
                value.push(0);
                value.extend_from_slice(class);
                value.push(0);
                WmClass::from_reply(x11rb::protocol::xproto::GetPropertyReply {
                    sequence: 0,
                    length: 0,
                    format: 8,
                    type_: AtomEnum::STRING.into(),
                    bytes_after: 0,
                    value_len: value.len() as u32,
                    value,
                })
                .unwrap()
                .unwrap()
            }

            assert!(is_thunderbird_class(&class(b"Mail", b"Thunderbird")));
            assert!(is_thunderbird_class(&class(
                b"thunderbird-default",
                b"Navigator"
            )));
            assert!(!is_thunderbird_class(&class(b"mail", b"other-client")));
        }
    }
}

#[cfg(feature = "x11")]
pub use implementation::{X11Error, X11WindowControl};

#[cfg(not(feature = "x11"))]
mod implementation {
    use thiserror::Error;

    use super::DesktopEnvironment;
    use crate::config::WindowBackend;
    use crate::window::{
        ActivationOutcome, ToggleVisibilityOutcome, WindowCapabilities, WindowControl, WindowError,
        WindowOperation,
    };

    pub struct X11WindowControl;

    impl X11WindowControl {
        pub fn connect(_environment: &DesktopEnvironment) -> Result<Option<Self>, X11Error> {
            Ok(None)
        }

        fn unsupported(operation: WindowOperation) -> WindowError {
            WindowError::Unsupported {
                backend: WindowBackend::X11,
                operation,
                reason: "the binary was built without the x11 feature".to_owned(),
            }
        }
    }

    impl WindowControl for X11WindowControl {
        fn backend(&self) -> WindowBackend {
            WindowBackend::X11
        }

        fn capabilities(&self) -> WindowCapabilities {
            WindowCapabilities::default()
        }

        fn detect(&self) -> Result<bool, WindowError> {
            Err(Self::unsupported(WindowOperation::Detect))
        }

        fn activate(&self) -> Result<ActivationOutcome, WindowError> {
            Err(Self::unsupported(WindowOperation::Activate))
        }

        fn hide(&self) -> Result<(), WindowError> {
            Err(Self::unsupported(WindowOperation::Hide))
        }

        fn show(&self) -> Result<(), WindowError> {
            Err(Self::unsupported(WindowOperation::Show))
        }

        fn toggle_visibility(&self) -> Result<ToggleVisibilityOutcome, WindowError> {
            Err(Self::unsupported(WindowOperation::Toggle))
        }
    }

    #[derive(Debug, Error)]
    #[error("the binary was built without the x11 feature")]
    pub struct X11Error;
}

#[cfg(not(feature = "x11"))]
pub use implementation::{X11Error, X11WindowControl};
