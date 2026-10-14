use crate::ConfigurationScope;
use mechkeymesh_driver_api::RgbColor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyColor {
    pub key_id: String,
    pub color: RgbColor,
}

impl KeyColor {
    pub fn new(key_id: impl Into<String>, color: RgbColor) -> Self {
        Self {
            key_id: key_id.into(),
            color,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesiredPerKeyRgb {
    pub scope: ConfigurationScope,
    pub brightness: u8,
    pub colors: Vec<KeyColor>,
}

impl DesiredPerKeyRgb {
    pub fn new(scope: ConfigurationScope, brightness: u8, mut colors: Vec<KeyColor>) -> Self {
        colors.sort_by(|left, right| left.key_id.cmp(&right.key_id));
        Self {
            scope,
            brightness,
            colors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::Capability;

    #[test]
    fn desired_per_key_rgb_is_canonicalized_by_key_id() {
        let desired = DesiredPerKeyRgb::new(
            ConfigurationScope::new("aula.f108pro", "default", Capability::PerKeyRgb),
            5,
            vec![
                KeyColor::new("key-056", RgbColor { r: 1, g: 2, b: 3 }),
                KeyColor::new("key-001", RgbColor { r: 4, g: 5, b: 6 }),
            ],
        );

        assert_eq!(desired.colors[0].key_id, "key-001");
        assert_eq!(desired.colors[1].key_id, "key-056");
    }
}
