pub use mechkeymesh_layout::{
    KeyAction, Layout, MediaAction, Modifier, MouseAction, WindowsAction,
};
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
    pub layout_ids: Vec<String>,
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
#[serde(rename_all = "kebab-case")]
pub enum LightingColorControl {
    None,
    ColorOrRainbow,
    RainbowOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LightingDirectionControl {
    LeftRight,
    UpDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingEffectDescriptor {
    pub mode: u8,
    pub name: String,
    pub color: LightingColorControl,
    pub brightness: bool,
    pub speed: bool,
    pub direction: Option<LightingDirectionControl>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    pub const fn is_black(self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerKeyRgbAssignment {
    pub led_index: u16,
    pub color: RgbColor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerKeyRgbConfig {
    pub brightness: u8,
    pub colors: Vec<PerKeyRgbAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerKeyRgbReadback {
    pub colors: Vec<PerKeyRgbAssignment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerKeyRgbCapabilities {
    pub brightness_min: u8,
    pub brightness_max: u8,
    pub readback_supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeymapLayer {
    Normal,
    Fn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyAssignment {
    pub device_key_index: u16,
    pub action: KeyAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyRestriction {
    pub device_key_index: u16,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeymapCapabilities {
    pub disable: bool,
    pub keyboard_key: bool,
    pub modifier: bool,
    pub shortcut: bool,
    pub media_actions: Vec<MediaAction>,
    pub mouse_actions: Vec<MouseAction>,
    pub windows_actions: Vec<WindowsAction>,
    pub open_application: bool,
    pub open_file: bool,
    pub open_url: bool,
    pub send_text: bool,
    pub macro_action: bool,
    #[serde(default)]
    pub protected_keys: Vec<KeyRestriction>,
    pub unsupported_host_actions_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FnActivationMode {
    Toggle,
    Momentary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FnActivationModeOption {
    pub mode: FnActivationMode,
    pub raw_value: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FnLayerDescriptor {
    pub modes: Vec<FnActivationModeOption>,
    pub current_mode: Option<FnActivationMode>,
    pub hardware_write_verified: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HardwareMacroMouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum HardwareMacroEvent {
    KeyDown { usage: u16 },
    KeyUp { usage: u16 },
    MouseDown { button: HardwareMacroMouseButton },
    MouseUp { button: HardwareMacroMouseButton },
    Delay { milliseconds: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareMacroProgram {
    pub id: String,
    pub slot: u8,
    pub events: Vec<HardwareMacroEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroBankDescriptor {
    pub slot_count: usize,
    pub maximum_bank_bytes: usize,
    pub maximum_events_per_macro: usize,
    pub minimum_delay_milliseconds: u32,
    pub maximum_delay_milliseconds: u32,
    pub minimum_loop_count: u16,
    pub maximum_loop_count: u16,
    pub hardware_write_verified: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SleepTimeout {
    Never,
    OneMinute,
    FiveMinutes,
    ThirtyMinutes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    pub disable_windows_key: bool,
    pub disable_alt_tab: bool,
    pub disable_alt_f4: bool,
    pub fn_mode: FnActivationMode,
    pub sleep_timeout: SleepTimeout,
    pub key_response_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepTimeoutOption {
    pub timeout: SleepTimeout,
    pub raw_value: u8,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResponseLevelOption {
    pub level: u8,
    pub raw_value: u8,
    pub label: String,
    pub wired_timing: String,
    pub wireless_timing: String,
    pub bluetooth_timing: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettingsDescriptor {
    pub sleep_options: Vec<SleepTimeoutOption>,
    pub fn_modes: Vec<FnActivationModeOption>,
    pub key_response_levels: Vec<KeyResponseLevelOption>,
    pub hardware_write_verified: bool,
    pub note: String,
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

    fn set_keymap(
        &mut self,
        _context: &OperationContext,
        _layer: KeymapLayer,
        _assignments: &[KeyAssignment],
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("key-remapping"))
    }

    fn set_macros(
        &mut self,
        _context: &OperationContext,
        _programs: &[HardwareMacroProgram],
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("macros"))
    }

    fn set_device_settings(
        &mut self,
        _context: &OperationContext,
        _settings: &DeviceSettings,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("device-settings"))
    }

    fn set_per_key_rgb(
        &mut self,
        _context: &OperationContext,
        _config: &PerKeyRgbConfig,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("per-key-rgb"))
    }

    fn read_per_key_rgb(
        &mut self,
        _context: &OperationContext,
        _led_indices: &[u16],
    ) -> Result<PerKeyRgbReadback, DriverError> {
        Err(DriverError::UnsupportedCapability("per-key-rgb-readback"))
    }

    fn close(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

pub trait KeyboardDriver: Send + Sync {
    fn manifest(&self) -> DriverManifest;

    fn layouts(&self) -> Vec<Layout> {
        Vec::new()
    }

    fn hardware_lighting_effects(&self) -> Vec<LightingEffectDescriptor> {
        Vec::new()
    }

    fn keymap_capabilities(&self, _layer: KeymapLayer) -> Option<KeymapCapabilities> {
        None
    }

    fn fn_layer_descriptor(&self) -> Option<FnLayerDescriptor> {
        None
    }

    fn macro_bank_descriptor(&self) -> Option<MacroBankDescriptor> {
        None
    }

    fn device_settings_descriptor(&self) -> Option<DeviceSettingsDescriptor> {
        None
    }

    fn validate_macro_bank(&self, _programs: &[HardwareMacroProgram]) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("macros"))
    }

    fn validate_device_settings(&self, _settings: &DeviceSettings) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("device-settings"))
    }

    fn per_key_rgb_capabilities(&self) -> Option<PerKeyRgbCapabilities> {
        None
    }

    fn validate_per_key_rgb(&self, _config: &PerKeyRgbConfig) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("per-key-rgb"))
    }

    fn validate_keymap(
        &self,
        _layer: KeymapLayer,
        _assignments: &[KeyAssignment],
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("key-remapping"))
    }

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
        assert_eq!(Capability::Keys.storage_key(), "keys");
        assert_eq!(Capability::FnLayer.storage_key(), "fn-layer");
    }

    #[test]
    fn fn_activation_mode_values_are_serializable_without_losing_raw_enum() {
        let descriptor = FnLayerDescriptor {
            modes: vec![
                FnActivationModeOption {
                    mode: FnActivationMode::Toggle,
                    raw_value: 0,
                },
                FnActivationModeOption {
                    mode: FnActivationMode::Momentary,
                    raw_value: 1,
                },
            ],
            current_mode: None,
            hardware_write_verified: false,
            note: "test".into(),
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        assert!(json.contains("\"mode\":\"toggle\""));
        assert!(json.contains("\"rawValue\":0"));
        assert!(json.contains("\"mode\":\"momentary\""));
        assert!(json.contains("\"rawValue\":1"));
    }

    #[test]
    fn per_key_rgb_configuration_round_trips_through_json() {
        let config = PerKeyRgbConfig {
            brightness: 5,
            colors: vec![
                PerKeyRgbAssignment {
                    led_index: 1,
                    color: RgbColor { r: 255, g: 0, b: 0 },
                },
                PerKeyRgbAssignment {
                    led_index: 56,
                    color: RgbColor { r: 0, g: 255, b: 0 },
                },
            ],
        };

        let json = serde_json::to_string(&config).unwrap();
        let decoded: PerKeyRgbConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, config);
        assert!(RgbColor::BLACK.is_black());
    }

    #[test]
    fn key_assignment_round_trips_through_json() {
        let assignment = KeyAssignment {
            device_key_index: 55,
            action: KeyAction::ModifiedKey {
                modifiers: vec![Modifier::LeftCtrl],
                key: 0x06,
            },
        };

        let json = serde_json::to_string(&assignment).unwrap();
        let decoded: KeyAssignment = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, assignment);
    }

    #[test]
    fn lighting_effect_descriptor_serializes_direction_and_color_mode() {
        let effect = LightingEffectDescriptor {
            mode: 10,
            name: "Scrolling".into(),
            color: LightingColorControl::ColorOrRainbow,
            brightness: true,
            speed: true,
            direction: Some(LightingDirectionControl::LeftRight),
        };

        let json = serde_json::to_string(&effect).unwrap();
        assert!(json.contains("\"color\":\"color-or-rainbow\""));
        assert!(json.contains("\"direction\":\"left-right\""));
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
