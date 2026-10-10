use crate::{LogEntry, LogRing};
use mechkeymesh_driver_api::{
    DeviceDescriptor, DeviceSession, DriverError, KeyboardDriver, LightingConfig,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceState {
    Disconnected,
    Detecting,
    Opening,
    Ready,
    Busy,
    Recovery,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub state: DeviceState,
    pub device: Option<DeviceDescriptor>,
    pub last_error: Option<String>,
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            state: DeviceState::Disconnected,
            device: None,
            last_error: None,
        }
    }
}

enum Command {
    Reconnect,
    SetLighting {
        config: LightingConfig,
        reply: Sender<Result<(), String>>,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct DeviceManagerHandle {
    tx: Sender<Command>,
    status: Arc<RwLock<DeviceStatus>>,
    log: LogRing,
}

impl DeviceManagerHandle {
    pub fn status(&self) -> DeviceStatus {
        self.status.read().expect("status lock poisoned").clone()
    }

    pub fn logs(&self) -> Vec<LogEntry> {
        self.log.snapshot()
    }

    pub fn reconnect(&self) {
        let _ = self.tx.send(Command::Reconnect);
    }

    pub fn set_lighting(&self, config: LightingConfig) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::SetLighting {
                config,
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;

        reply_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "device command timed out".to_string())?
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

pub struct DeviceManager;

impl DeviceManager {
    pub fn start(drivers: Vec<Arc<dyn KeyboardDriver>>) -> DeviceManagerHandle {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(RwLock::new(DeviceStatus::default()));
        let log = LogRing::new(400);

        let handle = DeviceManagerHandle {
            tx,
            status: status.clone(),
            log: log.clone(),
        };

        thread::Builder::new()
            .name("mechkeymesh-device-manager".into())
            .spawn(move || worker(rx, status, log, drivers))
            .expect("failed to start device manager");

        handle
    }
}

fn worker(
    rx: Receiver<Command>,
    status: Arc<RwLock<DeviceStatus>>,
    log: LogRing,
    drivers: Vec<Arc<dyn KeyboardDriver>>,
) {
    log.push("info", "device manager started");
    let mut session: Option<Box<dyn DeviceSession>> = None;

    loop {
        if session.is_none() {
            discover(&mut session, &status, &log, &drivers);
        }

        match rx.recv_timeout(Duration::from_millis(1200)) {
            Ok(Command::Shutdown) => {
                if let Some(mut device) = session.take() {
                    let _ = device.close();
                }
                log.push("info", "device manager stopped");
                return;
            }
            Ok(Command::Reconnect) => {
                if let Some(mut device) = session.take() {
                    let _ = device.close();
                }
                update_status(&status, DeviceState::Disconnected, None, None);
                log.push("info", "manual reconnect requested");
            }
            Ok(Command::SetLighting { config, reply }) => {
                let result = if let Some(device) = session.as_mut() {
                    let descriptor = device.descriptor();
                    update_status(
                        &status,
                        DeviceState::Busy,
                        Some(descriptor.clone()),
                        None,
                    );
                    log.push("info", "applying onboard lighting");
                    match device.set_lighting(config) {
                        Ok(()) => {
                            update_status(
                                &status,
                                DeviceState::Ready,
                                Some(descriptor),
                                None,
                            );
                            log.push("info", "onboard lighting applied");
                            Ok(())
                        }
                        Err(error) => {
                            let message = error.to_string();
                            log.push("warning", format!("lighting failed: {message}"));
                            update_status(
                                &status,
                                DeviceState::Recovery,
                                Some(descriptor),
                                Some(message.clone()),
                            );
                            if let Some(mut failed) = session.take() {
                                let _ = failed.close();
                            }
                            Err(message)
                        }
                    }
                } else {
                    Err("no supported keyboard is ready".to_string())
                };
                let _ = reply.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn discover(
    session: &mut Option<Box<dyn DeviceSession>>,
    status: &Arc<RwLock<DeviceStatus>>,
    log: &LogRing,
    drivers: &[Arc<dyn KeyboardDriver>],
) {
    update_status(status, DeviceState::Detecting, None, None);

    let mut saw_supported_platform = false;
    for driver in drivers {
        update_status(status, DeviceState::Opening, None, None);
        match driver.open() {
            Ok(opened) => {
                let descriptor = opened.descriptor();
                log.push(
                    "info",
                    format!(
                        "{} connected through driver {}",
                        descriptor.name,
                        driver.id()
                    ),
                );
                update_status(
                    status,
                    DeviceState::Ready,
                    Some(descriptor),
                    None,
                );
                *session = Some(opened);
                return;
            }
            Err(DriverError::NotFound) => {
                saw_supported_platform = true;
            }
            Err(DriverError::UnsupportedPlatform) => {}
            Err(error) => {
                saw_supported_platform = true;
                log.push(
                    "warning",
                    format!("{} probe failed: {error}", driver.display_name()),
                );
            }
        }
    }

    if saw_supported_platform {
        update_status(status, DeviceState::Disconnected, None, None);
    } else {
        update_status(
            status,
            DeviceState::Unsupported,
            None,
            Some("no installed driver supports this platform".into()),
        );
    }
}

fn update_status(
    status: &Arc<RwLock<DeviceStatus>>,
    state: DeviceState,
    device: Option<DeviceDescriptor>,
    last_error: Option<String>,
) {
    *status.write().expect("status lock poisoned") = DeviceStatus {
        state,
        device,
        last_error,
    };
}
