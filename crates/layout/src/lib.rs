use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutKind {
    FullSize,
    Compact1800,
    Tkl,
    Percent75,
    Percent65,
    Percent60,
    Percent40,
    Alice,
    Split,
    Ortholinear,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinateSpace {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixPosition {
    pub row: u16,
    pub column: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPosition {
    pub row: u16,
    pub column: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Modifier {
    LeftCtrl,
    LeftShift,
    LeftAlt,
    LeftMeta,
    RightCtrl,
    RightShift,
    RightAlt,
    RightMeta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaAction {
    PlayPause,
    Stop,
    Previous,
    Next,
    VolumeUp,
    VolumeDown,
    Mute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MouseAction {
    LeftClick,
    MiddleClick,
    RightClick,
    DoubleClick,
    ScrollUp,
    ScrollDown,
    Button4,
    Button5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowsAction {
    ShowDesktop,
    Computer,
    Lock,
    CloseWindow,
    SwitchWindow,
    LogOff,
    Sleep,
    Shutdown,
    Restart,
    Calculator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerAction {
    Momentary,
    Toggle,
    SetDefault,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum KeyAction {
    Default,
    Disabled,
    KeyboardKey {
        usage: u16,
    },
    ModifiedKey {
        modifiers: Vec<Modifier>,
        key: u16,
    },
    Media {
        action: MediaAction,
    },
    Mouse {
        action: MouseAction,
    },
    Windows {
        action: WindowsAction,
    },
    OpenApplication {
        path: String,
    },
    OpenFile {
        path: String,
    },
    OpenUrl {
        url: String,
    },
    SendText {
        text: String,
    },
    Macro {
        macro_id: String,
    },
    Layer {
        action: LayerAction,
        layer_id: String,
    },
    VendorSpecific {
        namespace: String,
        action: String,
        data: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalKey {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub source_label: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub matrix: Option<MatrixPosition>,
    #[serde(default)]
    pub grid_position: Option<GridPosition>,
    #[serde(default)]
    pub hid_usage: Option<u16>,
    #[serde(default)]
    pub device_key_index: Option<u16>,
    #[serde(default)]
    pub led_index: Option<u16>,
    #[serde(default = "default_true")]
    pub fn_programmable: bool,
    pub default_action: KeyAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub schema_version: u16,
    pub id: String,
    pub device_model_id: String,
    pub name: String,
    pub kind: LayoutKind,
    pub coordinate_space: CoordinateSpace,
    pub keys: Vec<PhysicalKey>,
    #[serde(default)]
    pub source_notes: Vec<String>,
}

#[derive(Debug, Error, PartialEq)]
pub enum LayoutError {
    #[error("layout id is empty")]
    EmptyId,
    #[error("device model id is empty")]
    EmptyDeviceModelId,
    #[error("layout coordinate space must be positive")]
    InvalidCoordinateSpace,
    #[error("layout contains no keys")]
    EmptyKeys,
    #[error("duplicate key id: {0}")]
    DuplicateKeyId(String),
    #[error("duplicate device key index: {0}")]
    DuplicateDeviceKeyIndex(u16),
    #[error("duplicate LED index: {0}")]
    DuplicateLedIndex(u16),
    #[error("key {0} has invalid geometry")]
    InvalidKeyGeometry(String),
}

impl Layout {
    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.id.trim().is_empty() {
            return Err(LayoutError::EmptyId);
        }
        if self.device_model_id.trim().is_empty() {
            return Err(LayoutError::EmptyDeviceModelId);
        }
        if self.coordinate_space.width <= 0.0 || self.coordinate_space.height <= 0.0 {
            return Err(LayoutError::InvalidCoordinateSpace);
        }
        if self.keys.is_empty() {
            return Err(LayoutError::EmptyKeys);
        }

        let mut ids = HashSet::new();
        let mut device_indices = HashSet::new();
        let mut led_indices = HashSet::new();

        for key in &self.keys {
            if !ids.insert(key.id.clone()) {
                return Err(LayoutError::DuplicateKeyId(key.id.clone()));
            }
            if key.x < 0.0
                || key.y < 0.0
                || key.width <= 0.0
                || key.height <= 0.0
                || !key.rotation.is_finite()
            {
                return Err(LayoutError::InvalidKeyGeometry(key.id.clone()));
            }
            if let Some(index) = key.device_key_index
                && !device_indices.insert(index)
            {
                return Err(LayoutError::DuplicateDeviceKeyIndex(index));
            }
            if let Some(index) = key.led_index
                && !led_indices.insert(index)
            {
                return Err(LayoutError::DuplicateLedIndex(index));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum LayoutRegistryError {
    #[error(transparent)]
    InvalidLayout(#[from] LayoutError),
    #[error("layout already registered: {0}")]
    DuplicateLayoutId(String),
}

#[derive(Default)]
pub struct LayoutRegistry {
    layouts: BTreeMap<String, Layout>,
}

impl LayoutRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, layout: Layout) -> Result<(), LayoutRegistryError> {
        layout.validate()?;
        if self.layouts.contains_key(&layout.id) {
            return Err(LayoutRegistryError::DuplicateLayoutId(layout.id));
        }
        self.layouts.insert(layout.id.clone(), layout);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Layout> {
        self.layouts.get(id)
    }

    pub fn list(&self) -> Vec<Layout> {
        self.layouts.values().cloned().collect()
    }
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str, device_index: u16) -> PhysicalKey {
        PhysicalKey {
            id: id.into(),
            label: id.into(),
            source_label: None,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            rotation: 0.0,
            matrix: None,
            grid_position: None,
            hid_usage: Some(4),
            device_key_index: Some(device_index),
            led_index: Some(device_index),
            fn_programmable: true,
            default_action: KeyAction::KeyboardKey { usage: 4 },
        }
    }

    #[test]
    fn validates_a_minimal_layout() {
        let layout = Layout {
            schema_version: 1,
            id: "test.layout".into(),
            device_model_id: "test.device".into(),
            name: "Test".into(),
            kind: LayoutKind::Custom,
            coordinate_space: CoordinateSpace {
                width: 10.0,
                height: 4.0,
            },
            keys: vec![key("a", 1)],
            source_notes: vec![],
        };

        assert!(layout.validate().is_ok());
    }

    #[test]
    fn rejects_duplicate_device_indices() {
        let layout = Layout {
            schema_version: 1,
            id: "test.layout".into(),
            device_model_id: "test.device".into(),
            name: "Test".into(),
            kind: LayoutKind::Custom,
            coordinate_space: CoordinateSpace {
                width: 10.0,
                height: 4.0,
            },
            keys: vec![key("a", 1), key("b", 1)],
            source_notes: vec![],
        };

        assert_eq!(
            layout.validate(),
            Err(LayoutError::DuplicateDeviceKeyIndex(1))
        );
    }

    #[test]
    fn registry_rejects_duplicate_layout_ids() {
        let layout = Layout {
            schema_version: 1,
            id: "test.layout".into(),
            device_model_id: "test.device".into(),
            name: "Test".into(),
            kind: LayoutKind::Custom,
            coordinate_space: CoordinateSpace {
                width: 10.0,
                height: 4.0,
            },
            keys: vec![key("a", 1)],
            source_notes: vec![],
        };

        let mut registry = LayoutRegistry::new();
        registry.register(layout.clone()).unwrap();
        assert_eq!(
            registry.register(layout),
            Err(LayoutRegistryError::DuplicateLayoutId("test.layout".into()))
        );
    }

    #[test]
    fn key_actions_round_trip_through_json() {
        let action = KeyAction::ModifiedKey {
            modifiers: vec![Modifier::LeftCtrl, Modifier::LeftShift],
            key: 6,
        };
        let json = serde_json::to_string(&action).unwrap();
        let decoded: KeyAction = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, action);
    }
}
