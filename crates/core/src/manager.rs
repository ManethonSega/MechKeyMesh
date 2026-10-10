use crate::{DriverRegistry, LogEntry, LogRing};
use mechkeymesh_driver_api::{
    CancellationToken, DeviceCandidate, DeviceDescriptor, DeviceSession, DriverError,
    DriverManifest, ErrorClass, KeyboardDriver, LightingConfig, OperationContext,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
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
    pub discovery_timeout: Duration,
    pub discovery_interval: Duration,
    pub open_timeout: Duration,
    pub initialization_timeout: Duration,
    pub health_timeout: Duration,
    pub health_interval: Duration,
    pub command_timeout: Duration,
    pub open_attempts: usize,
    pub retry_delay: Duration,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            discovery_timeout: Duration::from_secs(2),
            discovery_interval: Duration::from_millis(900),
            open_timeout: Duration::from_secs(2),
            initialization_timeout: Duration::from_secs(2),
            health_timeout: Duration::from_millis(700),
            health_interval: Duration::from_millis(900),
            command_timeout: Duration::from_secs(3),
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
    SetLighting {
        id: u64,
        config: LightingConfig,
        submitted: Instant,
        cancellation: CancellationToken,
        reply: Sender<Result<(), String>>,
    },
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

        match reply_rx.recv_timeout(self.command_timeout + Duration::from_millis(250)) {
            Ok(result) => result,
            Err(_) => {
                cancellation.cancel();
                Err(format!(
                    "device command timed out after {} ms",
                    self.command_timeout.as_millis()
                ))
            }
        }
    }

    pub fn shutdown(&self) {
        self.cancel_active_operation();
        let _ = self.tx.send(Command::Shutdown);
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

        let handle = DeviceManagerHandle {
            tx,
            status: status.clone(),
            log: log.clone(),
            manifests,
            active_cancellation: active_cancellation.clone(),
            next_operation_id: Arc::new(AtomicU64::new(1)),
            command_timeout: config.command_timeout,
        };

        thread::Builder::new()
            .name("mechkeymesh-device-manager".into())
            .spawn(move || worker(rx, status, log, registry, active_cancellation, config))
            .expect("failed to start device manager");

        handle
    }
}

