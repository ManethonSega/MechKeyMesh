use super::{
    AulaF108ProSession, CONFIG_USAGE_PAGE, DRIVER_ID, PRODUCT_ID, VENDOR_ID, f108pro_layout,
    lighting_effects, map_transport_error, validate_candidate,
};
use crate::{live_rgb, macro_bank, per_key_rgb, remap, settings, wireless};
use mechkeymesh_driver_api::{
    Capability, Connection, DeviceCandidate, DeviceDescriptor, DeviceMatch, DeviceSession,
    DeviceSettings, DeviceSettingsDescriptor, DriverError, DriverManifest, FnActivationMode,
    FnActivationModeOption, FnLayerDescriptor, HardwareMacroProgram, KeyAssignment, KeyboardDriver,
    KeymapCapabilities, KeymapLayer, LightingConfig, LightingEffectDescriptor, LiveRgbFrame,
    MacroBankDescriptor, OperationContext, PerKeyRgbCapabilities, PerKeyRgbConfig,
    PerKeyRgbReadback, TransportKind, TransportRequirement,
};
use mechkeymesh_hid::{
    FEATURE_REPORT_SIZE, INTERRUPT_REPORT_SIZE, enumerate_feature_devices,
    enumerate_interrupt_devices, open_feature_path, open_interrupt_path,
};

const USB_CAPABILITIES: &[Capability] = &[
    Capability::Keys,
    Capability::FnLayer,
    Capability::Macros,
    Capability::HardwareRgb,
    Capability::PerKeyRgb,
    Capability::LiveRgb,
    Capability::Music,
    Capability::DeviceSettings,
];

pub struct AulaF108ProRemapDriver;

