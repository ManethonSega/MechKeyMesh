use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

pub const SUPPORTED_LAYOUT_SCHEMA_VERSION: u16 = 1;
const BOUNDS_EPSILON: f32 = 0.01;

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixPosition {
    #[serde(default)]
    pub unit: u16,
    pub row: u16,
    pub column: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPosition {
    #[serde(default)]
    pub unit: u16,
    pub row: u16,
    pub column: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
        /// Resolved hardware-bank slot. Stored mappings leave this unset;
        /// the application resolves it immediately before a device write.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hardware_slot: Option<u8>,
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

#[derive(Debug, Error, PartialEq)]
pub enum ActionError {
    #[error("keyboard usage 0 is reserved; use Default or Disabled instead")]
    ReservedKeyboardUsage,
    #[error("modified-key action has no modifiers")]
    EmptyModifierSet,
    #[error("modified-key action contains duplicate modifier {0:?}")]
    DuplicateModifier(Modifier),
    #[error("{0} must not be empty")]
    EmptyValue(&'static str),
}

impl KeyAction {
    pub fn validate(&self) -> Result<(), ActionError> {
        match self {
            Self::Default
            | Self::Disabled
            | Self::Media { .. }
            | Self::Mouse { .. }
            | Self::Windows { .. } => Ok(()),
            Self::KeyboardKey { usage } => {
                if *usage == 0 {
                    Err(ActionError::ReservedKeyboardUsage)
                } else {
                    Ok(())
                }
            }
            Self::ModifiedKey { modifiers, .. } => {
                if modifiers.is_empty() {
                    return Err(ActionError::EmptyModifierSet);
                }
                let mut unique = HashSet::new();
                for modifier in modifiers {
                    if !unique.insert(*modifier) {
                        return Err(ActionError::DuplicateModifier(*modifier));
                    }
                }
                Ok(())
            }
            Self::OpenApplication { path } => require_text(path, "application path"),
            Self::OpenFile { path } => require_text(path, "file path"),
            Self::OpenUrl { url } => require_text(url, "URL"),
            Self::SendText { text } => require_text(text, "text"),
            Self::Macro { macro_id, .. } => require_text(macro_id, "macro ID"),
            Self::Layer { layer_id, .. } => require_text(layer_id, "layer ID"),
            Self::VendorSpecific {
                namespace, action, ..
            } => {
                require_text(namespace, "vendor namespace")?;
                require_text(action, "vendor action")
            }
        }
    }
}

fn require_text(value: &str, field: &'static str) -> Result<(), ActionError> {
    if value.trim().is_empty() {
        Err(ActionError::EmptyValue(field))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalKey {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub source_label: Option<String>,
    #[serde(default)]
    pub section: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub rotation_origin: Option<Point>,
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

impl PhysicalKey {
    pub fn rotation_pivot(&self) -> Point {
        self.rotation_origin.unwrap_or(Point {
            x: self.x + self.width / 2.0,
            y: self.y + self.height / 2.0,
        })
    }

    fn rotated_bounds(&self) -> (f32, f32, f32, f32) {
        let pivot = self.rotation_pivot();
        let radians = self.rotation.to_radians();
        let sin = radians.sin();
        let cos = radians.cos();
        let corners = [
            (self.x, self.y),
            (self.x + self.width, self.y),
            (self.x, self.y + self.height),
            (self.x + self.width, self.y + self.height),
        ];

        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for (x, y) in corners {
            let dx = x - pivot.x;
            let dy = y - pivot.y;
            let rotated_x = pivot.x + dx * cos - dy * sin;
            let rotated_y = pivot.y + dx * sin + dy * cos;
            min_x = min_x.min(rotated_x);
            min_y = min_y.min(rotated_y);
            max_x = max_x.max(rotated_x);
            max_y = max_y.max(rotated_y);
        }

        (min_x, min_y, max_x, max_y)
    }
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
    #[error("unsupported layout schema version {found}; supported version is {supported}")]
    UnsupportedSchemaVersion { found: u16, supported: u16 },
    #[error("layout id is empty")]
    EmptyId,
    #[error("device model id is empty")]
    EmptyDeviceModelId,
    #[error("layout name is empty")]
    EmptyName,
    #[error("layout coordinate space must be finite and positive")]
    InvalidCoordinateSpace,
    #[error("layout contains no keys")]
    EmptyKeys,
    #[error("key at position {0} has an empty id")]
    EmptyKeyId(usize),
    #[error("key {0} has an empty label")]
    EmptyKeyLabel(String),
    #[error("key {0} has an empty section id")]
    EmptyKeySection(String),
    #[error("duplicate key id: {0}")]
    DuplicateKeyId(String),
    #[error("duplicate device key index: {0}")]
    DuplicateDeviceKeyIndex(u16),
    #[error("duplicate LED index: {0}")]
    DuplicateLedIndex(u16),
    #[error("duplicate matrix position: unit {unit}, row {row}, column {column}")]
    DuplicateMatrixPosition { unit: u16, row: u16, column: u16 },
    #[error("duplicate source-grid position: unit {unit}, row {row}, column {column}")]
    DuplicateGridPosition { unit: u16, row: u16, column: u16 },
    #[error("key {0} has invalid or non-finite geometry")]
    InvalidKeyGeometry(String),
    #[error("key {0} has an invalid rotation origin")]
    InvalidRotationOrigin(String),
    #[error("key {0} extends outside the declared coordinate space")]
    KeyOutsideCoordinateSpace(String),
    #[error("key {key_id} has an invalid default action: {source}")]
    InvalidDefaultAction { key_id: String, source: ActionError },
}

impl Layout {
    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.schema_version != SUPPORTED_LAYOUT_SCHEMA_VERSION {
            return Err(LayoutError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: SUPPORTED_LAYOUT_SCHEMA_VERSION,
            });
        }
        if self.id.trim().is_empty() {
            return Err(LayoutError::EmptyId);
        }
        if self.device_model_id.trim().is_empty() {
            return Err(LayoutError::EmptyDeviceModelId);
        }
        if self.name.trim().is_empty() {
            return Err(LayoutError::EmptyName);
        }
        if !self.coordinate_space.width.is_finite()
            || !self.coordinate_space.height.is_finite()
            || self.coordinate_space.width <= 0.0
            || self.coordinate_space.height <= 0.0
        {
            return Err(LayoutError::InvalidCoordinateSpace);
        }
        if self.keys.is_empty() {
            return Err(LayoutError::EmptyKeys);
        }

        let mut ids = HashSet::new();
        let mut device_indices = HashSet::new();
        let mut led_indices = HashSet::new();
        let mut matrix_positions = HashSet::new();
        let mut grid_positions = HashSet::new();

        for (position, key) in self.keys.iter().enumerate() {
            if key.id.trim().is_empty() {
                return Err(LayoutError::EmptyKeyId(position));
            }
            if key.label.trim().is_empty() {
                return Err(LayoutError::EmptyKeyLabel(key.id.clone()));
            }
            if key
                .section
                .as_ref()
                .is_some_and(|section| section.trim().is_empty())
            {
                return Err(LayoutError::EmptyKeySection(key.id.clone()));
            }
            if !ids.insert(key.id.clone()) {
                return Err(LayoutError::DuplicateKeyId(key.id.clone()));
            }
            if !key.x.is_finite()
                || !key.y.is_finite()
                || !key.width.is_finite()
                || !key.height.is_finite()
                || !key.rotation.is_finite()
                || key.x < 0.0
                || key.y < 0.0
                || key.width <= 0.0
                || key.height <= 0.0
            {
                return Err(LayoutError::InvalidKeyGeometry(key.id.clone()));
            }
            if let Some(origin) = key.rotation_origin
                && (!origin.x.is_finite() || !origin.y.is_finite())
            {
                return Err(LayoutError::InvalidRotationOrigin(key.id.clone()));
            }

            let (min_x, min_y, max_x, max_y) = key.rotated_bounds();
            if min_x < -BOUNDS_EPSILON
                || min_y < -BOUNDS_EPSILON
                || max_x > self.coordinate_space.width + BOUNDS_EPSILON
                || max_y > self.coordinate_space.height + BOUNDS_EPSILON
            {
                return Err(LayoutError::KeyOutsideCoordinateSpace(key.id.clone()));
            }

            key.default_action
                .validate()
                .map_err(|source| LayoutError::InvalidDefaultAction {
                    key_id: key.id.clone(),
                    source,
                })?;

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
            if let Some(matrix) = key.matrix
                && !matrix_positions.insert(matrix)
            {
                return Err(LayoutError::DuplicateMatrixPosition {
                    unit: matrix.unit,
                    row: matrix.row,
                    column: matrix.column,
                });
            }
            if let Some(grid) = key.grid_position
                && !grid_positions.insert(grid)
            {
                return Err(LayoutError::DuplicateGridPosition {
                    unit: grid.unit,
                    row: grid.row,
                    column: grid.column,
                });
            }
        }

        Ok(())
    }

    pub fn key(&self, id: &str) -> Option<&PhysicalKey> {
        self.keys.iter().find(|key| key.id == id)
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

    pub fn for_device_model(&self, device_model_id: &str) -> Vec<&Layout> {
        self.layouts
            .values()
            .filter(|layout| layout.device_model_id == device_model_id)
            .collect()
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
            device_key_index: Some(device_index),
            led_index: Some(device_index),
            fn_programmable: true,
            default_action: KeyAction::KeyboardKey { usage: 4 },
        }
    }

    fn layout(keys: Vec<PhysicalKey>) -> Layout {
        Layout {
            schema_version: SUPPORTED_LAYOUT_SCHEMA_VERSION,
            id: "test.layout".into(),
            device_model_id: "test.device".into(),
            name: "Test".into(),
            kind: LayoutKind::Custom,
            coordinate_space: CoordinateSpace {
                width: 10.0,
                height: 6.0,
            },
            keys,
            source_notes: vec![],
        }
    }

    #[test]
    fn validates_a_minimal_layout() {
        assert!(layout(vec![key("a", 1)]).validate().is_ok());
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let mut candidate = layout(vec![key("a", 1)]);
        candidate.schema_version = 2;
        assert_eq!(
            candidate.validate(),
            Err(LayoutError::UnsupportedSchemaVersion {
                found: 2,
                supported: SUPPORTED_LAYOUT_SCHEMA_VERSION,
            })
        );
    }

    #[test]
    fn rejects_duplicate_device_indices() {
        assert_eq!(
            layout(vec![key("a", 1), key("b", 1)]).validate(),
            Err(LayoutError::DuplicateDeviceKeyIndex(1))
        );
    }

    #[test]
    fn rejects_duplicate_grid_positions_within_same_unit() {
        let mut first = key("a", 1);
        first.grid_position = Some(GridPosition {
            unit: 0,
            row: 1,
            column: 2,
        });
        let mut second = key("b", 2);
        second.grid_position = first.grid_position;

        assert_eq!(
            layout(vec![first, second]).validate(),
            Err(LayoutError::DuplicateGridPosition {
                unit: 0,
                row: 1,
                column: 2,
            })
        );
    }

    #[test]
    fn separate_matrix_units_can_reuse_row_and_column() {
        let mut left = key("left", 1);
        left.matrix = Some(MatrixPosition {
            unit: 0,
            row: 1,
            column: 2,
        });
        let mut right = key("right", 2);
        right.matrix = Some(MatrixPosition {
            unit: 1,
            row: 1,
            column: 2,
        });

        assert!(layout(vec![left, right]).validate().is_ok());
    }

    #[test]
    fn supports_shared_rotation_origins_for_alice_and_split_layouts() {
        let mut rotated = key("rotated", 1);
        rotated.x = 4.0;
        rotated.y = 2.0;
        rotated.rotation = 15.0;
        rotated.rotation_origin = Some(Point { x: 5.0, y: 3.0 });

        assert_eq!(rotated.rotation_pivot(), Point { x: 5.0, y: 3.0 });
        assert!(layout(vec![rotated]).validate().is_ok());
    }

    #[test]
    fn rejects_keys_outside_coordinate_space_after_rotation() {
        let mut rotated = key("outside", 1);
        rotated.x = 9.4;
        rotated.y = 5.0;
        rotated.width = 1.0;
        rotated.height = 1.0;
        rotated.rotation = 45.0;

        assert_eq!(
            layout(vec![rotated]).validate(),
            Err(LayoutError::KeyOutsideCoordinateSpace("outside".into()))
        );
    }

    #[test]
    fn validates_key_action_payloads() {
        assert_eq!(
            KeyAction::KeyboardKey { usage: 0 }.validate(),
            Err(ActionError::ReservedKeyboardUsage)
        );
        assert_eq!(
            KeyAction::ModifiedKey {
                modifiers: vec![Modifier::LeftCtrl, Modifier::LeftCtrl],
                key: 6,
            }
            .validate(),
            Err(ActionError::DuplicateModifier(Modifier::LeftCtrl))
        );
        assert_eq!(
            KeyAction::OpenUrl { url: " ".into() }.validate(),
            Err(ActionError::EmptyValue("URL"))
        );
    }

    #[test]
    fn registry_rejects_duplicate_layout_ids() {
        let candidate = layout(vec![key("a", 1)]);
        let mut registry = LayoutRegistry::new();
        registry.register(candidate.clone()).unwrap();
        assert_eq!(
            registry.register(candidate),
            Err(LayoutRegistryError::DuplicateLayoutId("test.layout".into()))
        );
    }

    #[test]
    fn registry_filters_by_device_model() {
        let mut second = layout(vec![key("b", 2)]);
        second.id = "other.layout".into();
        second.device_model_id = "other.device".into();

        let mut registry = LayoutRegistry::new();
        registry.register(layout(vec![key("a", 1)])).unwrap();
        registry.register(second).unwrap();

        assert_eq!(registry.for_device_model("test.device").len(), 1);
        assert_eq!(registry.for_device_model("other.device").len(), 1);
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