fn worker(
    rx: Receiver<Command>,
    status: Arc<RwLock<DeviceStatus>>,
    log: LogRing,
    registry: DriverRegistry,
    active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    config: ManagerConfig,
) {
    log.push("info", "Device Manager 2.0 started");
    let mut active: Option<ActiveDevice> = None;
    let mut ever_connected = false;

    loop {
        if active.is_none() {
            active = discover_and_open(&registry, &status, &log, &config, &mut ever_connected);
        }

        let wait = if active.is_some() {
            config.health_interval
        } else {
            config.discovery_interval
        };

        match rx.recv_timeout(wait) {
            Ok(Command::Shutdown) => {
                close_active(&mut active, &log);
                log.push("info", "device manager stopped");
                return;
            }
            Ok(Command::Reconnect) => {
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
                let result = execute_lighting(
                    &mut active,
                    &status,
                    &log,
                    &active_cancellation,
                    &config,
                    id,
                    lighting,
                    submitted,
                    cancellation,
                );
                let _ = reply.send(result);
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

fn discover_and_open(
    registry: &DriverRegistry,
    status: &Arc<RwLock<DeviceStatus>>,
    log: &LogRing,
    config: &ManagerConfig,
    ever_connected: &mut bool,
) -> Option<ActiveDevice> {
    set_status(status, DeviceState::Detecting, None, None, None, 0);
    let discovery_context = OperationContext::new(config.discovery_timeout);

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
    let mut last_error: Option<DriverError> = None;

    for attempt in 1..=attempts {
        let open_context = OperationContext::new(config.open_timeout);
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

                let initialization_context = OperationContext::new(config.initialization_timeout);
                match session.initialize(&initialization_context) {
                    Ok(()) => {
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

#[allow(clippy::too_many_arguments)]
fn execute_lighting(
    active: &mut Option<ActiveDevice>,
    status: &Arc<RwLock<DeviceStatus>>,
    log: &LogRing,
    active_cancellation: &Arc<Mutex<Option<CancellationToken>>>,
    config: &ManagerConfig,
    id: u64,
    lighting: LightingConfig,
    submitted: Instant,
    cancellation: CancellationToken,
) -> Result<(), String> {
    let Some(device) = active.as_mut() else {
        return Err("no supported keyboard is ready".into());
    };

    let elapsed = submitted.elapsed();
    if elapsed >= config.command_timeout {
        return Err(format!(
            "operation expired in the queue after {} ms",
            elapsed.as_millis()
        ));
    }

    let remaining = config.command_timeout - elapsed;
    let descriptor = device.session.descriptor();
    let operation = ActiveOperation {
        id,
        name: "apply built-in lighting".into(),
        started_unix_millis: unix_millis(),
        timeout_millis: remaining.as_millis().try_into().unwrap_or(u64::MAX),
    };

    {
        *active_cancellation
            .lock()
            .expect("active cancellation lock poisoned") = Some(cancellation.clone());
    }

    set_status(
        status,
        DeviceState::Busy,
        Some(descriptor.clone()),
        None,
        Some(operation),
        1,
    );
    log.push(
        "info",
        format!("operation {id} started: apply built-in lighting"),
    );

    let context = OperationContext::with_token(remaining, cancellation);
    let result = device.session.set_lighting(&context, lighting);

    {
        *active_cancellation
            .lock()
            .expect("active cancellation lock poisoned") = None;
    }

    match result {
        Ok(()) => {
            set_status(status, DeviceState::Ready, Some(descriptor), None, None, 1);
            log.push(
                "info",
                format!("operation {id} completed: apply built-in lighting"),
            );
            Ok(())
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

fn active_device_present(active: &mut ActiveDevice, log: &LogRing, config: &ManagerConfig) -> bool {
    let context = OperationContext::new(config.health_timeout);

    match active.driver.is_present(&context, &active.candidate) {
        Ok(false) | Err(DriverError::NotFound) | Err(DriverError::Disconnected(_)) => false,
        Ok(true) => match active.session.health_check(&context) {
            Ok(()) => true,
            Err(DriverError::Cancelled) => true,
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
    if let Some(mut device) = active.take() {
        if let Err(error) = device.session.close() {
            log.push("warning", format!("device close failed: {error}"));
        }
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
        Capability, Connection, DeviceMatch, DriverManifest, TransportKind, TransportRequirement,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    struct FakeDriver {
        present: Arc<AtomicBool>,
        candidates: Arc<AtomicUsize>,
        open_failures: Arc<AtomicUsize>,
        max_parallel: Arc<AtomicUsize>,
        active_operations: Arc<AtomicUsize>,
        operation_delay: Duration,
    }

    struct FakeSession {
        candidate: DeviceCandidate,
        max_parallel: Arc<AtomicUsize>,
        active_operations: Arc<AtomicUsize>,
        operation_delay: Duration,
    }

    impl FakeDriver {
        fn new() -> Self {
            Self {
                present: Arc::new(AtomicBool::new(true)),
                candidates: Arc::new(AtomicUsize::new(1)),
                open_failures: Arc::new(AtomicUsize::new(0)),
                max_parallel: Arc::new(AtomicUsize::new(0)),
                active_operations: Arc::new(AtomicUsize::new(0)),
                operation_delay: Duration::from_millis(15),
            }
        }

        fn candidate(&self, index: usize) -> DeviceCandidate {
            DeviceCandidate {
                driver_id: "fake.keyboard.usb".into(),
                instance_id: format!("fake-{index}"),
                name: "Fake Keyboard".into(),
                connection: Connection::Usb,
                vendor_id: Some(0x1234),
                product_id: Some(0x5678),
                usage_page: Some(0xFF00),
                interface_number: Some(0),
                serial_number: None,
            }
        }
    }

    impl KeyboardDriver for FakeDriver {
        fn manifest(&self) -> DriverManifest {
            DriverManifest {
                driver_id: "fake.keyboard.usb".into(),
                vendor: "Fake".into(),
                device_name: "Fake Keyboard".into(),
                supported_connections: vec![Connection::Usb],
                capabilities: vec![Capability::HardwareRgb],
                matchers: vec![DeviceMatch {
                    vendor_id: 0x1234,
                    product_id: 0x5678,
                    usage_page: Some(0xFF00),
                    interface_number: Some(0),
                }],
                transport_requirements: vec![TransportRequirement {
                    kind: TransportKind::HidFeature,
                    usage_page: Some(0xFF00),
                    interface_number: Some(0),
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
                .map(|index| self.candidate(index))
                .collect())
        }

        fn open(
            &self,
            context: &OperationContext,
            candidate: &DeviceCandidate,
        ) -> Result<Box<dyn DeviceSession>, DriverError> {
            context.check()?;
            let remaining = self.open_failures.load(Ordering::Acquire);
            if remaining > 0 {
                self.open_failures.fetch_sub(1, Ordering::AcqRel);
                return Err(DriverError::Busy("simulated busy device".into()));
            }
            Ok(Box::new(FakeSession {
                candidate: candidate.clone(),
                max_parallel: self.max_parallel.clone(),
                active_operations: self.active_operations.clone(),
                operation_delay: self.operation_delay,
            }))
        }
    }

    impl DeviceSession for FakeSession {
        fn descriptor(&self) -> DeviceDescriptor {
            DeviceDescriptor {
                driver_id: self.candidate.driver_id.clone(),
                instance_id: self.candidate.instance_id.clone(),
                vendor: "Fake".into(),
                name: self.candidate.name.clone(),
                connection: self.candidate.connection,
                vendor_id: self.candidate.vendor_id,
                product_id: self.candidate.product_id,
                capabilities: vec![Capability::HardwareRgb],
                firmware: None,
                battery_percent: None,
            }
        }

        fn set_lighting(
            &mut self,
            context: &OperationContext,
            _config: LightingConfig,
        ) -> Result<(), DriverError> {
            let active = self.active_operations.fetch_add(1, Ordering::AcqRel) + 1;
            self.max_parallel.fetch_max(active, Ordering::AcqRel);
            let result = context.sleep(self.operation_delay);
            self.active_operations.fetch_sub(1, Ordering::AcqRel);
            result
        }
    }

    fn test_config() -> ManagerConfig {
        ManagerConfig {
            discovery_timeout: Duration::from_millis(100),
            discovery_interval: Duration::from_millis(15),
            open_timeout: Duration::from_millis(100),
            initialization_timeout: Duration::from_millis(100),
            health_timeout: Duration::from_millis(100),
            health_interval: Duration::from_millis(15),
            command_timeout: Duration::from_millis(150),
            open_attempts: 2,
            retry_delay: Duration::from_millis(2),
        }
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
        panic!(
            "manager did not reach {wanted:?}; final status = {:?}",
            handle.status()
        );
    }

    #[test]
    fn lifecycle_reaches_ready_and_recovers_after_removal() {
        let driver = Arc::new(FakeDriver::new());
        let handle = start_fake(driver.clone());

        wait_for_state(&handle, DeviceState::Ready);
        driver.present.store(false, Ordering::Release);
        wait_for_state(&handle, DeviceState::Disconnected);

        driver.present.store(true, Ordering::Release);
        wait_for_state(&handle, DeviceState::Ready);
        assert!(handle.status().reconnect_count >= 1);

        handle.shutdown();
    }

    #[test]
    fn duplicate_candidates_are_not_opened_automatically() {
        let driver = Arc::new(FakeDriver::new());
        driver.candidates.store(2, Ordering::Release);
        let handle = start_fake(driver);

        wait_for_state(&handle, DeviceState::Ambiguous);
        assert_eq!(handle.status().candidate_count, 2);
        assert_eq!(
            handle.status().last_error_class,
            Some(ErrorClass::DuplicateDevice)
        );

        handle.shutdown();
    }

    #[test]
    fn open_retry_is_bounded_and_can_recover() {
        let driver = Arc::new(FakeDriver::new());
        driver.open_failures.store(1, Ordering::Release);
        let handle = start_fake(driver);

        wait_for_state(&handle, DeviceState::Ready);
        handle.shutdown();
    }

    #[test]
    fn hardware_operations_are_serialized() {
        let driver = Arc::new(FakeDriver::new());
        let max_parallel = driver.max_parallel.clone();
        let handle = start_fake(driver);
        wait_for_state(&handle, DeviceState::Ready);

        let first = handle.clone();
        let second = handle.clone();
        let config = LightingConfig {
            mode: 1,
            r: 1,
            g: 2,
            b: 3,
            brightness: 5,
            speed: 3,
            direction: 0,
            colorful: false,
        };

        let a = thread::spawn(move || first.set_lighting(config));
        let b = thread::spawn(move || second.set_lighting(config));
        assert!(a.join().unwrap().is_ok());
        assert!(b.join().unwrap().is_ok());
        assert_eq!(max_parallel.load(Ordering::Acquire), 1);

        handle.shutdown();
    }

    #[test]
    fn active_operation_can_be_cancelled() {
        let driver = Arc::new(FakeDriver {
            operation_delay: Duration::from_millis(500),
            ..FakeDriver::new()
        });
        let handle = start_fake(driver);
        wait_for_state(&handle, DeviceState::Ready);

        let worker = handle.clone();
        let config = LightingConfig {
            mode: 1,
            r: 1,
            g: 2,
            b: 3,
            brightness: 5,
            speed: 3,
            direction: 0,
            colorful: false,
        };
        let task = thread::spawn(move || worker.set_lighting(config));

        wait_for_state(&handle, DeviceState::Busy);
        assert!(handle.cancel_active_operation());
        assert!(task.join().unwrap().is_err());
        wait_for_state(&handle, DeviceState::Ready);

        handle.shutdown();
    }
}
