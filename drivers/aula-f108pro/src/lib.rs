use mechkeymesh_driver_api::{
    Capability, Connection, DeviceCandidate, DeviceDescriptor, DeviceMatch, DeviceSession,
    DriverError, DriverManifest, KeyboardDriver, LightingConfig, OperationContext, TransportKind,
    TransportRequirement,
};
use mechkeymesh_hid::{
    FEATURE_REPORT_SIZE, FeatureTransport, HidTransportError, enumerate_feature_devices,
    open_feature_path,
};
use std::time::Duration;

pub const VENDOR_ID: u16 = 0x0C45;
pub const PRODUCT_ID: u16 = 0x800A;
pub const CONFIG_USAGE_PAGE: u16 = 0xFF13;
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
    fn manifest(&self) -> DriverManifest {
        DriverManifest {
            driver_id: "aula.f108pro.usb".into(),
            device_model_id: "aula.f108pro".into(),
            vendor: "AULA".into(),
            device_name: "AULA F108 Pro".into(),
            supported_connections: vec![Connection::Usb],
            capabilities: vec![Capability::HardwareRgb],
            matchers: vec![DeviceMatch {
                vendor_id: VENDOR_ID,
                product_id: PRODUCT_ID,
                usage_page: Some(CONFIG_USAGE_PAGE),
                // The Windows HID path is selected by usage page rather than
                // assuming a stable interface number across HID backends.
                interface_number: None,
            }],
            transport_requirements: vec![TransportRequirement {
                kind: TransportKind::HidFeature,
                usage_page: Some(CONFIG_USAGE_PAGE),
                interface_number: None,
                report_size: Some(FEATURE_REPORT_SIZE),
            }],
        }
    }

    fn discover(&self, context: &OperationContext) -> Result<Vec<DeviceCandidate>, DriverError> {
        context.check()?;
        let devices = enumerate_feature_devices(VENDOR_ID, PRODUCT_ID, CONFIG_USAGE_PAGE)
            .map_err(map_transport_error)?;

        context.check()?;
        Ok(devices
            .into_iter()
            .map(|device| DeviceCandidate {
                driver_id: "aula.f108pro.usb".into(),
                instance_id: device.path,
                name: "AULA F108 Pro".into(),
                connection: Connection::Usb,
                vendor_id: Some(device.vendor_id),
                product_id: Some(device.product_id),
                usage_page: Some(device.usage_page),
                interface_number: Some(device.interface_number),
                serial_number: device.serial_number,
            })
            .collect())
    }

    fn open(
        &self,
        context: &OperationContext,
        candidate: &DeviceCandidate,
    ) -> Result<Box<dyn DeviceSession>, DriverError> {
        context.check()?;
        validate_candidate(candidate)?;

        let transport = open_feature_path(&candidate.instance_id).map_err(map_transport_error)?;
        context.check()?;

        Ok(Box::new(AulaF108ProSession {
            candidate: candidate.clone(),
            transport,
        }))
    }
}

struct AulaF108ProSession {
    candidate: DeviceCandidate,
    transport: Box<dyn FeatureTransport>,
}

impl DeviceSession for AulaF108ProSession {
    fn descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor {
            driver_id: self.candidate.driver_id.clone(),
            device_model_id: "aula.f108pro".into(),
            instance_id: self.candidate.instance_id.clone(),
            vendor: "AULA".into(),
            name: self.candidate.name.clone(),
            connection: self.candidate.connection,
            vendor_id: self.candidate.vendor_id,
            product_id: self.candidate.product_id,
            // Expose only capabilities implemented and testable in the current
            // driver, not every capability known to exist in the OEM software.
            capabilities: vec![Capability::HardwareRgb],
            firmware: None,
            battery_percent: None,
        }
    }

    fn initialize(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        // R2 establishes the lifecycle state without inventing an undocumented
        // F108 initialization command. Opening the verified FF13 collection is
        // sufficient for this driver today.
        context.check()
    }

    fn health_check(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        // Device-removal detection is performed by rediscovering this exact HID
        // path through the driver. Avoid sending probe commands merely to test
        // liveness.
        context.check()
    }

    fn set_lighting(
        &mut self,
        context: &OperationContext,
        config: LightingConfig,
    ) -> Result<(), DriverError> {
        context.check()?;
        let payload = build_lighting_payload(config)?;

        self.begin_transaction(context)?;
        self.lighting_init(context)?;
        self.send(context, payload, false)?;
        self.apply_transaction(context)?;
        self.finalize_transaction(context)?;
        Ok(())
    }
}

