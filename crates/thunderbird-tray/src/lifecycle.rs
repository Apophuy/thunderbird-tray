// SPDX-License-Identifier: GPL-3.0-only

//! Session-bus ownership and Native Messaging stream handoff.

use std::fs::File;
use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;
use zbus::blocking::connection::Builder;
use zbus::blocking::{Connection, Proxy};
use zbus::names::BusName;
use zbus::zvariant::{Fd, OwnedFd};

pub const APPLICATION_ID: &str = env!("THUNDERBIRD_TRAY_APPLICATION_ID");
pub const LIFECYCLE_INTERFACE: &str = env!("THUNDERBIRD_TRAY_LIFECYCLE_INTERFACE");
pub const LIFECYCLE_OBJECT_PATH: &str = env!("THUNDERBIRD_TRAY_LIFECYCLE_OBJECT_PATH");
pub const DETACH_SERVICE_ENVIRONMENT: &str = "THUNDERBIRD_TRAY_DETACH_SERVICE";

const SERVICE_START_TIMEOUT: Duration = Duration::from_secs(3);
const SERVICE_START_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub struct AttachedNativeStream {
    pub input: File,
    pub output: File,
    pub completion_guard: UnixStream,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowActionReport {
    pub request_id: String,
    pub outcome: String,
}

struct LifecycleInterface {
    attached_streams: Sender<AttachedNativeStream>,
    window_reports: Sender<WindowActionReport>,
    configuration_changes: Sender<()>,
    session_active: Arc<AtomicBool>,
}

#[zbus::interface(name = "io.github.apophuy.thunderbird_tray.Lifecycle")]
impl LifecycleInterface {
    fn native_session_active(&self) -> bool {
        self.session_active.load(Ordering::Acquire)
    }

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

    fn report_window_action(&self, request_id: String, outcome: String) -> zbus::fdo::Result<()> {
        if request_id.len() > 64 || outcome.len() > 32 {
            return Err(zbus::fdo::Error::InvalidArgs(
                "window action report is too large".to_owned(),
            ));
        }
        self.window_reports
            .send(WindowActionReport {
                request_id,
                outcome,
            })
            .map_err(|_| {
                zbus::fdo::Error::Failed(
                    "the primary thunderbird-tray process is shutting down".to_owned(),
                )
            })
    }

    fn configuration_changed(&self) -> zbus::fdo::Result<()> {
        self.configuration_changes.send(()).map_err(|_| {
            zbus::fdo::Error::Failed(
                "the primary thunderbird-tray process is shutting down".to_owned(),
            )
        })
    }
}

pub struct LifecycleService {
    _connection: Connection,
    session_active: Arc<AtomicBool>,
}

impl LifecycleService {
    pub fn claim(
        attached_streams: Sender<AttachedNativeStream>,
        window_reports: Sender<WindowActionReport>,
        configuration_changes: Sender<()>,
    ) -> Result<Self, ClaimError> {
        let session_active = Arc::new(AtomicBool::new(false));
        let interface = LifecycleInterface {
            attached_streams,
            window_reports,
            configuration_changes,
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

    pub fn ensure_service_running(&self, config_path: Option<&Path>) -> Result<(), LifecycleError> {
        if self.service_is_running()? {
            return Ok(());
        }

        let executable = std::env::current_exe().map_err(LifecycleError::CurrentExecutable)?;
        service_command(&executable, config_path)
            .spawn()
            .map_err(LifecycleError::StartService)?;

        let deadline = Instant::now() + SERVICE_START_TIMEOUT;
        while Instant::now() < deadline {
            if self.service_is_running()? {
                return Ok(());
            }
            thread::sleep(SERVICE_START_POLL_INTERVAL);
        }
        Err(LifecycleError::ServiceStartTimeout)
    }

    pub fn native_session_active(&self) -> Result<bool, LifecycleError> {
        let proxy = Proxy::new(
            &self.connection,
            APPLICATION_ID,
            LIFECYCLE_OBJECT_PATH,
            LIFECYCLE_INTERFACE,
        )?;
        Ok(proxy.call("NativeSessionActive", &())?)
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

    pub fn notify_configuration_changed(&self) -> Result<(), LifecycleError> {
        let proxy = Proxy::new(
            &self.connection,
            APPLICATION_ID,
            LIFECYCLE_OBJECT_PATH,
            LIFECYCLE_INTERFACE,
        )?;
        let _: () = proxy.call("ConfigurationChanged", &())?;
        Ok(())
    }
}

fn service_command(executable: &Path, config_path: Option<&Path>) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("--service")
        .env(DETACH_SERVICE_ENVIRONMENT, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(config_path) = config_path {
        command.arg("--config").arg(config_path);
    }
    command
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
    #[error("could not resolve the thunderbird-tray executable: {0}")]
    CurrentExecutable(#[source] io::Error),
    #[error("could not start the thunderbird-tray service: {0}")]
    StartService(#[source] io::Error),
    #[error("timed out waiting for the thunderbird-tray service to start")]
    ServiceStartTimeout,
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
    fn service_launch_is_detached_and_preserves_an_explicit_config_path() {
        let command = service_command(
            Path::new("/opt/thunderbird-tray/bin/thunderbird-tray"),
            Some(Path::new("/tmp/settings/config.toml")),
        );
        assert_eq!(
            command.get_program(),
            "/opt/thunderbird-tray/bin/thunderbird-tray"
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--service", "--config", "/tmp/settings/config.toml"]
        );
        assert!(command.get_envs().any(|(name, value)| {
            name == DETACH_SERVICE_ENVIRONMENT && value.is_some_and(|value| value == "1")
        }));
    }

    #[test]
    fn only_one_native_stream_is_accepted_until_the_session_finishes() {
        let (attached_tx, attached_rx) = mpsc::channel();
        let (window_tx, _window_rx) = mpsc::channel();
        let (configuration_tx, _configuration_rx) = mpsc::channel();
        let session_active = Arc::new(AtomicBool::new(false));
        let interface = LifecycleInterface {
            attached_streams: attached_tx,
            window_reports: window_tx,
            configuration_changes: configuration_tx,
            session_active: Arc::clone(&session_active),
        };
        assert!(!interface.native_session_active());

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
        assert!(interface.native_session_active());
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

    #[test]
    fn window_action_reports_are_bounded_and_forwarded() {
        let (attached_tx, _attached_rx) = mpsc::channel();
        let (window_tx, window_rx) = mpsc::channel();
        let (configuration_tx, _configuration_rx) = mpsc::channel();
        let interface = LifecycleInterface {
            attached_streams: attached_tx,
            window_reports: window_tx,
            configuration_changes: configuration_tx,
            session_active: Arc::new(AtomicBool::new(false)),
        };

        interface
            .report_window_action("123-4".to_owned(), "activated".to_owned())
            .unwrap();
        assert_eq!(
            window_rx.recv().unwrap(),
            WindowActionReport {
                request_id: "123-4".to_owned(),
                outcome: "activated".to_owned(),
            }
        );

        assert!(
            interface
                .report_window_action("x".repeat(65), "found".to_owned())
                .is_err()
        );
        assert!(
            interface
                .report_window_action("123-5".to_owned(), "x".repeat(33))
                .is_err()
        );
    }

    #[test]
    fn configuration_changes_are_forwarded() {
        let (attached_tx, _attached_rx) = mpsc::channel();
        let (window_tx, _window_rx) = mpsc::channel();
        let (configuration_tx, configuration_rx) = mpsc::channel();
        let interface = LifecycleInterface {
            attached_streams: attached_tx,
            window_reports: window_tx,
            configuration_changes: configuration_tx,
            session_active: Arc::new(AtomicBool::new(false)),
        };

        interface.configuration_changed().unwrap();
        configuration_rx.recv().unwrap();
    }
}
