mod layout;

pub use layout::f108pro_layout;

use mechkeymesh_driver_api::{
    Capability, Connection, DeviceCandidate, DeviceClock, DeviceDescriptor, DeviceMatch,
    DeviceSession, DriverError, DriverManifest, KeyboardDriver, LcdUpload, LightingColorControl, LightingConfig,
    LightingDirectionControl, LightingEffectDescriptor, OperationContext, TransportKind,
    TransportRequirement,
};
use mechkeymesh_hid::{
    FEATURE_REPORT_SIZE, FeatureTransport, HidTransportError, LcdTransport,
    enumerate_feature_devices, open_feature_path, open_lcd_path,
};
use std::time::Duration;

pub const VENDOR_ID: u16 = 0x0C45;
pub const PRODUCT_ID: u16 = 0x800A;
pub const CONFIG_USAGE_PAGE: u16 = 0xFF13;
pub const LCD_USAGE_PAGE: u16 = 0xFF68;
pub const DRIVER_ID: &str = "aula.f108pro";
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
            driver_id: DRIVER_ID.into(),
            device_model_id: "aula.f108pro".into(),
            vendor: "AULA".into(),
            device_name: "AULA F108 Pro".into(),
            supported_connections: vec![Connection::Usb],
            capabilities: vec![Capability::HardwareRgb, Capability::Display, Capability::Clock],
            layout_ids: vec!["aula.f108pro.ansi".into()],
            matchers: vec![DeviceMatch {
                vendor_id: VENDOR_ID,
                product_id: PRODUCT_ID,
                usage_page: Some(CONFIG_USAGE_PAGE),
                // The Windows HID path is selected by usage page rather than
                // assuming a stable interface number across HID backends.
                interface_number: None,
            }],
            transport_requirements: vec![
                TransportRequirement {
                    kind: TransportKind::HidFeature,
                    usage_page: Some(CONFIG_USAGE_PAGE),
                    interface_number: None,
                    report_size: Some(FEATURE_REPORT_SIZE),
                },
                TransportRequirement {
                    kind: TransportKind::HidInterrupt,
                    usage_page: Some(LCD_USAGE_PAGE),
                    interface_number: Some(2),
                    report_size: Some(mechkeymesh_hid::LCD_PAGE_SIZE),
                },
            ],
        }
    }

    fn layouts(&self) -> Vec<mechkeymesh_layout::Layout> {
        vec![f108pro_layout()]
    }

    fn hardware_lighting_effects(&self) -> Vec<LightingEffectDescriptor> {
        lighting_effects()
    }

    fn discover(&self, context: &OperationContext) -> Result<Vec<DeviceCandidate>, DriverError> {
        context.check()?;
        let devices = enumerate_feature_devices(VENDOR_ID, PRODUCT_ID, CONFIG_USAGE_PAGE)
            .map_err(map_transport_error)?;

        context.check()?;
        Ok(devices
            .into_iter()
            .map(|device| DeviceCandidate {
                driver_id: DRIVER_ID.into(),
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
        let lcd_transport = open_lcd_transport(candidate)?;
        context.check()?;

        Ok(Box::new(AulaF108ProSession {
            candidate: candidate.clone(),
            transport,
            lcd_transport,
        }))
    }
}

struct AulaF108ProSession {
    candidate: DeviceCandidate,
    transport: Box<dyn FeatureTransport>,
    lcd_transport: Option<Box<dyn LcdTransport>>,
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
            capabilities: if self.lcd_transport.is_some() {
                vec![Capability::HardwareRgb, Capability::Display, Capability::Clock]
            } else {
                vec![Capability::HardwareRgb, Capability::Clock]
            },
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

    fn upload_lcd(
        &mut self,
        context: &OperationContext,
        upload: &LcdUpload,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<(), DriverError> {
        crate::lcd::upload(self, context, upload, progress)
    }

    fn sync_clock(
        &mut self,
        context: &OperationContext,
        clock: DeviceClock,
    ) -> Result<(), DriverError> {
        crate::lcd::sync_clock(self, context, clock)
    }
}

impl AulaF108ProSession {
    pub(crate) fn send(
        &self,
        context: &OperationContext,
        report: [u8; FEATURE_REPORT_SIZE],
        readback: bool,
    ) -> Result<(), DriverError> {
        self.send_with_readback(
            context,
            report,
            if readback {
                FeatureReadback::Acknowledgment
            } else {
                FeatureReadback::None
            },
        )
    }

    pub(crate) fn send_with_readback(
        &self,
        context: &OperationContext,
        report: [u8; FEATURE_REPORT_SIZE],
        readback: FeatureReadback,
    ) -> Result<(), DriverError> {
        context.check()?;
        self.transport
            .set_feature_report(&report)
            .map_err(map_transport_error)?;
        context.sleep(COMMAND_DELAY)?;

        if !matches!(readback, FeatureReadback::None) {
            context.check()?;
            let response = self
                .transport
                .get_feature_report()
                .map_err(map_transport_error)?;
            match readback {
                FeatureReadback::Acknowledgment => validate_ack(&response),
                FeatureReadback::ClockData => crate::lcd::validate_clock_readback(&report, &response),
                FeatureReadback::None => unreachable!(),
            }
            .map_err(|error| {
                DriverError::Protocol(format!(
                    "feature readback after sent {:02X?}: {error}",
                    &report[..16],
                ))
            })?;
            context.sleep(COMMAND_DELAY)?;
        }
        Ok(())
    }

    pub(crate) fn begin_transaction(&self, context: &OperationContext) -> Result<(), DriverError> {
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

    pub(crate) fn apply_transaction(&self, context: &OperationContext) -> Result<(), DriverError> {
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

#[derive(Clone, Copy)]
pub(crate) enum FeatureReadback {
    None,
    Acknowledgment,
    ClockData,
}

fn open_lcd_transport(
    candidate: &DeviceCandidate,
) -> Result<Option<Box<dyn LcdTransport>>, DriverError> {
    let interfaces = enumerate_feature_devices(VENDOR_ID, PRODUCT_ID, LCD_USAGE_PAGE)
        .map_err(map_transport_error)?;
    let selected = interfaces.iter().find(|interface| {
        match (&candidate.serial_number, &interface.serial_number) {
            (Some(candidate_serial), Some(interface_serial)) => candidate_serial == interface_serial,
            _ => true,
        }
    });
    selected
        .map(|interface| open_lcd_path(&interface.path).map_err(map_transport_error))
        .transpose()
}

#[derive(Debug, Clone, Copy)]
struct F108LightingEffect {
    mode: u8,
    name: &'static str,
    config_func: u8,
    color: LightingColorControl,
    direction: Option<LightingDirectionControl>,
}

const F108_LIGHTING_EFFECTS: [F108LightingEffect; 20] = [
    F108LightingEffect {
        mode: 0,
        name: "Off",
        config_func: 0x00,
        color: LightingColorControl::None,
        direction: None,
    },
    F108LightingEffect {
        mode: 1,
        name: "Static",
        config_func: 0x31,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 2,
        name: "SingleOn",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 3,
        name: "SingleOff",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 4,
        name: "Glittering",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 5,
        name: "Falling",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 6,
        name: "Colourful",
        config_func: 0x03,
        color: LightingColorControl::RainbowOnly,
        direction: None,
    },
    F108LightingEffect {
        mode: 7,
        name: "Breath",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 8,
        name: "Spectrum",
        config_func: 0x03,
        color: LightingColorControl::RainbowOnly,
        direction: None,
    },
    F108LightingEffect {
        mode: 9,
        name: "Outward",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 10,
        name: "Scrolling",
        config_func: 0x3B,
        color: LightingColorControl::ColorOrRainbow,
        direction: Some(LightingDirectionControl::LeftRight),
    },
    F108LightingEffect {
        mode: 11,
        name: "Rolling",
        config_func: 0x37,
        color: LightingColorControl::ColorOrRainbow,
        direction: Some(LightingDirectionControl::UpDown),
    },
    F108LightingEffect {
        mode: 12,
        name: "Rotating",
        config_func: 0x37,
        color: LightingColorControl::ColorOrRainbow,
        direction: Some(LightingDirectionControl::UpDown),
    },
    F108LightingEffect {
        mode: 13,
        name: "Explode",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 14,
        name: "Launch",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 15,
        name: "Ripples",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 16,
        name: "Flowing",
        config_func: 0x37,
        color: LightingColorControl::ColorOrRainbow,
        direction: Some(LightingDirectionControl::UpDown),
    },
    F108LightingEffect {
        mode: 17,
        name: "Pulsating",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
    F108LightingEffect {
        mode: 18,
        name: "Tilt",
        config_func: 0x37,
        color: LightingColorControl::ColorOrRainbow,
        direction: Some(LightingDirectionControl::UpDown),
    },
    F108LightingEffect {
        mode: 19,
        name: "Shuttle",
        config_func: 0x33,
        color: LightingColorControl::ColorOrRainbow,
        direction: None,
    },
];

const CONFIG_BRIGHTNESS: u8 = 0x01;
const CONFIG_SPEED: u8 = 0x02;

pub fn lighting_effects() -> Vec<LightingEffectDescriptor> {
    F108_LIGHTING_EFFECTS
        .iter()
        .map(|effect| LightingEffectDescriptor {
            mode: effect.mode,
            name: effect.name.into(),
            color: effect.color,
            brightness: effect.config_func & CONFIG_BRIGHTNESS != 0,
            speed: effect.config_func & CONFIG_SPEED != 0,
            direction: effect.direction,
        })
        .collect()
}

fn effect_for_mode(mode: u8) -> Option<F108LightingEffect> {
    F108_LIGHTING_EFFECTS
        .iter()
        .copied()
        .find(|effect| effect.mode == mode)
}

fn validate_candidate(candidate: &DeviceCandidate) -> Result<(), DriverError> {
    if candidate.driver_id != DRIVER_ID
        || candidate.connection != Connection::Usb
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

pub(crate) fn build_lighting_payload(
    config: LightingConfig,
) -> Result<[u8; FEATURE_REPORT_SIZE], DriverError> {
    let effect = effect_for_mode(config.mode).ok_or_else(|| {
        DriverError::InvalidConfiguration(format!("lighting mode {} is outside 0..19", config.mode))
    })?;

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

    match effect.color {
        LightingColorControl::None | LightingColorControl::RainbowOnly => {}
        LightingColorControl::ColorOrRainbow => {
            payload[1] = config.r;
            payload[2] = config.g;
            payload[3] = config.b;
            payload[8] = u8::from(config.colorful);
        }
    }

    if effect.config_func & CONFIG_BRIGHTNESS != 0 {
        payload[9] = config.brightness;
    }
    if effect.config_func & CONFIG_SPEED != 0 {
        payload[10] = config.speed;
    }
    if effect.direction.is_some() {
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
            "unexpected acknowledgment byte 3: 0x{:02X}; received {:02X?}",
            response[3], &response[..16]
        )));
    }
    Ok(())
}

fn validate_data_readback(
    payload: &[u8],
    response: &[u8; FEATURE_REPORT_SIZE],
    family: &str,
) -> Result<(), DriverError> {
    let first = payload.get(..FEATURE_REPORT_SIZE);
    let last = payload.get(payload.len().saturating_sub(FEATURE_REPORT_SIZE)..);
    if first == Some(response.as_slice()) || last == Some(response.as_slice()) {
        return Ok(());
    }
    if validate_ack(response).is_ok() {
        return Ok(());
    }
    Err(DriverError::Protocol(format!(
        "{family} payload readback did not match ACK or transmitted first/last report; received {:02X?}; first {:02X?}; last {:02X?}",
        response, first.unwrap_or(&[]), last.unwrap_or(&[]),
    )))
}

fn map_transport_error(error: HidTransportError) -> DriverError {
    match error {
        HidTransportError::NotFound => DriverError::NotFound,
        HidTransportError::Disconnected(message) => DriverError::Disconnected(message),
        HidTransportError::PermissionDenied(message) => DriverError::PermissionDenied(message),
        HidTransportError::Busy(message) => DriverError::Busy(format!(
            "Another application appears to be using the keyboard configuration interface. Close the OEM application and reconnect. Details: {message}"
        )),
        HidTransportError::Timeout(message) => DriverError::Timeout(message),
        HidTransportError::UnsupportedPlatform => DriverError::UnsupportedPlatform,
        HidTransportError::Backend(message) => DriverError::Transport(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> DeviceCandidate {
        DeviceCandidate {
            driver_id: DRIVER_ID.into(),
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
        assert_eq!(manifest.driver_id, DRIVER_ID);
        assert_eq!(manifest.device_model_id, "aula.f108pro");
        assert_eq!(manifest.supported_connections, vec![Connection::Usb]);
        assert_eq!(
            manifest.capabilities,
            vec![Capability::HardwareRgb, Capability::Display, Capability::Clock]
        );
        assert_eq!(manifest.layout_ids, vec!["aula.f108pro.ansi"]);
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
    fn exposes_all_twenty_oem_effects_in_order() {
        let effects = lighting_effects();
        assert_eq!(effects.len(), 20);
        for (mode, effect) in effects.iter().enumerate() {
            assert_eq!(effect.mode as usize, mode);
        }

        assert_eq!(effects[0].name, "Off");
        assert_eq!(effects[1].name, "Static");
        assert_eq!(effects[19].name, "Shuttle");
    }

    #[test]
    fn effect_parameter_metadata_matches_oem_mode_table() {
        let effects = lighting_effects();

        let off = &effects[0];
        assert_eq!(off.color, LightingColorControl::None);
        assert!(!off.brightness);
        assert!(!off.speed);
        assert_eq!(off.direction, None);

        let static_mode = &effects[1];
        assert_eq!(static_mode.color, LightingColorControl::ColorOrRainbow);
        assert!(static_mode.brightness);
        assert!(!static_mode.speed);
        assert_eq!(static_mode.direction, None);

        for mode in [6usize, 8usize] {
            assert_eq!(effects[mode].color, LightingColorControl::RainbowOnly);
            assert!(effects[mode].brightness);
            assert!(effects[mode].speed);
            assert_eq!(effects[mode].direction, None);
        }

        assert_eq!(
            effects[10].direction,
            Some(LightingDirectionControl::LeftRight)
        );
        for mode in [11usize, 12usize, 16usize, 18usize] {
            assert_eq!(
                effects[mode].direction,
                Some(LightingDirectionControl::UpDown)
            );
        }
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
        assert_eq!(payload[11], 0);
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
    fn static_zeros_unused_speed_and_direction() {
        let payload = build_lighting_payload(LightingConfig {
            mode: 1,
            r: 10,
            g: 20,
            b: 30,
            brightness: 4,
            speed: 5,
            direction: 1,
            colorful: false,
        })
        .unwrap();

        assert_eq!(&payload[1..4], &[10, 20, 30]);
        assert_eq!(payload[9], 4);
        assert_eq!(payload[10], 0);
        assert_eq!(payload[11], 0);
    }

    #[test]
    fn rainbow_only_modes_ignore_stale_color_and_colorful_values() {
        for mode in [6u8, 8u8] {
            let payload = build_lighting_payload(LightingConfig {
                mode,
                r: 255,
                g: 1,
                b: 2,
                brightness: 5,
                speed: 4,
                direction: 1,
                colorful: true,
            })
            .unwrap();

            assert_eq!(&payload[1..4], &[0, 0, 0]);
            assert_eq!(payload[8], 0);
            assert_eq!(payload[9], 5);
            assert_eq!(payload[10], 4);
            assert_eq!(payload[11], 0);
        }
    }

    #[test]
    fn only_direction_modes_emit_direction_byte() {
        for mode in 0u8..=19 {
            let payload = build_lighting_payload(LightingConfig {
                mode,
                r: 1,
                g: 2,
                b: 3,
                brightness: 5,
                speed: 3,
                direction: 1,
                colorful: true,
            })
            .unwrap();

            let expected = matches!(mode, 10 | 11 | 12 | 16 | 18);
            assert_eq!(payload[11] == 1, expected, "mode {mode}");
        }
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
