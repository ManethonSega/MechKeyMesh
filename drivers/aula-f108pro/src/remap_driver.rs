use super::{
    AulaF108ProSession, CONFIG_USAGE_PAGE, PRODUCT_ID, VENDOR_ID, f108pro_layout,
    lighting_effects, map_transport_error, validate_candidate,
};
use crate::remap;
use mechkeymesh_driver_api::{
    Capability, Connection, DeviceCandidate, DeviceDescriptor, DeviceMatch, DeviceSession,
    DriverError, DriverManifest, KeyboardDriver, KeyAssignment, KeymapLayer, LightingConfig,
    LightingEffectDescriptor, NormalRemapCapabilities, OperationContext, TransportKind,
    TransportRequirement,
};
use mechkeymesh_hid::{
    FEATURE_REPORT_SIZE, enumerate_feature_devices, open_feature_path,
};

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
            driver_id: "aula.f108pro.usb".into(),
            device_model_id: "aula.f108pro".into(),
            vendor: "AULA".into(),
            device_name: "AULA F108 Pro".into(),
            supported_connections: vec![Connection::Usb],
            capabilities: vec![Capability::Keys, Capability::HardwareRgb],
            layout_ids: vec!["aula.f108pro.ansi".into()],
            matchers: vec![DeviceMatch {
                vendor_id: VENDOR_ID,
                product_id: PRODUCT_ID,
                usage_page: Some(CONFIG_USAGE_PAGE),
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

    fn layouts(&self) -> Vec<mechkeymesh_layout::Layout> {
        vec![f108pro_layout()]
    }

    fn hardware_lighting_effects(&self) -> Vec<LightingEffectDescriptor> {
        lighting_effects()
    }

    fn normal_remap_capabilities(&self) -> Option<NormalRemapCapabilities> {
        Some(remap::normal_remap_capabilities())
    }

    fn validate_keymap(
        &self,
        layer: KeymapLayer,
        assignments: &[KeyAssignment],
    ) -> Result<(), DriverError> {
        remap::validate_keymap(layer, assignments)
    }

    fn discover(
        &self,
        context: &OperationContext,
    ) -> Result<Vec<DeviceCandidate>, DriverError> {
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

        Ok(Box::new(AulaF108ProRemapSession {
            inner: AulaF108ProSession {
                candidate: candidate.clone(),
                transport,
            },
        }))
    }
}

struct AulaF108ProRemapSession {
    inner: AulaF108ProSession,
}

impl DeviceSession for AulaF108ProRemapSession {
    fn descriptor(&self) -> DeviceDescriptor {
        let mut descriptor = self.inner.descriptor();
        descriptor.capabilities = vec![Capability::Keys, Capability::HardwareRgb];
        descriptor
    }

    fn initialize(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        self.inner.initialize(context)
    }

    fn health_check(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        self.inner.health_check(context)
    }

    fn set_lighting(
        &mut self,
        context: &OperationContext,
        config: LightingConfig,
    ) -> Result<(), DriverError> {
        self.inner.set_lighting(context, config)
    }

    fn set_keymap(
        &mut self,
        context: &OperationContext,
        layer: KeymapLayer,
        assignments: &[KeyAssignment],
    ) -> Result<(), DriverError> {
        match layer {
            KeymapLayer::Normal => remap::send_normal_keymap(&mut self.inner, context, assignments),
            KeymapLayer::Fn => Err(DriverError::UnsupportedCapability(
                "F108 Fn remapping is implemented in R7, not R6",
            )),
        }
    }

    fn close(&mut self) -> Result<(), DriverError> {
        self.inner.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::{KeyAction, MediaAction, Modifier, MouseAction, WindowsAction};

    #[test]
    fn r6_manifest_exposes_only_implemented_key_and_rgb_capabilities() {
        let driver = AulaF108ProRemapDriver::new();
        let manifest = driver.manifest();
        assert_eq!(
            manifest.capabilities,
            vec![Capability::Keys, Capability::HardwareRgb]
        );
    }

    #[test]
    fn r6_capability_matrix_excludes_unverified_host_and_mouse_actions() {
        let capabilities = AulaF108ProRemapDriver::new()
            .normal_remap_capabilities()
            .unwrap();
        assert!(capabilities.disable);
        assert!(capabilities.keyboard_key);
        assert!(capabilities.shortcut);
        assert!(!capabilities.open_application);
        assert!(!capabilities.send_text);
        assert!(!capabilities.mouse_actions.contains(&MouseAction::DoubleClick));
        assert!(capabilities.media_actions.contains(&MediaAction::Mute));
        assert!(capabilities.windows_actions.contains(&WindowsAction::Calculator));
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
        assert!(driver.validate_keymap(KeymapLayer::Normal, &accepted).is_ok());

        let rejected = [KeyAssignment {
            device_key_index: 2,
            action: KeyAction::OpenApplication {
                path: "notepad.exe".into(),
            },
        }];
        assert!(driver.validate_keymap(KeymapLayer::Normal, &rejected).is_err());
    }
}