impl AulaF108ProRemapDriver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AulaF108ProRemapDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardDriver for AulaF108ProRemapDriver {
    fn manifest(&self) -> DriverManifest {
        DriverManifest {
            driver_id: DRIVER_ID.into(),
            device_model_id: "aula.f108pro".into(),
            vendor: "AULA".into(),
            device_name: "AULA F108 Pro".into(),
            supported_connections: vec![Connection::Usb, Connection::Wireless24G],
            capabilities: vec![
                Capability::Keys,
                Capability::FnLayer,
                Capability::Macros,
                Capability::HardwareRgb,
                Capability::PerKeyRgb,
                Capability::LiveRgb,
                Capability::Music,
                Capability::DeviceSettings,
                Capability::Battery,
            ],
            layout_ids: vec!["aula.f108pro.ansi".into()],
            matchers: vec![
                DeviceMatch {
                    vendor_id: VENDOR_ID,
                    product_id: PRODUCT_ID,
                    usage_page: Some(CONFIG_USAGE_PAGE),
                    interface_number: None,
                },
                DeviceMatch {
                    vendor_id: wireless::WIRELESS_VENDOR_ID,
                    product_id: wireless::WIRELESS_PRODUCT_ID,
                    usage_page: Some(wireless::WIRELESS_USAGE_PAGE),
                    interface_number: Some(wireless::WIRELESS_INTERFACE_NUMBER),
                },
            ],
            transport_requirements: vec![
                TransportRequirement {
                    kind: TransportKind::HidFeature,
                    usage_page: Some(CONFIG_USAGE_PAGE),
                    interface_number: None,
                    report_size: Some(FEATURE_REPORT_SIZE),
                },
                TransportRequirement {
                    kind: TransportKind::HidInterrupt,
                    usage_page: Some(wireless::WIRELESS_USAGE_PAGE),
                    interface_number: Some(wireless::WIRELESS_INTERFACE_NUMBER),
                    report_size: Some(INTERRUPT_REPORT_SIZE),
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

    fn keymap_capabilities(&self, layer: KeymapLayer) -> Option<KeymapCapabilities> {
        Some(remap::keymap_capabilities(layer))
    }

    fn fn_layer_descriptor(&self) -> Option<FnLayerDescriptor> {
        Some(FnLayerDescriptor {
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
            note: "OEM binary maps fn_switch raw 0 to Toggle and raw 1 to Momentary. R11 writes it only as part of the complete 04 17 / 00 01 settings block; physical verification remains pending.".into(),
        })
    }

    fn macro_bank_descriptor(&self) -> Option<MacroBankDescriptor> {
        Some(macro_bank::descriptor())
    }

    fn device_settings_descriptor(&self) -> Option<DeviceSettingsDescriptor> {
        Some(settings::descriptor())
    }

    fn validate_macro_bank(&self, programs: &[HardwareMacroProgram]) -> Result<(), DriverError> {
        macro_bank::validate_bank(programs)
    }

    fn validate_device_settings(
        &self,
        device_settings: &DeviceSettings,
    ) -> Result<(), DriverError> {
        settings::validate(device_settings)
    }

    fn per_key_rgb_capabilities(&self) -> Option<PerKeyRgbCapabilities> {
        Some(per_key_rgb::capabilities())
    }

    fn validate_per_key_rgb(&self, config: &PerKeyRgbConfig) -> Result<(), DriverError> {
        per_key_rgb::validate_config(config)
    }

    fn validate_keymap(
        &self,
        layer: KeymapLayer,
        assignments: &[KeyAssignment],
    ) -> Result<(), DriverError> {
        remap::validate_keymap(layer, assignments)
    }

    fn discover(&self, context: &OperationContext) -> Result<Vec<DeviceCandidate>, DriverError> {
        context.check()?;
        let wired_devices = enumerate_feature_devices(VENDOR_ID, PRODUCT_ID, CONFIG_USAGE_PAGE)
            .map_err(map_transport_error)?;
        context.check()?;

        let mut candidates: Vec<DeviceCandidate> = wired_devices
            .into_iter()
            .map(|device| DeviceCandidate {
                driver_id: DRIVER_ID.into(),
                instance_id: device.path,
                name: "AULA F108 Pro USB".into(),
                connection: Connection::Usb,
                vendor_id: Some(device.vendor_id),
                product_id: Some(device.product_id),
                usage_page: Some(device.usage_page),
                interface_number: Some(device.interface_number),
                serial_number: device.serial_number,
            })
            .collect();

        let wireless_devices = enumerate_interrupt_devices(
            wireless::WIRELESS_VENDOR_ID,
            wireless::WIRELESS_PRODUCT_ID,
            wireless::WIRELESS_USAGE_PAGE,
            wireless::WIRELESS_INTERFACE_NUMBER,
        )
        .map_err(map_transport_error)?;

        for device in wireless_devices {
            context.check()?;
            let candidate = DeviceCandidate {
                driver_id: DRIVER_ID.into(),
                instance_id: device.path,
                name: "AULA F108 Pro 2.4G".into(),
                connection: Connection::Wireless24G,
                vendor_id: Some(device.vendor_id),
                product_id: Some(device.product_id),
                usage_page: Some(device.usage_page),
                interface_number: Some(device.interface_number),
                serial_number: device.serial_number,
            };
            let Ok(transport) = open_interrupt_path(&candidate.instance_id) else {
                continue;
            };
            let session = wireless::AulaF108WirelessSession::new(candidate.clone(), transport);
            if session.probe(context).is_ok() {
                candidates.push(candidate);
            }
        }

        Ok(candidates)
    }

    fn open(
        &self,
        context: &OperationContext,
        candidate: &DeviceCandidate,
    ) -> Result<Box<dyn DeviceSession>, DriverError> {
        context.check()?;
        let inner = match candidate.connection {
            Connection::Usb => {
                validate_candidate(candidate)?;
                let transport =
                    open_feature_path(&candidate.instance_id).map_err(map_transport_error)?;
                RemapTransportSession::Usb(AulaF108ProSession {
                    candidate: candidate.clone(),
                    transport,
                })
            }
            Connection::Wireless24G => {
                validate_wireless_candidate(candidate)?;
                let transport =
                    open_interrupt_path(&candidate.instance_id).map_err(map_transport_error)?;
                RemapTransportSession::Wireless(wireless::AulaF108WirelessSession::new(
                    candidate.clone(),
                    transport,
                ))
            }
            _ => {
                return Err(DriverError::InvalidConfiguration(
                    "candidate uses an unsupported F108 connection".into(),
                ));
            }
        };
        context.check()?;
        Ok(Box::new(AulaF108ProRemapSession { inner }))
    }
}

enum RemapTransportSession {
    Usb(AulaF108ProSession),
    Wireless(wireless::AulaF108WirelessSession),
}

struct AulaF108ProRemapSession {
    inner: RemapTransportSession,
}

impl DeviceSession for AulaF108ProRemapSession {
    fn descriptor(&self) -> DeviceDescriptor {
        match &self.inner {
            RemapTransportSession::Usb(inner) => {
                let mut descriptor = inner.descriptor();
                descriptor.capabilities = USB_CAPABILITIES.to_vec();
                descriptor.name = "AULA F108 Pro USB".into();
                descriptor
            }
            RemapTransportSession::Wireless(inner) => inner.descriptor(),
        }
    }

    fn initialize(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => inner.initialize(context),
            RemapTransportSession::Wireless(inner) => inner.initialize(context),
        }
    }

    fn health_check(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => inner.health_check(context),
            RemapTransportSession::Wireless(inner) => inner.health_check(context),
        }
    }

    fn set_lighting(
        &mut self,
        context: &OperationContext,
        config: LightingConfig,
    ) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => inner.set_lighting(context, config),
            RemapTransportSession::Wireless(inner) => inner.set_lighting(context, config),
        }
    }

