// SPDX-License-Identifier: GPL-3.0-only

//! Session-bus ownership and Native Messaging stream handoff.

use std::fs::File;
use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use thiserror::Error;
use zbus::blocking::connection::Builder;
use zbus::blocking::{Connection, Proxy};
use zbus::names::BusName;
use zbus::zvariant::{Fd, OwnedFd};

pub const APPLICATION_ID: &str = env!("THUNDERBIRD_TRAY_APPLICATION_ID");
pub const LIFECYCLE_INTERFACE: &str = env!("THUNDERBIRD_TRAY_LIFECYCLE_INTERFACE");
pub const LIFECYCLE_OBJECT_PATH: &str = env!("THUNDERBIRD_TRAY_LIFECYCLE_OBJECT_PATH");

pub struct AttachedNativeStream {
    pub input: File,
    pub output: File,
    pub completion_guard: UnixStream,
}

struct LifecycleInterface {
    attached_streams: Sender<AttachedNativeStream>,
    session_active: Arc<AtomicBool>,
}

#[zbus::interface(name = "io.github.apophuy.thunderbird_tray.Lifecycle")]
impl LifecycleInterface {
    fn attach_native_stream(&self, input: OwnedFd, output: OwnedFd) -> zbus::fdo::Result<OwnedFd> {
        if self
            .session_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(zbus::fdo::Error::Failed(
                "a Native Messaging session is already active".to_owned(),
            ));
        }

        let (completion_waiter, completion_guard) = UnixStream::pair().map_err(|error| {
            self.session_active.store(false, Ordering::Release);
            zbus::fdo::Error::Failed(format!("could not create completion channel: {error}"))
        })?;
        let stream = AttachedNativeStream {
            input: File::from(std::os::fd::OwnedFd::from(input)),
            output: File::from(std::os::fd::OwnedFd::from(output)),
            completion_guard,
        };
        if self.attached_streams.send(stream).is_err() {
            self.session_active.store(false, Ordering::Release);
            return Err(zbus::fdo::Error::Failed(
                "the primary thunderbird-tray process is shutting down".to_owned(),
            ));
        }

        Ok(OwnedFd::from(std::os::fd::OwnedFd::from(completion_waiter)))
    }
}

pub struct LifecycleService {
    _connection: Connection,
    session_active: Arc<AtomicBool>,
}

impl LifecycleService {
    pub fn claim(attached_streams: Sender<AttachedNativeStream>) -> Result<Self, ClaimError> {
        let session_active = Arc::new(AtomicBool::new(false));
        let interface = LifecycleInterface {
            attached_streams,
            session_active: Arc::clone(&session_active),
        };
        let connection = Builder::session()
            .map_err(ClaimError::Bus)?
            .serve_at(LIFECYCLE_OBJECT_PATH, interface)
            .map_err(ClaimError::Bus)?
            .name(APPLICATION_ID)
            .map_err(ClaimError::Bus)?
            .allow_name_replacements(false)
            .replace_existing_names(false)
            .build()
            .map_err(|error| match error {
                zbus::Error::NameTaken => ClaimError::AlreadyRunning,
                source => ClaimError::Bus(source),
            })?;

        Ok(Self {
            _connection: connection,
            session_active,
        })
    }

    pub fn session_finished(&self) {
        self.session_active.store(false, Ordering::Release);
    }
}

pub struct LifecycleClient {
    connection: Connection,
}

impl LifecycleClient {
    pub fn connect() -> Result<Self, LifecycleError> {
        Ok(Self {
            connection: Connection::session()?,
        })
    }

    pub fn service_is_running(&self) -> Result<bool, LifecycleError> {
        let proxy = zbus::blocking::fdo::DBusProxy::new(&self.connection)?;
        let name = BusName::try_from(APPLICATION_ID)?;
        Ok(proxy.name_has_owner(name)?)
    }

