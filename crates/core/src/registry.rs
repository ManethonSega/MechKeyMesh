use mechkeymesh_driver_api::{
    DeviceCandidate, DriverError, DriverManifest, ErrorClass, KeyboardDriver, OperationContext,
};
use mechkeymesh_layout::{Layout, LayoutError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("driver id is empty")]
    EmptyDriverId,
    #[error("driver {0} has an empty device-model id")]
    EmptyDeviceModelId(String),
    #[error("driver already registered: {0}")]
    DuplicateDriverId(String),
    #[error("driver {driver_id} declares layout id more than once: {layout_id}")]
    DuplicateDeclaredLayoutId {
        driver_id: String,
        layout_id: String,
    },
    #[error("driver {driver_id} provides layout id more than once: {layout_id}")]
    DuplicateProvidedLayoutId {
        driver_id: String,
        layout_id: String,
    },
    #[error("driver layout declaration mismatch: {0}")]
    LayoutDeclarationMismatch(String),
    #[error(
        "driver {driver_id} provides layout {layout_id} for device model {layout_device_model_id}, expected {driver_device_model_id}"
    )]
    LayoutDeviceModelMismatch {
        driver_id: String,
        layout_id: String,
        layout_device_model_id: String,
        driver_device_model_id: String,
    },
    #[error("driver {driver_id} provides invalid layout {layout_id}: {source}")]
    InvalidDriverLayout {
        driver_id: String,
        layout_id: String,
        #[source]
        source: LayoutError,
    },
    #[error(
        "layout {layout_id} is already owned by driver {existing_driver_id}; driver {new_driver_id} cannot register it"
    )]
    DuplicateLayoutId {
        layout_id: String,
        existing_driver_id: String,
        new_driver_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverProbeFailure {
    pub driver_id: String,
    pub class: ErrorClass,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryReport {
    pub candidates: Vec<DeviceCandidate>,
    pub failures: Vec<DriverProbeFailure>,
}

pub struct DriverRegistry {
    drivers: BTreeMap<String, Arc<dyn KeyboardDriver>>,
    layouts: BTreeMap<String, Layout>,
    layout_owners: BTreeMap<String, String>,
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            drivers: BTreeMap::new(),
            layouts: BTreeMap::new(),
            layout_owners: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, driver: Arc<dyn KeyboardDriver>) -> Result<(), RegistryError> {
        let manifest = driver.manifest();
        if manifest.driver_id.trim().is_empty() {
            return Err(RegistryError::EmptyDriverId);
        }
        if manifest.device_model_id.trim().is_empty() {
            return Err(RegistryError::EmptyDeviceModelId(manifest.driver_id));
        }
        if self.drivers.contains_key(&manifest.driver_id) {
            return Err(RegistryError::DuplicateDriverId(manifest.driver_id));
        }

        let mut declared_seen = HashSet::new();
        for layout_id in &manifest.layout_ids {
            if !declared_seen.insert(layout_id.clone()) {
                return Err(RegistryError::DuplicateDeclaredLayoutId {
                    driver_id: manifest.driver_id,
                    layout_id: layout_id.clone(),
                });
            }
        }

        let provided_layouts = driver.layouts();
        let mut provided_seen = HashSet::new();
        for layout in &provided_layouts {
            if !provided_seen.insert(layout.id.clone()) {
                return Err(RegistryError::DuplicateProvidedLayoutId {
                    driver_id: manifest.driver_id.clone(),
                    layout_id: layout.id.clone(),
                });
            }
            layout
                .validate()
                .map_err(|source| RegistryError::InvalidDriverLayout {
                    driver_id: manifest.driver_id.clone(),
                    layout_id: layout.id.clone(),
                    source,
                })?;
            if layout.device_model_id != manifest.device_model_id {
                return Err(RegistryError::LayoutDeviceModelMismatch {
                    driver_id: manifest.driver_id.clone(),
                    layout_id: layout.id.clone(),
                    layout_device_model_id: layout.device_model_id.clone(),
                    driver_device_model_id: manifest.device_model_id.clone(),
                });
            }
            if let Some(existing_driver_id) = self.layout_owners.get(&layout.id) {
                return Err(RegistryError::DuplicateLayoutId {
                    layout_id: layout.id.clone(),
                    existing_driver_id: existing_driver_id.clone(),
                    new_driver_id: manifest.driver_id.clone(),
                });
            }
        }

        let mut declared = manifest.layout_ids.clone();
        declared.sort();
        let mut provided: Vec<String> = provided_layouts
            .iter()
            .map(|layout| layout.id.clone())
            .collect();
        provided.sort();

        if declared != provided {
            return Err(RegistryError::LayoutDeclarationMismatch(format!(
                "{} declares {:?} but provides {:?}",
                manifest.driver_id, declared, provided
            )));
        }

        for layout in provided_layouts {
            self.layout_owners
                .insert(layout.id.clone(), manifest.driver_id.clone());
            self.layouts.insert(layout.id.clone(), layout);
        }
        self.drivers.insert(manifest.driver_id, driver);
        Ok(())
    }

    pub fn manifests(&self) -> Vec<DriverManifest> {
        self.drivers
            .values()
            .map(|driver| driver.manifest())
            .collect()
    }

    pub fn get(&self, driver_id: &str) -> Option<Arc<dyn KeyboardDriver>> {
        self.drivers.get(driver_id).cloned()
    }

    pub fn layouts(&self) -> Vec<Layout> {
        self.layouts.values().cloned().collect()
    }

    pub fn layout(&self, layout_id: &str) -> Option<&Layout> {
        self.layouts.get(layout_id)
    }

    pub fn layouts_for_driver(&self, driver_id: &str) -> Vec<&Layout> {
        self.layout_owners
            .iter()
            .filter(|(_, owner)| owner.as_str() == driver_id)
            .filter_map(|(layout_id, _)| self.layouts.get(layout_id))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.drivers.is_empty()
    }

    pub fn discover(&self, context: &OperationContext) -> Result<DiscoveryReport, DriverError> {
        context.check()?;
        let mut report = DiscoveryReport::default();
        let mut seen = HashSet::new();

        for driver in self.drivers.values() {
            context.check()?;
            let manifest = driver.manifest();

            match driver.discover(context) {
                Ok(candidates) => {
                    for candidate in candidates {
                        let key = (candidate.driver_id.clone(), candidate.instance_id.clone());
                        if seen.insert(key) {
                            report.candidates.push(candidate);
                        }
                    }
                }
                Err(DriverError::UnsupportedPlatform) => {
                    report.failures.push(DriverProbeFailure {
                        driver_id: manifest.driver_id,
                        class: ErrorClass::Unsupported,
                        message: "platform is not supported by this driver".into(),
                    });
                }
                Err(error) => {
                    report.failures.push(DriverProbeFailure {
                        driver_id: manifest.driver_id,
                        class: error.class(),
                        message: error.to_string(),
                    });
                }
            }
        }

        report.candidates.sort_by(|a, b| {
            a.driver_id
                .cmp(&b.driver_id)
                .then_with(|| a.instance_id.cmp(&b.instance_id))
        });

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::{
        Capability, Connection, DeviceDescriptor, DeviceMatch, DeviceSession, TransportKind,
        TransportRequirement,
    };
    use mechkeymesh_layout::{
        CoordinateSpace, KeyAction, LayoutKind, PhysicalKey, SUPPORTED_LAYOUT_SCHEMA_VERSION,
    };

    struct FakeDriver {
        id: &'static str,
        device_model_id: &'static str,
        candidate_count: usize,
        layouts: Vec<Layout>,
        declared_layout_ids: Option<Vec<String>>,
    }

    struct FakeSession;

    impl DeviceSession for FakeSession {
        fn descriptor(&self) -> DeviceDescriptor {
            DeviceDescriptor {
                driver_id: "fake".into(),
                device_model_id: "fake.keyboard".into(),
                instance_id: "fake-0".into(),
                vendor: "Fake".into(),
                name: "Fake Keyboard".into(),
                connection: Connection::Usb,
                vendor_id: Some(1),
                product_id: Some(2),
                capabilities: vec![],
                firmware: None,
                battery_percent: None,
            }
        }
    }

    impl KeyboardDriver for FakeDriver {
        fn manifest(&self) -> DriverManifest {
            DriverManifest {
                driver_id: self.id.into(),
                device_model_id: self.device_model_id.into(),
                vendor: "Fake".into(),
                device_name: "Fake Keyboard".into(),
                supported_connections: vec![Connection::Usb],
                capabilities: vec![Capability::HardwareRgb],
                layout_ids: self.declared_layout_ids.clone().unwrap_or_else(|| {
                    self.layouts
                        .iter()
                        .map(|layout| layout.id.clone())
                        .collect()
                }),
                matchers: vec![DeviceMatch {
                    vendor_id: 1,
                    product_id: 2,
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

        fn layouts(&self) -> Vec<Layout> {
            self.layouts.clone()
        }

        fn discover(
            &self,
            context: &OperationContext,
        ) -> Result<Vec<DeviceCandidate>, DriverError> {
            context.check()?;
            Ok((0..self.candidate_count)
                .map(|index| DeviceCandidate {
                    driver_id: self.id.into(),
                    instance_id: format!("fake-{index}"),
                    name: "Fake Keyboard".into(),
                    connection: Connection::Usb,
                    vendor_id: Some(1),
                    product_id: Some(2),
                    usage_page: Some(0xFF00),
                    interface_number: Some(0),
                    serial_number: None,
                })
                .collect())
        }

        fn open(
            &self,
            _context: &OperationContext,
            _candidate: &DeviceCandidate,
        ) -> Result<Box<dyn DeviceSession>, DriverError> {
            Ok(Box::new(FakeSession))
        }
    }

    fn fake_layout(id: &str, device_model_id: &str, index: u16) -> Layout {
        Layout {
            schema_version: SUPPORTED_LAYOUT_SCHEMA_VERSION,
            id: id.into(),
            device_model_id: device_model_id.into(),
            name: id.into(),
            kind: LayoutKind::Custom,
            coordinate_space: CoordinateSpace {
                width: 10.0,
                height: 5.0,
            },
            keys: vec![PhysicalKey {
                id: format!("key-{index}"),
                label: "A".into(),
                source_label: None,
                section: None,
                x: 1.0,
                y: 1.0,
                width: 1.0,
                height: 1.0,
                rotation: 0.0,
                rotation_origin: None,
                matrix: None,
                grid_position: None,
                hid_usage: Some(4),
                device_key_index: Some(index),
                led_index: Some(index),
                fn_programmable: true,
                default_action: KeyAction::Default,
            }],
            source_notes: vec![],
        }
    }

    fn fake_driver(id: &'static str) -> FakeDriver {
        FakeDriver {
            id,
            device_model_id: "fake.keyboard",
            candidate_count: 0,
            layouts: vec![],
            declared_layout_ids: None,
        }
    }

    #[test]
    fn rejects_duplicate_driver_ids() {
        let mut registry = DriverRegistry::new();
        registry.register(Arc::new(fake_driver("fake.driver"))).unwrap();

        let error = registry
            .register(Arc::new(fake_driver("fake.driver")))
            .unwrap_err();

        assert!(matches!(error, RegistryError::DuplicateDriverId(_)));
    }

    #[test]
    fn rejects_layout_manifest_mismatch() {
        let mut driver = fake_driver("bad.layout.driver");
        driver.declared_layout_ids = Some(vec!["missing.layout".into()]);

        let mut registry = DriverRegistry::new();
        let error = registry.register(Arc::new(driver)).unwrap_err();
        assert!(matches!(error, RegistryError::LayoutDeclarationMismatch(_)));
    }

    #[test]
    fn rejects_layout_for_wrong_device_model() {
        let mut driver = fake_driver("wrong.model.driver");
        driver.layouts = vec![fake_layout("wrong.layout", "other.keyboard", 1)];

        let mut registry = DriverRegistry::new();
        let error = registry.register(Arc::new(driver)).unwrap_err();
        assert!(matches!(
            error,
            RegistryError::LayoutDeviceModelMismatch { .. }
        ));
    }

    #[test]
    fn rejects_duplicate_layout_ids_across_drivers() {
        let mut first = fake_driver("first.driver");
        first.layouts = vec![fake_layout("shared.layout", "fake.keyboard", 1)];
        let mut second = fake_driver("second.driver");
        second.layouts = vec![fake_layout("shared.layout", "fake.keyboard", 2)];

        let mut registry = DriverRegistry::new();
        registry.register(Arc::new(first)).unwrap();
        let error = registry.register(Arc::new(second)).unwrap_err();
        assert!(matches!(error, RegistryError::DuplicateLayoutId { .. }));
    }

    #[test]
    fn caches_validated_layouts_once_registered() {
        let mut driver = fake_driver("layout.driver");
        driver.layouts = vec![fake_layout("layout.one", "fake.keyboard", 1)];

        let mut registry = DriverRegistry::new();
        registry.register(Arc::new(driver)).unwrap();

        assert_eq!(registry.layouts().len(), 1);
        assert_eq!(registry.layouts_for_driver("layout.driver").len(), 1);
        assert_eq!(registry.layout("layout.one").unwrap().id, "layout.one");
    }

    #[test]
    fn discovers_registered_driver_candidates() {
        let mut driver = fake_driver("fake.driver");
        driver.candidate_count = 2;

        let mut registry = DriverRegistry::new();
        registry.register(Arc::new(driver)).unwrap();

        let report = registry
            .discover(&OperationContext::new(std::time::Duration::from_secs(1)))
            .unwrap();

        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].driver_id, "fake.driver");
    }
}