    fn set_keymap(
        &mut self,
        context: &OperationContext,
        layer: KeymapLayer,
        assignments: &[KeyAssignment],
    ) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => match layer {
                KeymapLayer::Normal => remap::send_normal_keymap(inner, context, assignments),
                KeymapLayer::Fn => remap::send_fn_keymap(inner, context, assignments),
            },
            RemapTransportSession::Wireless(inner) => inner.set_keymap(context, layer, assignments),
        }
    }

    fn set_macros(
        &mut self,
        context: &OperationContext,
        programs: &[HardwareMacroProgram],
    ) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => macro_bank::send_bank(inner, context, programs),
            RemapTransportSession::Wireless(_) => {
                Err(DriverError::UnsupportedCapability("macros over 2.4 GHz"))
            }
        }
    }

    fn set_device_settings(
        &mut self,
        context: &OperationContext,
        device_settings: &DeviceSettings,
    ) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => settings::send(inner, context, device_settings),
            RemapTransportSession::Wireless(inner) => {
                inner.set_device_settings(context, device_settings)
            }
        }
    }

    fn set_per_key_rgb(
        &mut self,
        context: &OperationContext,
        config: &PerKeyRgbConfig,
    ) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => {
                per_key_rgb::send_per_key_rgb(inner, context, config)
            }
            RemapTransportSession::Wireless(inner) => inner.set_per_key_rgb(context, config),
        }
    }

    fn read_per_key_rgb(
        &mut self,
        context: &OperationContext,
        led_indices: &[u16],
    ) -> Result<PerKeyRgbReadback, DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => {
                per_key_rgb::read_per_key_rgb(inner, context, led_indices)
            }
            RemapTransportSession::Wireless(_) => Err(DriverError::UnsupportedCapability(
                "stored RGB readback over 2.4 GHz",
            )),
        }
    }

    fn send_live_rgb_frame(
        &mut self,
        context: &OperationContext,
        frame: &LiveRgbFrame,
    ) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => live_rgb::send_frame(inner, context, frame),
            RemapTransportSession::Wireless(_) => {
                Err(DriverError::UnsupportedCapability("live RGB over 2.4 GHz"))
            }
        }
    }

    fn close(&mut self) -> Result<(), DriverError> {
        match &mut self.inner {
            RemapTransportSession::Usb(inner) => inner.close(),
            RemapTransportSession::Wireless(_) => Ok(()),
        }
    }
}

