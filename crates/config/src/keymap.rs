use super::ConfigurationScope;
use mechkeymesh_driver_api::{KeyAction, KeymapLayer};
use serde::{Deserialize, Serialize};

pub const NORMAL_LAYER_ID: &str = "normal";
pub const FN_LAYER_ID: &str = "fn";

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
    pub fn for_layer(
        scope: ConfigurationScope,
        layer: KeymapLayer,
        mut mappings: Vec<KeyMapping>,
    ) -> Self {
        mappings.sort_by(|left, right| left.key_id.cmp(&right.key_id));
        Self {
            scope,
            layer,
            layer_id: layer_id(layer).into(),
            mappings,
        }
    }

    pub fn normal(scope: ConfigurationScope, mappings: Vec<KeyMapping>) -> Self {
        Self::for_layer(scope, KeymapLayer::Normal, mappings)
    }

    pub fn fn_layer(scope: ConfigurationScope, mappings: Vec<KeyMapping>) -> Self {
        Self::for_layer(scope, KeymapLayer::Fn, mappings)
    }
}

pub const fn layer_id(layer: KeymapLayer) -> &'static str {
    match layer {
        KeymapLayer::Normal => NORMAL_LAYER_ID,
        KeymapLayer::Fn => FN_LAYER_ID,
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
    fn fn_keymap_uses_independent_scope_and_layer_id() {
        let desired = DesiredKeymap::fn_layer(
            ConfigurationScope::new("aula.f108pro", "default", Capability::FnLayer),
            vec![KeyMapping::new("key-055", KeyAction::Disabled)],
        );

        assert_eq!(desired.layer, KeymapLayer::Fn);
        assert_eq!(desired.layer_id, FN_LAYER_ID);
        assert_eq!(desired.scope.capability, Capability::FnLayer);
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