impl AulaF108ProSession {
    fn send(
        &self,
        context: &OperationContext,
        report: [u8; FEATURE_REPORT_SIZE],
        readback: bool,
    ) -> Result<(), DriverError> {
        context.check()?;
        self.transport
            .set_feature_report(&report)
            .map_err(map_transport_error)?;
        context.sleep(COMMAND_DELAY)?;

        if readback {
            context.check()?;
            let response = self
                .transport
                .get_feature_report()
                .map_err(map_transport_error)?;
            validate_ack(&response)?;
            context.sleep(COMMAND_DELAY)?;
        }
        Ok(())
    }

    fn begin_transaction(&self, context: &OperationContext) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0x18;
        self.send(context, report, true)
    }

    fn lighting_init(&self, context: &OperationContext) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0x13;
        report[8] = 0x01;
        self.send(context, report, true)
    }

    fn apply_transaction(&self, context: &OperationContext) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0x02;
        self.send(context, report, true)
    }

    fn finalize_transaction(&self, context: &OperationContext) -> Result<(), DriverError> {
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x04;
        report[1] = 0xF0;
        self.send(context, report, false)
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

fn validate_candidate(candidate: &DeviceCandidate) -> Result<(), DriverError> {
    if candidate.driver_id != "aula.f108pro.usb"
        || candidate.vendor_id != Some(VENDOR_ID)
        || candidate.product_id != Some(PRODUCT_ID)
        || candidate.usage_page != Some(CONFIG_USAGE_PAGE)
    {
        return Err(DriverError::InvalidConfiguration(
            "candidate does not match the AULA F108 Pro USB configuration interface".into(),
        ));
    }
    Ok(())
}

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

fn validate_ack(response: &[u8; FEATURE_REPORT_SIZE]) -> Result<(), DriverError> {
    if response[3] != 0x01 {
        return Err(DriverError::Protocol(format!(
            "unexpected acknowledgment byte 3: 0x{:02X}",
            response[3]
        )));
    }
    Ok(())
}

fn map_transport_error(error: HidTransportError) -> DriverError {
    match error {
        HidTransportError::NotFound => DriverError::NotFound,
        HidTransportError::Disconnected(message) => DriverError::Disconnected(message),
        HidTransportError::PermissionDenied(message) => DriverError::PermissionDenied(message),
        HidTransportError::Busy(message) => DriverError::Busy(message),
        HidTransportError::UnsupportedPlatform => DriverError::UnsupportedPlatform,
        HidTransportError::Backend(message) => DriverError::Transport(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> DeviceCandidate {
        DeviceCandidate {
            driver_id: "aula.f108pro.usb".into(),
            instance_id: "test-path".into(),
            name: "AULA F108 Pro".into(),
            connection: Connection::Usb,
            vendor_id: Some(VENDOR_ID),
            product_id: Some(PRODUCT_ID),
            usage_page: Some(CONFIG_USAGE_PAGE),
            interface_number: Some(3),
            serial_number: None,
        }
    }

    #[test]
    fn manifest_declares_f108_transport_requirements() {
        let manifest = AulaF108ProDriver::new().manifest();
        assert_eq!(manifest.driver_id, "aula.f108pro.usb");
        assert_eq!(manifest.device_model_id, "aula.f108pro");
        assert_eq!(manifest.supported_connections, vec![Connection::Usb]);
        assert_eq!(manifest.capabilities, vec![Capability::HardwareRgb]);
        assert_eq!(manifest.matchers[0].vendor_id, VENDOR_ID);
        assert_eq!(manifest.matchers[0].product_id, PRODUCT_ID);
        assert_eq!(manifest.matchers[0].usage_page, Some(CONFIG_USAGE_PAGE));
        assert_eq!(
            manifest.transport_requirements[0].kind,
            TransportKind::HidFeature
        );
    }

    #[test]
    fn rejects_foreign_candidate() {
        let mut wrong = candidate();
        wrong.product_id = Some(0xFFFF);
        assert!(validate_candidate(&wrong).is_err());
    }

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
        assert!(
            build_lighting_payload(LightingConfig {
                brightness: 6,
                ..base
            })
            .is_err()
        );
        assert!(build_lighting_payload(LightingConfig { speed: 6, ..base }).is_err());
        assert!(
            build_lighting_payload(LightingConfig {
                direction: 2,
                ..base
            })
            .is_err()
        );
    }

    #[test]
    fn validates_device_acknowledgment() {
        let mut valid = [0u8; FEATURE_REPORT_SIZE];
        valid[3] = 0x01;
        assert!(validate_ack(&valid).is_ok());

        let invalid = [0u8; FEATURE_REPORT_SIZE];
        assert!(validate_ack(&invalid).is_err());
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
