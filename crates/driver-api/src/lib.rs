use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    Keys,
    FnLayer,
    Macros,
    HardwareRgb,
    PerKeyRgb,
    LiveRgb,
    Music,
    Display,
    Clock,
    DeviceSettings,
    Battery,
    Encoder,
    Firmware,
}

impl Capability {
    pub const fn storage_key(self) -> &'static str {
        match self {
            Self::Keys => "keys",
            Self::FnLayer => "fn-layer",
            Self::Macros => "macros",
            Self::HardwareRgb => "hardware-rgb",
            Self::PerKeyRgb => "per-key-rgb",
            Self::LiveRgb => "live-rgb",
            Self::Music => "music",
            Self::Display => "display",
            Self::Clock => "clock",
            Self::DeviceSettings => "device-settings",
            Self::Battery => "battery",
            Self::Encoder => "encoder",
            Self::Firmware => "firmware",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Connection {
    Usb,
    Wireless24G,
    Bluetooth,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    HidFeature,
    HidInterrupt,
    UsbBulk,
    BleGatt,
    Serial,
    Network,
    VendorApi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMatch {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: Option<u16>,
    pub interface_number: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportRequirement {
    pub kind: TransportKind,
    pub usage_page: Option<u16>,
    pub interface_number: Option<i32>,
    pub report_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverManifest {
    pub driver_id: String,
    pub device_model_id: String,
    pub vendor: String,
    pub device_name: String,
    pub supported_connections: Vec<Connection>,
    pub capabilities: Vec<Capability>,
    pub matchers: Vec<DeviceMatch>,
    pub transport_requirements: Vec<TransportRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCandidate {
    pub driver_id: String,
    pub instance_id: String,
    pub name: String,
    pub connection: Connection,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: Option<u16>,
    pub interface_number: Option<i32>,
    pub serial_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDescriptor {
    pub driver_id: String,
    pub device_model_id: String,
    pub instance_id: String,
    pub vendor: String,
    pub name: String,
    pub connection: Connection,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub capabilities: Vec<Capability>,
    pub firmware: Option<String>,
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorClass {
    NotFound,
    Disconnected,
    PermissionDenied,
    Busy,
    Timeout,
    Cancelled,
    Unsupported,
    InvalidConfiguration,
    Protocol,
    Transport,
    DuplicateDevice,
    Internal,
}

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("device not found")]
    NotFound,
    #[error("device disconnected: {0}")]
    Disconnected(String),
    #[error("device access denied: {0}")]
    PermissionDenied(String),
    #[error("device is busy: {0}")]
    Busy(String),
    #[error("operation timed out: {0}")]
    Timeout(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("platform is not supported by this driver")]
    UnsupportedPlatform,
    #[error("capability is not supported: {0}")]
    UnsupportedCapability(&'static str),
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("duplicate device: {0}")]
    DuplicateDevice(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("device protocol error: {0}")]
    Protocol(String),
    #[error("internal driver error: {0}")]
    Internal(String),
}

impl DriverError {
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::NotFound => ErrorClass::NotFound,
            Self::Disconnected(_) => ErrorClass::Disconnected,
            Self::PermissionDenied(_) => ErrorClass::PermissionDenied,
            Self::Busy(_) => ErrorClass::Busy,
            Self::Timeout(_) => ErrorClass::Timeout,
            Self::Cancelled => ErrorClass::Cancelled,
            Self::UnsupportedPlatform | Self::UnsupportedCapability(_) => ErrorClass::Unsupported,
            Self::InvalidConfiguration(_) => ErrorClass::InvalidConfiguration,
            Self::DuplicateDevice(_) => ErrorClass::DuplicateDevice,
            Self::Transport(_) => ErrorClass::Transport,
            Self::Protocol(_) => ErrorClass::Protocol,
            Self::Internal(_) => ErrorClass::Internal,
        }
    }

    pub fn retryable_open(&self) -> bool {
        matches!(
            self,
            Self::Busy(_) | Self::Disconnected(_) | Self::Transport(_)
        )
    }

    pub fn requires_recovery(&self) -> bool {
        matches!(
            self,
            Self::NotFound
                | Self::Disconnected(_)
                | Self::Busy(_)
                | Self::Timeout(_)
                | Self::Transport(_)
                | Self::Protocol(_)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingConfig {
    pub mode: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub brightness: u8,
    pub speed: u8,
    pub direction: u8,
    pub colorful: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone)]
pub struct OperationContext {
    deadline: Instant,
    cancellation: CancellationToken,
}

impl OperationContext {
    pub fn new(timeout: Duration) -> Self {
        Self::with_token(timeout, CancellationToken::new())
    }

    pub fn with_token(timeout: Duration, cancellation: CancellationToken) -> Self {
        Self {
            deadline: Instant::now() + timeout,
            cancellation,
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn check(&self) -> Result<(), DriverError> {
        if self.cancellation.is_cancelled() {
            return Err(DriverError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(DriverError::Timeout("deadline exceeded".into()));
        }
        Ok(())
    }

    pub fn sleep(&self, duration: Duration) -> Result<(), DriverError> {
        const SLICE: Duration = Duration::from_millis(10);
        let end = Instant::now() + duration;

        loop {
            self.check()?;
            let now = Instant::now();
            if now >= end {
                return Ok(());
            }
            let until_end = end.saturating_duration_since(now);
            let until_deadline = self.remaining();
            let sleep_for = SLICE.min(until_end).min(until_deadline);
            if sleep_for.is_zero() {
                return self.check();
            }
            std::thread::sleep(sleep_for);
        }
    }
}

pub trait DeviceSession: Send {
    fn descriptor(&self) -> DeviceDescriptor;

    fn initialize(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        context.check()
    }

    fn health_check(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        context.check()
    }

    fn set_lighting(
        &mut self,
        _context: &OperationContext,
        _config: LightingConfig,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("hardware-rgb"))
    }

    fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

pub trait KeyboardDriver: Send + Sync {
    fn manifest(&self) -> DriverManifest;

    fn discover(&self, context: &OperationContext) -> Result<Vec<DeviceCandidate>, DriverError>;

    fn open(
        &self,
        context: &OperationContext,
        candidate: &DeviceCandidate,
    ) -> Result<Box<dyn DeviceSession>, DriverError>;

    fn is_present(
        &self,
        context: &OperationContext,
        candidate: &DeviceCandidate,
    ) -> Result<bool, DriverError> {
        context.check()?;
        Ok(self
            .discover(context)?
            .iter()
            .any(|found| found.instance_id == candidate.instance_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_observed() {
        let token = CancellationToken::new();
        let context = OperationContext::with_token(Duration::from_secs(1), token.clone());
        assert!(context.check().is_ok());
        token.cancel();
        assert!(matches!(context.check(), Err(DriverError::Cancelled)));
    }

    #[test]
    fn zero_timeout_expires() {
        let context = OperationContext::new(Duration::ZERO);
        assert!(matches!(context.check(), Err(DriverError::Timeout(_))));
    }

    #[test]
    fn capability_storage_keys_are_stable() {
        assert_eq!(Capability::HardwareRgb.storage_key(), "hardware-rgb");
        assert_eq!(Capability::FnLayer.storage_key(), "fn-layer");
        assert_eq!(Capability::DeviceSettings.storage_key(), "device-settings");
    }

    #[test]
    fn error_classes_are_stable() {
        assert_eq!(
            DriverError::PermissionDenied("denied".into()).class(),
            ErrorClass::PermissionDenied
        );
        assert!(DriverError::Busy("busy".into()).retryable_open());
        assert!(DriverError::Protocol("bad ack".into()).requires_recovery());
        assert!(!DriverError::InvalidConfiguration("bad value".into()).requires_recovery());
    }
}
