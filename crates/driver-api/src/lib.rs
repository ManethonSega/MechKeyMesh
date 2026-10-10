use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Connection {
    Usb,
    Wireless24G,
    Bluetooth,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDescriptor {
    pub driver_id: String,
    pub vendor: String,
    pub name: String,
    pub connection: Connection,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub capabilities: Vec<Capability>,
    pub firmware: Option<String>,
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("device not found")]
    NotFound,
    #[error("platform is not supported by this driver")]
    UnsupportedPlatform,
    #[error("capability is not supported: {0}")]
    UnsupportedCapability(&'static str),
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("device protocol error: {0}")]
    Protocol(String),
}

pub trait DeviceSession: Send {
    fn descriptor(&self) -> DeviceDescriptor;

    fn set_lighting(&mut self, _config: LightingConfig) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("hardware-rgb"))
    }

    fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

pub trait KeyboardDriver: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn open(&self) -> Result<Box<dyn DeviceSession>, DriverError>;
}
