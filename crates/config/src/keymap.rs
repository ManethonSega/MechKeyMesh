use super::ConfigurationScope;
use mechkeymesh_driver_api::{KeyAction, KeymapLayer};
use serde::{Deserialize, Serialize};

pub const NORMAL_LAYER_ID: &str = "normal";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyMapping {
    pub key_id: String,
    pub action: KeyAction,
}

impl KeyMapping {
    pub fn new(key_id: impl Into<String>, action: KeyAction) -> Self {
        Self {
            key_id: key_id.into(),
            action,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesiredKeymap {
    pub scope: ConfigurationScope,
    pub layer: KeymapLayer,
    pub layer_id: String,
    pub mappings: Vec<KeyMapping>,
}

impl DesiredKeymap {
    pub fn normal(scope: ConfigurationScope, mut mappings: Vec<KeyMapping>) -> Self {
        mappings.sort_by(|left, right| left.key_id.cmp(&right.key_id));
        Self {
            scope,
            layer: KeymapLayer::Normal,
            layer_id: NORMAL_LAYER_ID.into(),
            mappings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::{Capability, Modifier};

    #[test]
    fn normal_keymap_keeps_scope_layer_and_action() {
        let desired = DesiredKeymap::normal(
            ConfigurationScope::new("aula.f108pro", "default", Capability::Keys),
            vec![KeyMapping::new(
                "key-055",
                KeyAction::ModifiedKey {
                    modifiers: vec![Modifier::LeftCtrl],
                    key: 0x06,
                },
            )],
        );

        assert_eq!(desired.layer, KeymapLayer::Normal);
        assert_eq!(desired.layer_id, NORMAL_LAYER_ID);
        assert_eq!(desired.scope.capability, Capability::Keys);
        assert_eq!(desired.mappings.len(), 1);
    }

    #[test]
    fn normal_keymap_is_canonicalized_by_key_id() {
        let desired = DesiredKeymap::normal(
            ConfigurationScope::new("aula.f108pro", "default", Capability::Keys),
            vec![
                KeyMapping::new("key-055", KeyAction::Disabled),
                KeyMapping::new("key-002", KeyAction::Default),
            ],
        );

        assert_eq!(desired.mappings[0].key_id, "key-002");
        assert_eq!(desired.mappings[1].key_id, "key-055");
    }
}