    pub fn attach_standard_streams(&self) -> Result<(), LifecycleError> {
        let input = io::stdin();
        let output = io::stdout();
        let proxy = Proxy::new(
            &self.connection,
            APPLICATION_ID,
            LIFECYCLE_OBJECT_PATH,
            LIFECYCLE_INTERFACE,
        )?;
        let input_fd = Fd::from(&input);
        let output_fd = Fd::from(&output);
        let completion: OwnedFd = proxy.call("AttachNativeStream", &(input_fd, output_fd))?;
        let mut completion = File::from(std::os::fd::OwnedFd::from(completion));
        let mut marker = [0_u8; 1];
        loop {
            match completion.read(&mut marker) {
                Ok(0) => break,
                Ok(_) => {}
                Err(source) if source.kind() == io::ErrorKind::Interrupted => {}
                Err(source) => return Err(LifecycleError::Completion(source)),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ClaimError {
    #[error("thunderbird-tray is already running in this user session")]
    AlreadyRunning,
    #[error("could not own the thunderbird-tray session service: {0}")]
    Bus(#[source] zbus::Error),
}

#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("session D-Bus operation failed: {0}")]
    Bus(#[from] zbus::Error),
    #[error("session D-Bus request failed: {0}")]
    Fdo(#[from] zbus::fdo::Error),
    #[error("invalid lifecycle bus name: {0}")]
    InvalidBusName(#[from] zbus::names::Error),
    #[error("could not wait for the primary process to finish the native session: {0}")]
    Completion(#[source] io::Error),
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::os::fd::OwnedFd as StdOwnedFd;
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn macro_interface_literal_matches_the_identifier_registry() {
        assert_eq!(
            LIFECYCLE_INTERFACE,
            "io.github.apophuy.thunderbird_tray.Lifecycle"
        );
        assert_eq!(APPLICATION_ID, "io.github.apophuy.thunderbird-tray");
        assert_eq!(
            LIFECYCLE_OBJECT_PATH,
            "/io/github/apophuy/thunderbird_tray/Lifecycle"
        );
    }

    #[test]
    fn only_one_native_stream_is_accepted_until_the_session_finishes() {
        let (attached_tx, attached_rx) = mpsc::channel();
        let session_active = Arc::new(AtomicBool::new(false));
        let interface = LifecycleInterface {
            attached_streams: attached_tx,
            session_active: Arc::clone(&session_active),
        };

        let (input, _input_peer) = UnixStream::pair().unwrap();
        let (output, _output_peer) = UnixStream::pair().unwrap();
        let completion = interface
            .attach_native_stream(
                OwnedFd::from(StdOwnedFd::from(input)),
                OwnedFd::from(StdOwnedFd::from(output)),
            )
            .unwrap();

        let (duplicate_input, _duplicate_input_peer) = UnixStream::pair().unwrap();
        let (duplicate_output, _duplicate_output_peer) = UnixStream::pair().unwrap();
        let duplicate = interface.attach_native_stream(
            OwnedFd::from(StdOwnedFd::from(duplicate_input)),
            OwnedFd::from(StdOwnedFd::from(duplicate_output)),
        );
        assert!(duplicate.is_err());

        let attached = attached_rx.recv().unwrap();
        session_active.store(false, Ordering::Release);
        drop(attached.completion_guard);

        let mut completion = File::from(StdOwnedFd::from(completion));
        let mut marker = [0_u8; 1];
        assert_eq!(completion.read(&mut marker).unwrap(), 0);

        let (reconnected_input, _reconnected_input_peer) = UnixStream::pair().unwrap();
        let (reconnected_output, _reconnected_output_peer) = UnixStream::pair().unwrap();
        let reconnected_completion = interface
            .attach_native_stream(
                OwnedFd::from(StdOwnedFd::from(reconnected_input)),
                OwnedFd::from(StdOwnedFd::from(reconnected_output)),
            )
            .unwrap();
        assert!(session_active.load(Ordering::Acquire));
        drop(attached_rx.recv().unwrap());
        drop(reconnected_completion);
    }
}
