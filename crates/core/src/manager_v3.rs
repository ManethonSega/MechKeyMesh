use crate::{DriverRegistry, LogEntry, LogRing};
use mechkeymesh_driver_api::{
    CancellationToken, Capability, DeviceCandidate, DeviceClock, DeviceDescriptor, DeviceSession,
    DeviceSettings, DriverError, DriverManifest, ErrorClass, HardwareMacroProgram, KeyAssignment,
    KeyboardDriver, KeymapLayer, LcdUpload, LightingConfig, LiveRgbFrame, OperationContext,
    PerKeyRgbConfig, PerKeyRgbReadback,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceState {
    Disconnected,
    Detecting,
    Opening,
    Initializing,
    Ready,
    Busy,
    Recovery,
    Ambiguous,
    Unsupported,
    Suspended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveOperation {
    pub id: u64,
    pub name: String,
    pub started_unix_millis: u128,
    pub timeout_millis: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub state: DeviceState,
    pub device: Option<DeviceDescriptor>,
    pub last_error: Option<String>,
    pub last_error_class: Option<ErrorClass>,
    pub active_operation: Option<ActiveOperation>,
    pub candidate_count: usize,
    pub reconnect_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRgbTransportStatus {
    pub active: bool,
    pub submitted_frames: u64,
    pub sent_frames: u64,
    pub coalesced_frames: u64,
    pub failed_frames: u64,
    pub buffered_frames: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcdTransferStatus {
    pub active: bool,
    pub pages_sent: usize,
    pub total_pages: usize,
    pub incomplete: bool,
    pub last_error: Option<String>,
}

#[derive(Default)]
struct LiveRgbTelemetry {
    active: AtomicBool,
    submitted_frames: AtomicU64,
    sent_frames: AtomicU64,
    coalesced_frames: AtomicU64,
    failed_frames: AtomicU64,
    last_error: Mutex<Option<String>>,
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            state: DeviceState::Disconnected,
            device: None,
            last_error: None,
            last_error_class: None,
            active_operation: None,
            candidate_count: 0,
            reconnect_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub suspend_guard: Arc<AtomicBool>,
    pub discovery_timeout: Duration,
    pub discovery_interval: Duration,
    pub open_timeout: Duration,
    pub initialization_timeout: Duration,
    pub health_timeout: Duration,
    pub health_interval: Duration,
    pub command_timeout: Duration,
    pub macro_upload_timeout: Duration,
    pub lcd_upload_timeout: Duration,
    pub open_attempts: usize,
    pub retry_delay: Duration,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            suspend_guard: Arc::new(AtomicBool::new(false)),
            discovery_timeout: Duration::from_secs(2),
            discovery_interval: Duration::from_millis(900),
            open_timeout: Duration::from_secs(2),
            initialization_timeout: Duration::from_secs(2),
            health_timeout: Duration::from_millis(700),
            health_interval: Duration::from_millis(900),
            command_timeout: Duration::from_secs(3),
            macro_upload_timeout: Duration::from_secs(15),
            lcd_upload_timeout: Duration::from_secs(20 * 60),
            open_attempts: 2,
            retry_delay: Duration::from_millis(120),
        }
    }
}

struct ActiveDevice {
    driver: Arc<dyn KeyboardDriver>,
    candidate: DeviceCandidate,
    session: Box<dyn DeviceSession>,
}

enum Command {
    Reconnect,
    Suspend,
    Resume,
    SetLighting {
        id: u64,
        config: LightingConfig,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
    SetKeymap {
        id: u64,
        layer: KeymapLayer,
        assignments: Vec<KeyAssignment>,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
    SetMacros {
        id: u64,
        programs: Vec<HardwareMacroProgram>,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
    SetDeviceSettings {
        id: u64,
        settings: DeviceSettings,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
    SetPerKeyRgb {
        id: u64,
        config: PerKeyRgbConfig,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
    ReadPerKeyRgb {
        id: u64,
        led_indices: Vec<u16>,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<PerKeyRgbReadback, String>>,
    },
    UploadLcd {
        id: u64,
        upload: LcdUpload,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
    SyncClock {
        id: u64,
        clock: DeviceClock,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
    LiveFrameReady,
    StopLiveRgb,
    Shutdown,
}

#[derive(Clone)]
pub struct DeviceManagerHandle {
    tx: Sender<Command>,
    status: Arc<RwLock<DeviceStatus>>,
    log: LogRing,
    manifests: Arc<Vec<DriverManifest>>,
    active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    next_operation_id: Arc<AtomicU64>,
    command_timeout: Duration,
    macro_upload_timeout: Duration,
    lcd_upload_timeout: Duration,
    lcd_transfer: Arc<RwLock<LcdTransferStatus>>,
    live_frame: Arc<Mutex<Option<LiveRgbFrame>>>,
    live_telemetry: Arc<LiveRgbTelemetry>,
    suspended: Arc<AtomicBool>,
}

impl DeviceManagerHandle {
    pub fn status(&self) -> DeviceStatus {
        self.status.read().expect("status lock poisoned").clone()
    }

    pub fn logs(&self) -> Vec<LogEntry> {
        self.log.snapshot()
    }

    pub fn drivers(&self) -> Vec<DriverManifest> {
        self.manifests.as_ref().clone()
    }

    pub fn reconnect(&self) {
        self.cancel_active_operation();
        let _ = self.tx.send(Command::Reconnect);
    }

    pub fn suspend(&self) {
        self.suspended.store(true, Ordering::Release);
        self.cancel_active_operation();
        self.stop_live_rgb();
        let _ = self.tx.send(Command::Suspend);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(Command::Resume);
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended.load(Ordering::Acquire)
    }

    pub fn cancel_active_operation(&self) -> bool {
        let guard = self
            .active_cancellation
            .lock()
            .expect("active cancellation lock poisoned");
        if let Some(token) = guard.as_ref() {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn set_lighting(&self, config: LightingConfig) -> Result<(), String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::SetLighting {
                id,
                config,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.command_timeout)
    }

    pub fn set_keymap(
        &self,
        layer: KeymapLayer,
        assignments: Vec<KeyAssignment>,
    ) -> Result<(), String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::SetKeymap {
                id,
                layer,
                assignments,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.command_timeout)
    }

    pub fn set_macros(&self, programs: Vec<HardwareMacroProgram>) -> Result<(), String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::SetMacros {
                id,
                programs,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.macro_upload_timeout)
    }

    pub fn set_device_settings(&self, settings: DeviceSettings) -> Result<(), String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::SetDeviceSettings {
                id,
                settings,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.command_timeout)
    }

    pub fn set_per_key_rgb(&self, config: PerKeyRgbConfig) -> Result<(), String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::SetPerKeyRgb {
                id,
                config,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.command_timeout)
    }

    pub fn read_per_key_rgb(&self, led_indices: Vec<u16>) -> Result<PerKeyRgbReadback, String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::ReadPerKeyRgb {
                id,
                led_indices,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.command_timeout)
    }

    pub fn upload_lcd(&self, upload: LcdUpload) -> Result<(), String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::UploadLcd {
                id,
                upload,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.lcd_upload_timeout)
    }

    pub fn sync_clock(&self, clock: DeviceClock) -> Result<(), String> {
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let id = self.next_operation_id.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::new();
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Command::SyncClock {
                id,
                clock,
                submitted: Instant::now(),
                cancellation: cancellation.clone(),
                reply: reply_tx,
            })
            .map_err(|_| "device manager is not running".to_string())?;
        wait_for_reply(reply_rx, cancellation, self.command_timeout)
    }

    pub fn lcd_transfer_status(&self) -> LcdTransferStatus {
        self.lcd_transfer
            .read()
            .expect("LCD transfer status lock poisoned")
            .clone()
    }

    pub fn submit_live_rgb(&self, frame: LiveRgbFrame) -> Result<(), String> {
        if let Some(error) = self
            .live_telemetry
            .last_error
            .lock()
            .expect("live RGB error lock poisoned")
            .as_ref()
        {
            return Err(format!(
                "Live RGB stopped after a transport failure. Press Start to retry. {error}"
            ));
        }
        if self.is_suspended() {
            return Err("Keyboard I/O is paused while Windows sleeps.".into());
        }
        let status = self.status();
        let Some(device) = status.device else {
            return Err("no supported keyboard is ready".into());
        };
        // Persistent operations temporarily own the transport. Drop this frame
        // without terminating its renderer or accumulating a queue.
        if status.state == DeviceState::Busy {
            return Ok(());
        }
        if status.state != DeviceState::Ready || !device.capabilities.contains(&Capability::LiveRgb)
        {
            return Err("live RGB is not available on the active connection".into());
        }

        self.live_telemetry
            .submitted_frames
            .fetch_add(1, Ordering::Relaxed);
        self.live_telemetry.active.store(true, Ordering::Release);
        let mut slot = self.live_frame.lock().expect("live frame lock poisoned");
        let signal = slot.is_none();
        if !signal {
            self.live_telemetry
                .coalesced_frames
                .fetch_add(1, Ordering::Relaxed);
        }
        *slot = Some(frame);
        drop(slot);

        if signal && self.tx.send(Command::LiveFrameReady).is_err() {
            self.live_frame
                .lock()
                .expect("live frame lock poisoned")
                .take();
            return Err("device manager is not running".into());
        }
        Ok(())
    }

    pub fn stop_live_rgb(&self) {
        self.live_telemetry.active.store(false, Ordering::Release);
        self.live_frame
            .lock()
            .expect("live frame lock poisoned")
            .take();
        let _ = self.tx.send(Command::StopLiveRgb);
    }

    pub fn reset_live_rgb_failure(&self) {
        *self
            .live_telemetry
            .last_error
            .lock()
            .expect("live RGB error lock poisoned") = None;
    }

    pub fn live_rgb_status(&self) -> LiveRgbTransportStatus {
        LiveRgbTransportStatus {
            active: self.live_telemetry.active.load(Ordering::Acquire),
            submitted_frames: self.live_telemetry.submitted_frames.load(Ordering::Relaxed),
            sent_frames: self.live_telemetry.sent_frames.load(Ordering::Relaxed),
            coalesced_frames: self.live_telemetry.coalesced_frames.load(Ordering::Relaxed),
            failed_frames: self.live_telemetry.failed_frames.load(Ordering::Relaxed),
            last_error: self
                .live_telemetry
                .last_error
                .lock()
                .expect("live RGB error lock poisoned")
                .clone(),
            buffered_frames: if self
                .live_frame
                .lock()
                .expect("live frame lock poisoned")
                .is_some()
            {
                1
            } else {
                0
            },
        }
    }

    pub fn shutdown(&self) {
        self.cancel_active_operation();
        let _ = self.tx.send(Command::Shutdown);
    }
}

fn wait_for_reply<T>(
    receiver: Receiver<Result<T, String>>,
    cancellation: CancellationToken,
    timeout: Duration,
) -> Result<T, String> {
    match receiver.recv_timeout(timeout + Duration::from_millis(250)) {
        Ok(result) => result,
        Err(_) => {
            cancellation.cancel();
            Err(format!(
                "device command timed out after {} ms",
                timeout.as_millis()
            ))
        }
    }
}

pub struct DeviceManager;

impl DeviceManager {
    pub fn start(registry: DriverRegistry, config: ManagerConfig) -> DeviceManagerHandle {
        let manifests = Arc::new(registry.manifests());
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(RwLock::new(DeviceStatus::default()));
        let log = LogRing::new(600);
        let active_cancellation = Arc::new(Mutex::new(None));
        let live_frame = Arc::new(Mutex::new(None));
        let live_telemetry = Arc::new(LiveRgbTelemetry::default());
        let lcd_transfer = Arc::new(RwLock::new(LcdTransferStatus::default()));
        let suspended = config.suspend_guard.clone();

        let handle = DeviceManagerHandle {
            tx,
            status: status.clone(),
            log: log.clone(),
            manifests,
            active_cancellation: active_cancellation.clone(),
            next_operation_id: Arc::new(AtomicU64::new(1)),
            command_timeout: config.command_timeout,
            macro_upload_timeout: config.macro_upload_timeout,
            lcd_upload_timeout: config.lcd_upload_timeout,
            lcd_transfer: lcd_transfer.clone(),
            live_frame: live_frame.clone(),
            live_telemetry: live_telemetry.clone(),
            suspended: suspended.clone(),
        };

        thread::Builder::new()
            .name("mechkeymesh-device-manager".into())
            .spawn(move || {
                worker(
                    rx,
                    status,
                    log,
                    registry,
                    active_cancellation,
                    live_frame,
                    live_telemetry,
                    lcd_transfer,
                    suspended,
                    config,
                )
            })
            .expect("failed to start device manager");
        handle
    }
}

#[allow(clippy::too_many_arguments)]
fn worker(
    rx: Receiver<Command>,
    status: Arc<RwLock<DeviceStatus>>,
    log: LogRing,
    registry: DriverRegistry,
    active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    live_frame: Arc<Mutex<Option<LiveRgbFrame>>>,
    live_telemetry: Arc<LiveRgbTelemetry>,
    lcd_transfer: Arc<RwLock<LcdTransferStatus>>,
    suspended: Arc<AtomicBool>,
    config: ManagerConfig,
) {
    log.push("info", "Device Manager 3.0 started");
    let mut active: Option<ActiveDevice> = None;
    let mut ever_connected = false;

    loop {
        if suspended.load(Ordering::Acquire) {
            close_active(&mut active, &log);
            live_frame.lock().expect("live frame lock poisoned").take();
            live_telemetry.active.store(false, Ordering::Release);
            set_status(&status, DeviceState::Suspended, None, None, None, 0);
        } else if active.is_none() {
            active = discover_and_open(&registry, &status, &log, &config, &mut ever_connected);
        }

        let wait = if active.is_some() {
            config.health_interval
        } else {
            config.discovery_interval
        };

        match rx.recv_timeout(wait) {
            Ok(Command::Suspend) => {
                close_active(&mut active, &log);
                log.push(
                    "info",
                    "Windows suspend: I/O cancelled and device handles released",
                );
            }
            Ok(Command::Resume) => {
                suspended.store(false, Ordering::Release);
                set_status(&status, DeviceState::Disconnected, None, None, None, 0);
                log.push("info", "Windows resume: rediscovery requested");
            }
            Ok(Command::Shutdown) => {
                close_active(&mut active, &log);
                log.push("info", "device manager stopped");
                return;
            }
            Ok(Command::Reconnect) => {
                live_telemetry.active.store(false, Ordering::Release);
                live_frame.lock().expect("live frame lock poisoned").take();
                close_active(&mut active, &log);
                set_status(&status, DeviceState::Disconnected, None, None, None, 0);
                log.push("info", "manual reconnect requested");
            }
            Ok(Command::SetLighting {
                id,
                config: lighting,
                submitted,
                cancellation,
                reply,
            }) => {
                let result = execute_operation(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    id,
                    "apply built-in lighting",
                    submitted,
                    cancellation,
                    |session, context| session.set_lighting(context, lighting),
                );
                let _ = reply.send(result);
            }
            Ok(Command::SetKeymap {
                id,
                layer,
                assignments,
                submitted,
                cancellation,
                reply,
            }) => {
                let name = match layer {
                    KeymapLayer::Normal => "apply normal-layer keymap",
                    KeymapLayer::Fn => "apply Fn-layer keymap",
                };
                let result = execute_operation(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    id,
                    name,
                    submitted,
                    cancellation,
                    |session, context| session.set_keymap(context, layer, &assignments),
                );
                let _ = reply.send(result);
            }
            Ok(Command::SetMacros {
                id,
                programs,
                submitted,
                cancellation,
                reply,
            }) => {
                let result = execute_operation_with_result(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    config.macro_upload_timeout,
                    id,
                    "upload hardware macro bank",
                    submitted,
                    cancellation,
                    |session, context| session.set_macros(context, &programs),
                );
                let _ = reply.send(result);
            }
            Ok(Command::SetDeviceSettings {
                id,
                settings,
                submitted,
                cancellation,
                reply,
            }) => {
                let result = execute_operation(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    id,
                    "apply device settings",
                    submitted,
                    cancellation,
                    |session, context| session.set_device_settings(context, &settings),
                );
                let _ = reply.send(result);
            }
            Ok(Command::SetPerKeyRgb {
                id,
                config: per_key,
                submitted,
                cancellation,
                reply,
            }) => {
                let result = execute_operation(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    id,
                    "apply stored per-key RGB",
                    submitted,
                    cancellation,
                    |session, context| session.set_per_key_rgb(context, &per_key),
                );
                let _ = reply.send(result);
            }
            Ok(Command::ReadPerKeyRgb {
                id,
                led_indices,
                submitted,
                cancellation,
                reply,
            }) => {
                let result = execute_operation_with_result(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    config.command_timeout,
                    id,
                    "read stored per-key RGB",
                    submitted,
                    cancellation,
                    |session, context| session.read_per_key_rgb(context, &led_indices),
                );
                let _ = reply.send(result);
            }
            Ok(Command::UploadLcd {
                id,
                upload,
                submitted,
                cancellation,
                reply,
            }) => {
                let total_pages = upload.payload.len() / 4096;
                *lcd_transfer
                    .write()
                    .expect("LCD transfer status lock poisoned") = LcdTransferStatus {
                    active: true,
                    pages_sent: 0,
                    total_pages,
                    incomplete: false,
                    last_error: None,
                };
                let progress_status = lcd_transfer.clone();
                let result = execute_operation_with_result(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    config.lcd_upload_timeout,
                    id,
                    "upload LCD media",
                    submitted,
                    cancellation,
                    |session, context| {
                        let mut progress = |sent, total| {
                            let mut transfer = progress_status
                                .write()
                                .expect("LCD transfer status lock poisoned");
                            transfer.pages_sent = sent;
                            transfer.total_pages = total;
                        };
                        session.upload_lcd(context, &upload, &mut progress)
                    },
                );
                {
                    let mut transfer = lcd_transfer
                        .write()
                        .expect("LCD transfer status lock poisoned");
                    transfer.active = false;
                    // Apply can fail after every page was acknowledged, too.
                    transfer.incomplete = result.is_err();
                    transfer.last_error = result.as_ref().err().cloned();
                }
                let _ = reply.send(result);
            }
            Ok(Command::SyncClock {
                id,
                clock,
                submitted,
                cancellation,
                reply,
            }) => {
                let result = execute_operation(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    id,
                    "sync LCD clock",
                    submitted,
                    cancellation,
                    |session, context| session.sync_clock(context, clock),
                );
                let _ = reply.send(result);
            }
            Ok(Command::LiveFrameReady) => {
                if !live_telemetry.active.load(Ordering::Acquire) {
                    live_frame.lock().expect("live frame lock poisoned").take();
                    continue;
                }
                let frame = live_frame.lock().expect("live frame lock poisoned").take();
                if let Some(frame) = frame {
                    send_live_frame(&mut active, &status, &log, &live_telemetry, &config, &frame);
                }
            }
            Ok(Command::StopLiveRgb) => {
                live_frame.lock().expect("live frame lock poisoned").take();
                live_telemetry.active.store(false, Ordering::Release);
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(device) = active.as_mut() {
                    if !active_device_present(device, &log, &config) {
                        let descriptor = device.session.descriptor();
                        set_status(
                            &status,
                            DeviceState::Recovery,
                            Some(descriptor),
                            Some((ErrorClass::Disconnected, "device removal detected".into())),
                            None,
                            0,
                        );
                        close_active(&mut active, &log);
                    } else {
                        status.write().expect("status lock poisoned").device =
                            Some(device.session.descriptor());
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                close_active(&mut active, &log);
                return;
            }
        }
    }
}

fn send_live_frame(
    active: &mut Option<ActiveDevice>,
    status: &Arc<RwLock<DeviceStatus>>,
    log: &LogRing,
    telemetry: &LiveRgbTelemetry,
    config: &ManagerConfig,
    frame: &LiveRgbFrame,
) {
    let Some(device) = active.as_mut() else {
        telemetry.failed_frames.fetch_add(1, Ordering::Relaxed);
        return;
    };
    if telemetry
        .last_error
        .lock()
        .expect("live RGB error lock poisoned")
        .is_some()
    {
        return;
    }
    if !device
        .session
        .descriptor()
        .capabilities
        .contains(&Capability::LiveRgb)
    {
        telemetry.failed_frames.fetch_add(1, Ordering::Relaxed);
        telemetry.active.store(false, Ordering::Release);
        return;
    }

    let timeout = config.command_timeout.min(Duration::from_millis(500));
    let context = OperationContext::new(timeout).with_suspend_guard(config.suspend_guard.clone());
    match device.session.send_live_rgb_frame(&context, frame) {
        Ok(()) => {
            telemetry.sent_frames.fetch_add(1, Ordering::Relaxed);
        }
        Err(error) => {
            telemetry.failed_frames.fetch_add(1, Ordering::Relaxed);
            telemetry.active.store(false, Ordering::Release);
            *telemetry
                .last_error
                .lock()
                .expect("live RGB error lock poisoned") = Some(error.to_string());
            log.push("warning", format!("live RGB frame failed: {error}"));
            if error.requires_recovery() {
                telemetry.active.store(false, Ordering::Release);
                let descriptor = device.session.descriptor();
                set_status(
                    status,
                    DeviceState::Recovery,
                    Some(descriptor),
                    Some((error.class(), error.to_string())),
                    None,
                    0,
                );
                close_active(active, log);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_operation<F>(
    active: &mut Option<ActiveDevice>,
    status: &Arc<RwLock<DeviceStatus>>,
    log: &LogRing,
    active_cancellation: &Arc<Mutex<Option<CancellationToken>>>,
    config: &ManagerConfig,
    id: u64,
    name: &str,
    submitted: Instant,
    cancellation: CancellationToken,
    operation: F,
) -> Result<(), String>
where
    F: FnOnce(&mut dyn DeviceSession, &OperationContext) -> Result<(), DriverError>,
{
    execute_operation_with_result(
        active,
        status,
        log,
        active_cancellation,
        config,
        config.command_timeout,
        id,
        name,
        submitted,
        cancellation,
        operation,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_operation_with_result<T, F>(
    active: &mut Option<ActiveDevice>,
    status: &Arc<RwLock<DeviceStatus>>,
    log: &LogRing,
    active_cancellation: &Arc<Mutex<Option<CancellationToken>>>,
    config: &ManagerConfig,
    command_timeout: Duration,
    id: u64,
    name: &str,
    submitted: Instant,
    cancellation: CancellationToken,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce(&mut dyn DeviceSession, &OperationContext) -> Result<T, DriverError>,
{
    let Some(device) = active.as_mut() else {
        return Err("no supported keyboard is ready".into());
    };

    let elapsed = submitted.elapsed();
    if elapsed >= command_timeout {
        return Err(format!(
            "operation expired in the queue after {} ms",
            elapsed.as_millis()
        ));
    }
    let remaining = command_timeout - elapsed;
    let descriptor = device.session.descriptor();
    let active_operation = ActiveOperation {
        id,
        name: name.into(),
        started_unix_millis: unix_millis(),
        timeout_millis: remaining.as_millis().try_into().unwrap_or(u64::MAX),
    };

    *active_cancellation
        .lock()
        .expect("active cancellation lock poisoned") = Some(cancellation.clone());
    set_status(
        status,
        DeviceState::Busy,
        Some(descriptor.clone()),
        None,
        Some(active_operation),
        1,
    );
    log.push("info", format!("operation {id} started: {name}"));

    let context = OperationContext::with_token(remaining, cancellation)
        .with_suspend_guard(config.suspend_guard.clone());
    let result = operation(device.session.as_mut(), &context);
    *active_cancellation
        .lock()
        .expect("active cancellation lock poisoned") = None;

    match result {
        Ok(value) => {
            set_status(status, DeviceState::Ready, Some(descriptor), None, None, 1);
            log.push("info", format!("operation {id} completed: {name}"));
            Ok(value)
        }
        Err(error) => {
            let class = error.class();
            let message = error.to_string();
            log.push(
                "warning",
                format!("operation {id} failed ({class:?}): {message}"),
            );
            if error.requires_recovery() {
                set_status(
                    status,
                    DeviceState::Recovery,
                    Some(descriptor),
                    Some((class, message.clone())),
                    None,
                    0,
                );
                close_active(active, log);
            } else {
                set_status(
                    status,
                    DeviceState::Ready,
                    Some(descriptor),
                    Some((class, message.clone())),
                    None,
                    1,
                );
            }
            Err(message)
        }
    }
}

fn discover_and_open(
    registry: &DriverRegistry,
    status: &Arc<RwLock<DeviceStatus>>,
    log: &LogRing,
    config: &ManagerConfig,
    ever_connected: &mut bool,
) -> Option<ActiveDevice> {
    set_status(status, DeviceState::Detecting, None, None, None, 0);
    let discovery_context = OperationContext::new(config.discovery_timeout)
        .with_suspend_guard(config.suspend_guard.clone());
    let report = match registry.discover(&discovery_context) {
        Ok(report) => report,
        Err(error) => {
            let class = error.class();
            log.push("warning", format!("device discovery failed: {error}"));
            set_status(
                status,
                DeviceState::Recovery,
                None,
                Some((class, error.to_string())),
                None,
                0,
            );
            return None;
        }
    };

    for failure in &report.failures {
        log.push(
            "warning",
            format!(
                "driver {} probe: {} ({:?})",
                failure.driver_id, failure.message, failure.class
            ),
        );
    }

    let candidate_count = report.candidates.len();
    if candidate_count == 0 {
        let all_unsupported = !report.failures.is_empty()
            && report
                .failures
                .iter()
                .all(|failure| failure.class == ErrorClass::Unsupported);
        set_status(
            status,
            if all_unsupported {
                DeviceState::Unsupported
            } else {
                DeviceState::Disconnected
            },
            None,
            report
                .failures
                .iter()
                .find(|failure| failure.class != ErrorClass::Unsupported)
                .map(|failure| (failure.class, failure.message.clone())),
            None,
            0,
        );
        return None;
    }

    if candidate_count > 1 {
        let message = format!(
            "{candidate_count} supported keyboard instances were detected; automatic selection is blocked"
        );
        log.push("warning", &message);
        set_status(
            status,
            DeviceState::Ambiguous,
            None,
            Some((ErrorClass::DuplicateDevice, message)),
            None,
            candidate_count,
        );
        return None;
    }

    let candidate = report.candidates.into_iter().next().expect("one candidate");
    let Some(driver) = registry.get(&candidate.driver_id) else {
        let message = format!(
            "candidate references unregistered driver {}",
            candidate.driver_id
        );
        log.push("error", &message);
        set_status(
            status,
            DeviceState::Recovery,
            None,
            Some((ErrorClass::Internal, message)),
            None,
            1,
        );
        return None;
    };

    set_status(status, DeviceState::Opening, None, None, None, 1);
    let attempts = config.open_attempts.max(1);
    let mut last_error = None;

    for attempt in 1..=attempts {
        let open_context = OperationContext::new(config.open_timeout)
            .with_suspend_guard(config.suspend_guard.clone());
        match driver.open(&open_context, &candidate) {
            Ok(mut session) => {
                let descriptor = session.descriptor();
                set_status(
                    status,
                    DeviceState::Initializing,
                    Some(descriptor.clone()),
                    None,
                    None,
                    1,
                );
                let initialization_context = OperationContext::new(config.initialization_timeout)
                    .with_suspend_guard(config.suspend_guard.clone());
                match session.initialize(&initialization_context) {
                    Ok(()) => {
                        let descriptor = session.descriptor();
                        if *ever_connected {
                            status
                                .write()
                                .expect("status lock poisoned")
                                .reconnect_count += 1;
                        }
                        *ever_connected = true;
                        set_status(
                            status,
                            DeviceState::Ready,
                            Some(descriptor.clone()),
                            None,
                            None,
                            1,
                        );
                        log.push(
                            "info",
                            format!(
                                "{} ready through driver {}",
                                descriptor.name, candidate.driver_id
                            ),
                        );
                        return Some(ActiveDevice {
                            driver,
                            candidate,
                            session,
                        });
                    }
                    Err(error) => {
                        let _ = session.close();
                        log.push("warning", format!("device initialization failed: {error}"));
                        set_status(
                            status,
                            DeviceState::Recovery,
                            Some(descriptor),
                            Some((error.class(), error.to_string())),
                            None,
                            1,
                        );
                        return None;
                    }
                }
            }
            Err(error) => {
                let retry = attempt < attempts && error.retryable_open();
                log.push(
                    "warning",
                    format!(
                        "open attempt {attempt}/{attempts} failed for {}: {error}",
                        candidate.driver_id
                    ),
                );
                last_error = Some(error);
                if retry {
                    thread::sleep(config.retry_delay);
                } else {
                    break;
                }
            }
        }
    }

    if let Some(error) = last_error {
        let state = if matches!(error, DriverError::NotFound | DriverError::Disconnected(_)) {
            DeviceState::Disconnected
        } else {
            DeviceState::Recovery
        };
        set_status(
            status,
            state,
            None,
            Some((error.class(), error.to_string())),
            None,
            1,
        );
    }
    None
}

fn active_device_present(active: &mut ActiveDevice, log: &LogRing, config: &ManagerConfig) -> bool {
    let context = OperationContext::new(config.health_timeout)
        .with_suspend_guard(config.suspend_guard.clone());
    match active.driver.is_present(&context, &active.candidate) {
        Ok(false) | Err(DriverError::NotFound) | Err(DriverError::Disconnected(_)) => false,
        Ok(true) => match active.session.health_check(&context) {
            Ok(()) | Err(DriverError::Cancelled) => true,
            Err(error) => {
                log.push("warning", format!("device health check failed: {error}"));
                !error.requires_recovery()
            }
        },
        Err(error) => {
            log.push("warning", format!("device presence check failed: {error}"));
            !error.requires_recovery()
        }
    }
}

fn close_active(active: &mut Option<ActiveDevice>, log: &LogRing) {
    if let Some(mut device) = active.take()
        && let Err(error) = device.session.close()
    {
        log.push("warning", format!("device close failed: {error}"));
    }
}

fn set_status(
    status: &Arc<RwLock<DeviceStatus>>,
    state: DeviceState,
    device: Option<DeviceDescriptor>,
    error: Option<(ErrorClass, String)>,
    active_operation: Option<ActiveOperation>,
    candidate_count: usize,
) {
    let mut current = status.write().expect("status lock poisoned");
    current.state = state;
    current.device = device;
    current.last_error_class = error.as_ref().map(|(class, _)| *class);
    current.last_error = error.map(|(_, message)| message);
    current.active_operation = active_operation;
    current.candidate_count = candidate_count;
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::{
        Capability, Connection, DeviceMatch, DriverManifest, KeyAction, Modifier,
        PerKeyRgbAssignment, PerKeyRgbConfig, PerKeyRgbReadback, RgbColor, TransportKind,
        TransportRequirement,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    struct FakeDriver {
        present: Arc<AtomicBool>,
        candidates: Arc<AtomicUsize>,
        max_parallel: Arc<AtomicUsize>,
        active_operations: Arc<AtomicUsize>,
        keymap_calls: Arc<AtomicUsize>,
        per_key_calls: Arc<AtomicUsize>,
        readback_calls: Arc<AtomicUsize>,
        live_calls: Arc<AtomicUsize>,
        live_fail: Arc<AtomicBool>,
        operation_delay: Duration,
    }

    struct FakeSession {
        candidate: DeviceCandidate,
        max_parallel: Arc<AtomicUsize>,
        active_operations: Arc<AtomicUsize>,
        keymap_calls: Arc<AtomicUsize>,
        per_key_calls: Arc<AtomicUsize>,
        readback_calls: Arc<AtomicUsize>,
        live_calls: Arc<AtomicUsize>,
        live_fail: Arc<AtomicBool>,
        operation_delay: Duration,
    }

    impl FakeDriver {
        fn new() -> Self {
            Self {
                present: Arc::new(AtomicBool::new(true)),
                candidates: Arc::new(AtomicUsize::new(1)),
                max_parallel: Arc::new(AtomicUsize::new(0)),
                active_operations: Arc::new(AtomicUsize::new(0)),
                keymap_calls: Arc::new(AtomicUsize::new(0)),
                per_key_calls: Arc::new(AtomicUsize::new(0)),
                readback_calls: Arc::new(AtomicUsize::new(0)),
                live_calls: Arc::new(AtomicUsize::new(0)),
                live_fail: Arc::new(AtomicBool::new(false)),
                operation_delay: Duration::from_millis(10),
            }
        }
    }

    impl KeyboardDriver for FakeDriver {
        fn manifest(&self) -> DriverManifest {
            DriverManifest {
                driver_id: "fake.keyboard.usb".into(),
                device_model_id: "fake.keyboard".into(),
                vendor: "Fake".into(),
                device_name: "Fake Keyboard".into(),
                supported_connections: vec![Connection::Usb],
                capabilities: vec![
                    Capability::Keys,
                    Capability::HardwareRgb,
                    Capability::LiveRgb,
                ],
                layout_ids: vec![],
                matchers: vec![DeviceMatch {
                    vendor_id: 0x1234,
                    product_id: 0x5678,
                    usage_page: Some(0xFF00),
                    interface_number: None,
                }],
                transport_requirements: vec![TransportRequirement {
                    kind: TransportKind::HidFeature,
                    usage_page: Some(0xFF00),
                    interface_number: None,
                    report_size: Some(64),
                }],
            }
        }

        fn discover(
            &self,
            context: &OperationContext,
        ) -> Result<Vec<DeviceCandidate>, DriverError> {
            context.check()?;
            if !self.present.load(Ordering::Acquire) {
                return Ok(vec![]);
            }
            Ok((0..self.candidates.load(Ordering::Acquire))
                .map(|index| DeviceCandidate {
                    driver_id: "fake.keyboard.usb".into(),
                    instance_id: format!("fake-{index}"),
                    name: "Fake Keyboard".into(),
                    connection: Connection::Usb,
                    vendor_id: Some(0x1234),
                    product_id: Some(0x5678),
                    usage_page: Some(0xFF00),
                    interface_number: Some(0),
                    serial_number: None,
                })
                .collect())
        }

        fn open(
            &self,
            context: &OperationContext,
            candidate: &DeviceCandidate,
        ) -> Result<Box<dyn DeviceSession>, DriverError> {
            context.check()?;
            Ok(Box::new(FakeSession {
                candidate: candidate.clone(),
                max_parallel: self.max_parallel.clone(),
                active_operations: self.active_operations.clone(),
                keymap_calls: self.keymap_calls.clone(),
                per_key_calls: self.per_key_calls.clone(),
                readback_calls: self.readback_calls.clone(),
                live_calls: self.live_calls.clone(),
                live_fail: self.live_fail.clone(),
                operation_delay: self.operation_delay,
            }))
        }
    }

    impl DeviceSession for FakeSession {
        fn descriptor(&self) -> DeviceDescriptor {
            DeviceDescriptor {
                driver_id: self.candidate.driver_id.clone(),
                device_model_id: "fake.keyboard".into(),
                instance_id: self.candidate.instance_id.clone(),
                vendor: "Fake".into(),
                name: self.candidate.name.clone(),
                connection: Connection::Usb,
                vendor_id: self.candidate.vendor_id,
                product_id: self.candidate.product_id,
                capabilities: vec![
                    Capability::Keys,
                    Capability::HardwareRgb,
                    Capability::LiveRgb,
                ],
                firmware: None,
                battery_percent: None,
            }
        }

        fn set_lighting(
            &mut self,
            context: &OperationContext,
            _config: LightingConfig,
        ) -> Result<(), DriverError> {
            run_fake_operation(
                context,
                &self.active_operations,
                &self.max_parallel,
                self.operation_delay,
            )
        }

        fn set_macros(
            &mut self,
            context: &OperationContext,
            _programs: &[HardwareMacroProgram],
        ) -> Result<(), DriverError> {
            run_fake_operation(
                context,
                &self.active_operations,
                &self.max_parallel,
                self.operation_delay,
            )
        }

        fn send_live_rgb_frame(
            &mut self,
            context: &OperationContext,
            _frame: &LiveRgbFrame,
        ) -> Result<(), DriverError> {
            context.check()?;
            self.live_calls.fetch_add(1, Ordering::AcqRel);
            if self.live_fail.load(Ordering::Acquire) {
                Err(DriverError::Protocol("simulated live frame failure".into()))
            } else {
                Ok(())
            }
        }

        fn set_keymap(
            &mut self,
            context: &OperationContext,
            _layer: KeymapLayer,
            _assignments: &[KeyAssignment],
        ) -> Result<(), DriverError> {
            self.keymap_calls.fetch_add(1, Ordering::AcqRel);
            run_fake_operation(
                context,
                &self.active_operations,
                &self.max_parallel,
                self.operation_delay,
            )
        }

        fn set_per_key_rgb(
            &mut self,
            context: &OperationContext,
            _config: &PerKeyRgbConfig,
        ) -> Result<(), DriverError> {
            self.per_key_calls.fetch_add(1, Ordering::AcqRel);
            run_fake_operation(
                context,
                &self.active_operations,
                &self.max_parallel,
                self.operation_delay,
            )
        }

        fn read_per_key_rgb(
            &mut self,
            context: &OperationContext,
            led_indices: &[u16],
        ) -> Result<PerKeyRgbReadback, DriverError> {
            self.readback_calls.fetch_add(1, Ordering::AcqRel);
            run_fake_operation(
                context,
                &self.active_operations,
                &self.max_parallel,
                self.operation_delay,
            )?;
            Ok(PerKeyRgbReadback {
                colors: led_indices
                    .iter()
                    .copied()
                    .map(|led_index| PerKeyRgbAssignment {
                        led_index,
                        color: RgbColor {
                            r: led_index as u8,
                            g: 2,
                            b: 3,
                        },
                    })
                    .collect(),
            })
        }
    }

    fn run_fake_operation(
        context: &OperationContext,
        active: &AtomicUsize,
        maximum: &AtomicUsize,
        delay: Duration,
    ) -> Result<(), DriverError> {
        let count = active.fetch_add(1, Ordering::AcqRel) + 1;
        maximum.fetch_max(count, Ordering::AcqRel);
        let result = context.sleep(delay);
        active.fetch_sub(1, Ordering::AcqRel);
        result
    }

    fn test_config() -> ManagerConfig {
        ManagerConfig {
            suspend_guard: Arc::new(AtomicBool::new(false)),
            discovery_timeout: Duration::from_millis(100),
            discovery_interval: Duration::from_millis(15),
            open_timeout: Duration::from_millis(100),
            initialization_timeout: Duration::from_millis(100),
            health_timeout: Duration::from_millis(100),
            health_interval: Duration::from_millis(15),
            command_timeout: Duration::from_millis(300),
            macro_upload_timeout: Duration::from_secs(2),
            lcd_upload_timeout: Duration::from_secs(2),
            open_attempts: 1,
            retry_delay: Duration::from_millis(2),
        }
    }

    #[test]
    fn live_failure_remains_stopped_after_reconnect_until_explicit_restart() {
        let driver = Arc::new(FakeDriver::new());
        driver.live_fail.store(true, Ordering::Release);
        let handle = start_fake(driver.clone());
        wait_for_state(&handle, DeviceState::Ready);
        handle
            .submit_live_rgb(LiveRgbFrame { colors: vec![] })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while handle.live_rgb_status().last_error.is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(handle.live_rgb_status().last_error.is_some());
        wait_for_state(&handle, DeviceState::Ready);
        for _ in 0..10 {
            assert!(
                handle
                    .submit_live_rgb(LiveRgbFrame { colors: vec![] })
                    .is_err()
            );
        }
        assert_eq!(driver.live_calls.load(Ordering::Acquire), 1);
        assert!(!handle.live_rgb_status().active);
        driver.live_fail.store(false, Ordering::Release);
        handle.reset_live_rgb_failure();
        handle
            .submit_live_rgb(LiveRgbFrame { colors: vec![] })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while handle.live_rgb_status().sent_frames == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(handle.live_rgb_status().sent_frames, 1);
        assert_eq!(driver.live_calls.load(Ordering::Acquire), 2);
        handle.shutdown();
    }

    #[test]
    fn macro_upload_uses_its_own_deadline_for_queue_and_execution() {
        let mut driver = FakeDriver::new();
        driver.operation_delay = Duration::from_millis(100);
        let mut registry = DriverRegistry::new();
        registry.register(Arc::new(driver)).unwrap();
        let mut config = test_config();
        config.command_timeout = Duration::from_millis(30);
        config.macro_upload_timeout = Duration::from_millis(300);
        let handle = DeviceManager::start(registry, config);
        wait_for_state(&handle, DeviceState::Ready);
        handle.set_macros(vec![]).unwrap();
        assert!(
            handle
                .set_lighting(LightingConfig {
                    mode: 1,
                    r: 0,
                    g: 0,
                    b: 0,
                    brightness: 5,
                    speed: 0,
                    direction: 0,
                    colorful: false
                })
                .is_err()
        );
        handle.shutdown();
    }

    fn start_fake(driver: Arc<FakeDriver>) -> DeviceManagerHandle {
        let mut registry = DriverRegistry::new();
        registry.register(driver).unwrap();
        DeviceManager::start(registry, test_config())
    }

    fn wait_for_state(handle: &DeviceManagerHandle, wanted: DeviceState) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if handle.status().state == wanted {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("manager did not reach {wanted:?}: {:?}", handle.status());
    }

    #[test]
    fn repeated_suspend_resume_blocks_writes_and_reopens() {
        let driver = Arc::new(FakeDriver::new());
        let calls = driver.keymap_calls.clone();
        let handle = start_fake(driver);
        wait_for_state(&handle, DeviceState::Ready);
        for _ in 0..20 {
            handle.suspend();
            assert!(handle.is_suspended());
            wait_for_state(&handle, DeviceState::Suspended);
            let before = calls.load(Ordering::Acquire);
            assert!(handle.set_keymap(KeymapLayer::Normal, Vec::new()).is_err());
            thread::sleep(Duration::from_millis(20));
            assert_eq!(calls.load(Ordering::Acquire), before);
            assert!(handle.status().device.is_none());
            handle.resume();
            wait_for_state(&handle, DeviceState::Ready);
            assert!(!handle.is_suspended());
        }
        handle.shutdown();
    }

    #[test]
    fn serializes_lighting_and_keymap_operations() {
        let driver = Arc::new(FakeDriver::new());
        let maximum = driver.max_parallel.clone();
        let keymap_calls = driver.keymap_calls.clone();
        let handle = start_fake(driver);
        wait_for_state(&handle, DeviceState::Ready);

        let lighting_handle = handle.clone();
        let keymap_handle = handle.clone();
        let lighting = thread::spawn(move || {
            lighting_handle.set_lighting(LightingConfig {
                mode: 1,
                r: 1,
                g: 2,
                b: 3,
                brightness: 5,
                speed: 0,
                direction: 0,
                colorful: false,
            })
        });
        let keymap = thread::spawn(move || {
            keymap_handle.set_keymap(
                KeymapLayer::Normal,
                vec![KeyAssignment {
                    device_key_index: 55,
                    action: KeyAction::ModifiedKey {
                        modifiers: vec![Modifier::LeftCtrl],
                        key: 0x06,
                    },
                }],
            )
        });

        assert!(lighting.join().unwrap().is_ok());
        assert!(keymap.join().unwrap().is_ok());
        assert_eq!(maximum.load(Ordering::Acquire), 1);
        assert_eq!(keymap_calls.load(Ordering::Acquire), 1);
        handle.shutdown();
    }

    #[test]
    fn serializes_per_key_rgb_with_other_hardware_operations() {
        let driver = Arc::new(FakeDriver::new());
        let maximum = driver.max_parallel.clone();
        let per_key_calls = driver.per_key_calls.clone();
        let handle = start_fake(driver);
        wait_for_state(&handle, DeviceState::Ready);

        let lighting_handle = handle.clone();
        let per_key_handle = handle.clone();
        let lighting = thread::spawn(move || {
            lighting_handle.set_lighting(LightingConfig {
                mode: 1,
                r: 1,
                g: 2,
                b: 3,
                brightness: 5,
                speed: 0,
                direction: 0,
                colorful: false,
            })
        });
        let per_key = thread::spawn(move || {
            per_key_handle.set_per_key_rgb(PerKeyRgbConfig {
                brightness: 5,
                colors: vec![PerKeyRgbAssignment {
                    led_index: 1,
                    color: RgbColor { r: 255, g: 0, b: 0 },
                }],
            })
        });

        assert!(lighting.join().unwrap().is_ok());
        assert!(per_key.join().unwrap().is_ok());
        assert_eq!(maximum.load(Ordering::Acquire), 1);
        assert_eq!(per_key_calls.load(Ordering::Acquire), 1);
        handle.shutdown();
    }

    #[test]
    fn returns_per_key_rgb_readback_through_serialized_manager() {
        let driver = Arc::new(FakeDriver::new());
        let readback_calls = driver.readback_calls.clone();
        let handle = start_fake(driver);
        wait_for_state(&handle, DeviceState::Ready);

        let readback = handle.read_per_key_rgb(vec![1, 56]).unwrap();
        assert_eq!(readback.colors.len(), 2);
        assert_eq!(readback.colors[0].led_index, 1);
        assert_eq!(readback.colors[1].led_index, 56);
        assert_eq!(readback_calls.load(Ordering::Acquire), 1);
        handle.shutdown();
    }

    #[test]
    fn tracks_keymap_as_active_operation() {
        let driver = Arc::new(FakeDriver {
            operation_delay: Duration::from_millis(120),
            ..FakeDriver::new()
        });
        let handle = start_fake(driver);
        wait_for_state(&handle, DeviceState::Ready);

        let worker = handle.clone();
        let task = thread::spawn(move || {
            worker.set_keymap(
                KeymapLayer::Normal,
                vec![KeyAssignment {
                    device_key_index: 1,
                    action: KeyAction::Disabled,
                }],
            )
        });
        wait_for_state(&handle, DeviceState::Busy);
        assert_eq!(
            handle.status().active_operation.unwrap().name,
            "apply normal-layer keymap"
        );
        assert!(task.join().unwrap().is_ok());
        wait_for_state(&handle, DeviceState::Ready);
        handle.shutdown();
    }

    #[test]
    fn removal_and_reconnect_still_work() {
        let driver = Arc::new(FakeDriver::new());
        let handle = start_fake(driver.clone());
        wait_for_state(&handle, DeviceState::Ready);
        driver.present.store(false, Ordering::Release);
        wait_for_state(&handle, DeviceState::Disconnected);
        driver.present.store(true, Ordering::Release);
        wait_for_state(&handle, DeviceState::Ready);
        handle.shutdown();
    }
}