fn validate_wireless_candidate(candidate: &DeviceCandidate) -> Result<(), DriverError> {
    if candidate.driver_id != DRIVER_ID
        || candidate.connection != Connection::Wireless24G
        || candidate.vendor_id != Some(wireless::WIRELESS_VENDOR_ID)
        || candidate.product_id != Some(wireless::WIRELESS_PRODUCT_ID)
        || candidate.usage_page != Some(wireless::WIRELESS_USAGE_PAGE)
        || candidate.interface_number != Some(wireless::WIRELESS_INTERFACE_NUMBER)
    {
        return Err(DriverError::InvalidConfiguration(
            "candidate does not match the AULA F108 Pro 2.4 GHz configuration interface".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::{KeyAction, MediaAction, Modifier, MouseAction, WindowsAction};

    #[test]
    fn r12_manifest_exposes_both_connection_variants_and_union_capabilities() {
        let driver = AulaF108ProRemapDriver::new();
        let manifest = driver.manifest();
        assert_eq!(
            manifest.capabilities,
            vec![
                Capability::Keys,
                Capability::FnLayer,
                Capability::Macros,
                Capability::HardwareRgb,
                Capability::PerKeyRgb,
                Capability::LiveRgb,
                Capability::Music,
                Capability::DeviceSettings,
                Capability::Battery,
            ]
        );
        assert_eq!(
            manifest.supported_connections,
            vec![Connection::Usb, Connection::Wireless24G]
        );
        assert_eq!(manifest.matchers.len(), 2);
        assert_eq!(manifest.matchers[1].vendor_id, wireless::WIRELESS_VENDOR_ID);
        assert_eq!(
            manifest.matchers[1].product_id,
            wireless::WIRELESS_PRODUCT_ID
        );
        assert_eq!(
            manifest.matchers[1].interface_number,
            Some(wireless::WIRELESS_INTERFACE_NUMBER)
        );
    }

    #[test]
    fn macro_descriptor_keeps_unverified_hardware_status_explicit() {
        let descriptor = AulaF108ProRemapDriver::new()
            .macro_bank_descriptor()
            .unwrap();
        assert_eq!(descriptor.slot_count, 100);
        assert_eq!(descriptor.minimum_delay_milliseconds, 10);
        assert_eq!(descriptor.maximum_loop_count, 99);
        assert!(!descriptor.hardware_write_verified);
    }

    #[test]
    fn r7_capability_matrix_excludes_unverified_host_and_mouse_actions() {
        let capabilities = AulaF108ProRemapDriver::new()
            .keymap_capabilities(KeymapLayer::Normal)
            .unwrap();
        assert!(capabilities.disable);
        assert!(capabilities.keyboard_key);
        assert!(capabilities.shortcut);
        assert!(!capabilities.open_application);
        assert!(!capabilities.send_text);
        assert!(
            !capabilities
                .mouse_actions
                .contains(&MouseAction::DoubleClick)
        );
        assert!(capabilities.media_actions.contains(&MediaAction::Mute));
        assert!(
            capabilities
                .windows_actions
                .contains(&WindowsAction::Calculator)
        );
    }

    #[test]
    fn fn_layer_capabilities_protect_native_controls() {
        let driver = AulaF108ProRemapDriver::new();
        let capabilities = driver.keymap_capabilities(KeymapLayer::Fn).unwrap();
        assert!(
            capabilities
                .protected_keys
                .iter()
                .any(|restriction| restriction.device_key_index == 1)
        );
        assert!(
            capabilities
                .protected_keys
                .iter()
                .any(|restriction| restriction.device_key_index == 96)
        );
        assert!(capabilities.keyboard_key);
        assert!(capabilities.shortcut);
    }

    #[test]
    fn fn_activation_enum_matches_oem_binary_mapping() {
        let descriptor = AulaF108ProRemapDriver::new().fn_layer_descriptor().unwrap();
        assert_eq!(descriptor.modes[0].mode, FnActivationMode::Toggle);
        assert_eq!(descriptor.modes[0].raw_value, 0);
        assert_eq!(descriptor.modes[1].mode, FnActivationMode::Momentary);
        assert_eq!(descriptor.modes[1].raw_value, 1);
        assert!(!descriptor.hardware_write_verified);
        assert_eq!(descriptor.current_mode, None);
    }

    #[test]
    fn per_key_rgb_capability_is_enabled_and_validated() {
        let driver = AulaF108ProRemapDriver::new();
        let capabilities = driver.per_key_rgb_capabilities().unwrap();
        assert_eq!(capabilities.brightness_min, 0);
        assert_eq!(capabilities.brightness_max, 5);
        assert!(capabilities.readback_supported);

        assert!(
            driver
                .validate_per_key_rgb(&PerKeyRgbConfig {
                    brightness: 5,
                    colors: vec![],
                })
                .is_ok()
        );
        assert!(
            driver
                .validate_per_key_rgb(&PerKeyRgbConfig {
                    brightness: 6,
                    colors: vec![],
                })
                .is_err()
        );
    }

    #[test]
    fn driver_validation_accepts_verified_actions_and_rejects_host_actions() {
        let driver = AulaF108ProRemapDriver::new();
        let accepted = vec![
            KeyAssignment {
                device_key_index: 2,
                action: KeyAction::ModifiedKey {
                    modifiers: vec![Modifier::LeftCtrl],
                    key: 0x06,
                },
            },
            KeyAssignment {
                device_key_index: 3,
                action: KeyAction::Windows {
                    action: WindowsAction::ShowDesktop,
                },
            },
        ];
        assert!(
            driver
                .validate_keymap(KeymapLayer::Normal, &accepted)
                .is_ok()
        );

        let rejected = [KeyAssignment {
            device_key_index: 2,
            action: KeyAction::OpenApplication {
                path: "notepad.exe".into(),
            },
        }];
        assert!(
            driver
                .validate_keymap(KeymapLayer::Normal, &rejected)
                .is_err()
        );
    }
}
