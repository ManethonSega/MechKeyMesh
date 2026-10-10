use mechkeymesh_driver_api::{Capability, LightingConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceModel {
    pub id: String,
    pub vendor: String,
    pub name: String,
}

impl DeviceModel {
    pub fn new(
        id: impl Into<String>,
        vendor: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            vendor: vendor.into(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub device_model_id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationScope {
    pub device_model_id: String,
    pub profile_id: String,
    pub capability: Capability,
}

impl ConfigurationScope {
    pub fn new(
        device_model_id: impl Into<String>,
        profile_id: impl Into<String>,
        capability: Capability,
    ) -> Self {
        Self {
            device_model_id: device_model_id.into(),
            profile_id: profile_id.into(),
            capability,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesiredLighting {
    pub scope: ConfigurationScope,
    pub config: LightingConfig,
}

impl DesiredLighting {
    pub fn new(scope: ConfigurationScope, config: LightingConfig) -> Self {
        Self { scope, config }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_keeps_device_profile_and_capability_together() {
        let scope = ConfigurationScope::new(
            "aula.f108pro",
            "default",
            Capability::HardwareRgb,
        );

        assert_eq!(scope.device_model_id, "aula.f108pro");
        assert_eq!(scope.profile_id, "default");
        assert_eq!(scope.capability.storage_key(), "hardware-rgb");
    }
}
