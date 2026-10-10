use mechkeymesh_driver_api::{
    DeviceCandidate, DriverError, DriverManifest, ErrorClass, KeyboardDriver, OperationContext,
};
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
    #[error("driver already registered: {0}")]
    DuplicateDriverId(String),
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
        }
    }

    pub fn register(&mut self, driver: Arc<dyn KeyboardDriver>) -> Result<(), RegistryError> {
        let manifest = driver.manifest();
        if manifest.driver_id.trim().is_empty() {
            return Err(RegistryError::EmptyDriverId);
        }
        if self.drivers.contains_key(&manifest.driver_id) {
            return Err(RegistryError::DuplicateDriverId(manifest.driver_id));
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

    pub fn layouts(&self) -> Vec<mechkeymesh_driver_api::Layout> {
        self.drivers
            .values()
            .flat_map(|driver| driver.layouts())
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

    struct FakeDriver {
        id: &'static str,
        candidate_count: usize,
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
                device_model_id: "fake.keyboard".into(),
                vendor: "Fake".into(),
                device_name: "Fake Keyboard".into(),
                supported_connections: vec![Connection::Usb],
                capabilities: vec![Capability::HardwareRgb],
                layout_ids: vec!["fake.layout".into()],
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

    #[test]
    fn rejects_duplicate_driver_ids() {
        let mut registry = DriverRegistry::new();
        registry
            .register(Arc::new(FakeDriver {
                id: "fake.driver",
                candidate_count: 0,
            }))
            .unwrap();

        let error = registry
            .register(Arc::new(FakeDriver {
                id: "fake.driver",
                candidate_count: 0,
            }))
            .unwrap_err();

        assert!(matches!(error, RegistryError::DuplicateDriverId(_)));
    }

    #[test]
    fn discovers_registered_driver_candidates() {
        let mut registry = DriverRegistry::new();
        registry
            .register(Arc::new(FakeDriver {
                id: "fake.driver",
                candidate_count: 2,
            }))
            .unwrap();

        let report = registry
            .discover(&OperationContext::new(std::time::Duration::from_secs(1)))
            .unwrap();

        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].driver_id, "fake.driver");
    }
}
