use mechkeymesh_driver_api::{
    Capability, Connection, DeviceDescriptor, DeviceSession, DriverError,
    KeyboardDriver, LightingConfig,
};
use mechkeymesh_hid::{
    open_feature_device, FeatureTransport, HidTransportError, FEATURE_REPORT_SIZE,
};
use std::{thread, time::Duration};

pub const VENDOR_ID: u16 = 0x0C45;
pub const PRODUCT_ID: u16 = 0x800A;
const CONFIG_USAGE_PAGE: u16 = 0xFF13;
const COMMAND_DELAY: Duration = Duration::from_millis(35);

pub struct AulaF108ProDriver;

impl AulaF108ProDriver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AulaF108ProDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardDriver for AulaF108ProDriver {
    fn id(&self) -> &'static str {
        "aula.f108pro.usb"
    }

    fn display_name(&self) -> &'static str {
        "AULA F108 Pro"
    }

    fn open(&self) -> Result<Box<dyn DeviceSession>, DriverError> {
        let transport = open_feature_device(
            VENDOR_ID,
            PRODUCT_ID,
            CONFIG_USAGE_PAGE,
        )
        .map_err(map_transport_error)?;

        Ok(Box::new(AulaF108ProSession { transport }))
    }
}

struct AulaF108ProSession {
    transport: Box<dyn FeatureTransport>,
}

impl DeviceSession for AulaF108ProSession {
    fn descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor {
            driver_id: "aula.f108pro.usb".into(),
            vendor: "AULA".into(),
            name: "AULA F108 Pro".into(),
            connection: Connection::Usb,
            vendor_id: Some(VENDOR_ID),
            product_id: Some(PRODUCT_ID),
            capabilities: vec![
                Capability::Keys,
                Capability::FnLayer,
                Capability::Macros,
                Capability::HardwareRgb,
                Capability::PerKeyRgb,
                Capability::LiveRgb,
                Capability::Music,
                Capability::Display,
                Capability::Clock,
                Capability::DeviceSettings,
                Capability::Encoder,
            ],
            firmware: None,
            battery_percent: None,
        }
    }

    fn set_lighting(&mut self, config: LightingConfig) -> Result<(), DriverError> {
        let payload = build_lighting_payload(config)?;

        self.begin_transaction()?;
        self.lighting_init()?;
        self.send(payload, false)?;
        self.apply_transaction()?;
        self.finalize_transaction()?;
        Ok(())
    }
}

impl AulaF108ProSession {
    fn send(
        &self,
        report: [u8; FEATURE_REPORT_SIZE],
        readback: bool,
    ) -> Result<(), DriverError> {
        self.transport
            .set_feature_report(&report)
            .map_err(map_transport_error)?;
        thread::sleep(COMMAND_DELAY);

        if readback {
            self.transport
                .get_feature_report()
                .map_err(map_transport_error)?;
            thread::sleep(COMMAND_DELAY);
        }
        Ok(())
    }

    fn begin_transaction(&self) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0x18;
        self.send(report, true)
    }

    fn lighting_init(&self) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0x13;
        report[8] = 0x01;
        self.send(report, true)
    }

    fn apply_transaction(&self) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0x02;
        self.send(report, true)
    }

    fn finalize_transaction(&self) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0xF0;
        self.send(report, false)
    }
}

pub const LIGHTING_MODES: [&str; 20] = [
    "Off",
    "Static",
    "Single On",
    "Single Off",
    "Glittering",
    "Falling",
    "Colourful",
    "Breath",
    "Spectrum",
    "Outward",
    "Scrolling",
    "Rolling",
    "Rotating",
    "Explode",
    "Launch",
    "Ripples",
    "Flowing",
    "Pulsating",
    "Tilt",
    "Shuttle",
];

fn build_lighting_payload(
    config: LightingConfig,
) -> Result<[u8; FEATURE_REPORT_SIZE], DriverError> {
    if config.mode as usize >= LIGHTING_MODES.len() {
        return Err(DriverError::InvalidConfiguration(format!(
            "lighting mode {} is outside 0..{}",
            config.mode,
            LIGHTING_MODES.len() - 1
        )));
    }
    if config.brightness > 5 {
        return Err(DriverError::InvalidConfiguration(
            "brightness must be between 0 and 5".into(),
        ));
    }
    if config.speed > 5 {
        return Err(DriverError::InvalidConfiguration(
            "speed must be between 0 and 5".into(),
        ));
    }
    if config.direction > 1 {
        return Err(DriverError::InvalidConfiguration(
            "direction must be 0 or 1".into(),
        ));
    }

    let mut payload = [0u8; FEATURE_REPORT_SIZE];
    payload[0] = config.mode;

    if config.mode != 0 {
        payload[1] = config.r;
        payload[2] = config.g;
        payload[3] = config.b;
        payload[8] = u8::from(config.colorful);
        payload[9] = config.brightness;
        payload[10] = config.speed;
        payload[11] = config.direction;
    }

    // Confirmed by the current working F108 Pro implementation.
    // Do not generalise this byte order to other packet families.
    payload[14] = 0x55;
    payload[15] = 0xAA;
    Ok(payload)
}

fn map_transport_error(error: HidTransportError) -> DriverError {
    match error {
        HidTransportError::NotFound => DriverError::NotFound,
        HidTransportError::UnsupportedPlatform => DriverError::UnsupportedPlatform,
        HidTransportError::Backend(message) => DriverError::Transport(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_known_breath_payload() {
        let payload = build_lighting_payload(LightingConfig {
            mode: 7,
            r: 1,
            g: 2,
            b: 3,
            brightness: 5,
            speed: 3,
            direction: 1,
            colorful: true,
        })
        .unwrap();

        assert_eq!(payload[0], 7);
        assert_eq!(&payload[1..4], &[1, 2, 3]);
        assert_eq!(payload[8], 1);
        assert_eq!(payload[9], 5);
        assert_eq!(payload[10], 3);
        assert_eq!(payload[11], 1);
        assert_eq!(&payload[14..16], &[0x55, 0xAA]);
    }

    #[test]
    fn rejects_out_of_range_values() {
        let base = LightingConfig {
            mode: 1,
            r: 0,
            g: 0,
            b: 0,
            brightness: 5,
            speed: 3,
            direction: 0,
            colorful: false,
        };

        assert!(build_lighting_payload(LightingConfig { mode: 20, ..base }).is_err());
        assert!(build_lighting_payload(LightingConfig { brightness: 6, ..base }).is_err());
        assert!(build_lighting_payload(LightingConfig { speed: 6, ..base }).is_err());
        assert!(build_lighting_payload(LightingConfig { direction: 2, ..base }).is_err());
    }

    #[test]
    fn off_does_not_leak_other_parameters() {
        let payload = build_lighting_payload(LightingConfig {
            mode: 0,
            r: 255,
            g: 255,
            b: 255,
            brightness: 5,
            speed: 5,
            direction: 1,
            colorful: true,
        })
        .unwrap();

        for index in [1, 2, 3, 8, 9, 10, 11] {
            assert_eq!(payload[index], 0);
        }
    }
}
