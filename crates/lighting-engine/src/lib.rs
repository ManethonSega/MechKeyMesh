use mechkeymesh_driver_api::{LiveRgbFrame, PerKeyRgbAssignment, RgbColor};
use mechkeymesh_layout::{Layout, PhysicalKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerKind {
    Static,
    Animation,
    Reactive,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode {
    Replace,
    Add,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiveEffect {
    Static,
    KeyHighlight,
    Ripple,
    Meteor,
    Lightning,
    Wave,
    Trail,
    Breathing,
    Gradient,
    Rain,
    Sparkle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingLayer {
    pub id: String,
    pub name: String,
    pub kind: LayerKind,
    pub effect: LiveEffect,
    pub enabled: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    #[serde(default)]
    pub mask: Vec<String>,
    pub order: i32,
    pub primary: RgbColor,
    pub secondary: RgbColor,
    pub speed: f32,
    pub width: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRgbScene {
    pub name: String,
    pub frames_per_second: u16,
    pub brightness: f32,
    pub layers: Vec<LightingLayer>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReactivePulse {
    pub key_id: String,
    pub age_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AudioBands {
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub amplitude: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderInput {
    pub elapsed_seconds: f32,
    pub reactive_pulses: Vec<ReactivePulse>,
    pub audio: AudioBands,
}

#[derive(Debug, Error, PartialEq)]
pub enum SceneError {
    #[error("live RGB scene name must not be empty")]
    EmptyName,
    #[error("live RGB frame rate must be between 1 and 60 FPS")]
    InvalidFrameRate,
    #[error("live RGB brightness must be finite and between 0 and 1")]
    InvalidBrightness,
    #[error("layer {0} has an empty id or name")]
    EmptyLayerIdentity(usize),
    #[error("duplicate live RGB layer id: {0}")]
    DuplicateLayerId(String),
    #[error("layer {0} has invalid opacity, speed, or width")]
    InvalidLayerParameter(String),
    #[error("layer {layer_id} references unknown key {key_id}")]
    UnknownMaskKey { layer_id: String, key_id: String },
    #[error("layout key {0} has no LED index")]
    MissingLedIndex(String),
}

impl LiveRgbScene {
    pub fn validate(&self, layout: &Layout) -> Result<(), SceneError> {
        if self.name.trim().is_empty() {
            return Err(SceneError::EmptyName);
        }
        if !(1..=60).contains(&self.frames_per_second) {
            return Err(SceneError::InvalidFrameRate);
        }
        if !self.brightness.is_finite() || !(0.0..=1.0).contains(&self.brightness) {
            return Err(SceneError::InvalidBrightness);
        }

        let key_ids: HashSet<&str> = layout.keys.iter().map(|key| key.id.as_str()).collect();
        let mut layer_ids = HashSet::new();
        for (index, layer) in self.layers.iter().enumerate() {
            if layer.id.trim().is_empty() || layer.name.trim().is_empty() {
                return Err(SceneError::EmptyLayerIdentity(index));
            }
            if !layer_ids.insert(layer.id.clone()) {
                return Err(SceneError::DuplicateLayerId(layer.id.clone()));
            }
            if !layer.opacity.is_finite()
                || !(0.0..=1.0).contains(&layer.opacity)
                || !layer.speed.is_finite()
                || layer.speed <= 0.0
                || !layer.width.is_finite()
                || layer.width <= 0.0
            {
                return Err(SceneError::InvalidLayerParameter(layer.id.clone()));
            }
            for key_id in &layer.mask {
                if !key_ids.contains(key_id.as_str()) {
                    return Err(SceneError::UnknownMaskKey {
                        layer_id: layer.id.clone(),
                        key_id: key_id.clone(),
                    });
                }
            }
        }

        for key in &layout.keys {
            if key.led_index.is_none() {
                return Err(SceneError::MissingLedIndex(key.id.clone()));
            }
        }
        Ok(())
    }
}

pub fn default_scene() -> LiveRgbScene {
    LiveRgbScene {
        name: "Live gradient".into(),
        frames_per_second: 30,
        brightness: 0.8,
        layers: vec![LightingLayer {
            id: "base-gradient".into(),
            name: "Gradient".into(),
            kind: LayerKind::Animation,
            effect: LiveEffect::Gradient,
            enabled: true,
            opacity: 1.0,
            blend_mode: BlendMode::Replace,
            mask: Vec::new(),
            order: 0,
            primary: RgbColor {
                r: 0,
                g: 155,
                b: 222,
            },
            secondary: RgbColor {
                r: 139,
                g: 92,
                b: 246,
            },
            speed: 0.2,
            width: 1.0,
        }],
    }
}

pub fn render(
    scene: &LiveRgbScene,
    layout: &Layout,
    input: &RenderInput,
) -> Result<LiveRgbFrame, SceneError> {
    scene.validate(layout)?;
    let mut layers: Vec<&LightingLayer> =
        scene.layers.iter().filter(|layer| layer.enabled).collect();
    layers.sort_by_key(|layer| layer.order);

    let key_centers: HashMap<&str, (f32, f32)> = layout
        .keys
        .iter()
        .map(|key| (key.id.as_str(), center(key)))
        .collect();
    let mut colors = Vec::with_capacity(layout.keys.len());
    for key in &layout.keys {
        let mut output = RgbColor::BLACK;
        for layer in &layers {
            if !layer.mask.is_empty() && !layer.mask.iter().any(|id| id == &key.id) {
                continue;
            }
            let source = effect_color(layer, key, layout, &key_centers, input);
            output = blend_mode(output, source, layer.opacity, layer.blend_mode);
        }
        colors.push(PerKeyRgbAssignment {
            led_index: key.led_index.expect("scene validation checks LED indices"),
            color: scale(output, scene.brightness),
        });
    }
    colors.sort_by_key(|assignment| assignment.led_index);
    Ok(LiveRgbFrame { colors })
}

fn effect_color(
    layer: &LightingLayer,
    key: &PhysicalKey,
    layout: &Layout,
    centers: &HashMap<&str, (f32, f32)>,
    input: &RenderInput,
) -> RgbColor {
    let (x, y) = center(key);
    let nx = x / layout.coordinate_space.width;
    let ny = y / layout.coordinate_space.height;
    let t = input.elapsed_seconds * layer.speed;

    match layer.effect {
        LiveEffect::Static => layer.primary,
        LiveEffect::KeyHighlight => reactive_color(layer, key, input, 0.55, false, centers),
        LiveEffect::Ripple => ripple_color(layer, x, y, layout, input, centers),
        LiveEffect::Meteor => {
            let head = (t * 1.4).fract() * 1.6 - 0.3;
            let distance = (nx * 0.85 + ny * 0.15 - head).abs();
            scale(layer.primary, falloff(distance, layer.width * 0.18))
        }
        LiveEffect::Lightning => {
            let phase = (t * 5.0).floor() as u64;
            let flash =
                hash01(u64::from(key.led_index.unwrap_or_default()) ^ phase.wrapping_mul(7919));
            let active = if flash > 0.9 { 1.0 } else { 0.04 };
            scale(layer.primary, active)
        }
        LiveEffect::Wave => {
            let amount = 0.5 + 0.5 * ((nx * layer.width * 8.0 - t * 6.0).sin());
            mix(layer.primary, layer.secondary, amount)
        }
        LiveEffect::Trail => reactive_color(layer, key, input, 1.4, true, centers),
        LiveEffect::Breathing => {
            let amount = 0.15 + 0.85 * (0.5 + 0.5 * (t * std::f32::consts::TAU).sin());
            scale(layer.primary, amount)
        }
        LiveEffect::Gradient => {
            let amount = (nx + t).fract();
            mix(layer.primary, layer.secondary, amount)
        }
        LiveEffect::Rain => {
            let column = (nx * 24.0).floor() as u64;
            let drop = (t * 0.7 + hash01(column.wrapping_mul(104_729))).fract();
            let distance = (ny - drop).abs();
            scale(layer.primary, falloff(distance, layer.width * 0.12))
        }
        LiveEffect::Sparkle => {
            let tick = (t * 12.0).floor() as u64;
            let seed = u64::from(key.led_index.unwrap_or_default()) * 65_537 ^ tick;
            let amount = hash01(seed).powf(9.0);
            mix(layer.secondary, layer.primary, amount)
        }
    }
}

fn reactive_color(
    layer: &LightingLayer,
    key: &PhysicalKey,
    input: &RenderInput,
    lifetime: f32,
    include_neighbors: bool,
    centers: &HashMap<&str, (f32, f32)>,
) -> RgbColor {
    let key_center = center(key);
    let mut strength: f32 = 0.0;
    for pulse in &input.reactive_pulses {
        if pulse.age_seconds < 0.0 || pulse.age_seconds > lifetime {
            continue;
        }
        let spatial = if pulse.key_id == key.id {
            1.0
        } else if include_neighbors {
            centers.get(pulse.key_id.as_str()).map_or(0.0, |source| {
                let distance = normalized_distance(key_center, *source, 800.0, 300.0);
                falloff(distance, layer.width * 0.2)
            })
        } else {
            0.0
        };
        strength = strength.max(spatial * (1.0 - pulse.age_seconds / lifetime));
    }
    scale(layer.primary, strength)
}

fn ripple_color(
    layer: &LightingLayer,
    x: f32,
    y: f32,
    layout: &Layout,
    input: &RenderInput,
    centers: &HashMap<&str, (f32, f32)>,
) -> RgbColor {
    let mut strength: f32 = 0.0;
    for pulse in &input.reactive_pulses {
        if !(0.0..=1.5).contains(&pulse.age_seconds) {
            continue;
        }
        let Some(source) = centers.get(pulse.key_id.as_str()) else {
            continue;
        };
        let distance = normalized_distance(
            (x, y),
            *source,
            layout.coordinate_space.width,
            layout.coordinate_space.height,
        );
        let ring = pulse.age_seconds * layer.speed * 0.8;
        strength = strength.max(falloff((distance - ring).abs(), layer.width * 0.08));
    }
    scale(layer.primary, strength)
}

fn center(key: &PhysicalKey) -> (f32, f32) {
    (key.x + key.width / 2.0, key.y + key.height / 2.0)
}

fn normalized_distance(a: (f32, f32), b: (f32, f32), width: f32, height: f32) -> f32 {
    let dx = (a.0 - b.0) / width.max(1.0);
    let dy = (a.1 - b.1) / height.max(1.0);
    (dx * dx + dy * dy).sqrt()
}

fn falloff(distance: f32, width: f32) -> f32 {
    (1.0 - distance / width.max(0.001)).clamp(0.0, 1.0)
}

fn blend_mode(base: RgbColor, source: RgbColor, opacity: f32, mode: BlendMode) -> RgbColor {
    if mode == BlendMode::Replace {
        return mix(base, source, opacity);
    }
    let target = match mode {
        BlendMode::Replace => source,
        BlendMode::Add => RgbColor {
            r: base.r.saturating_add(source.r),
            g: base.g.saturating_add(source.g),
            b: base.b.saturating_add(source.b),
        },
        BlendMode::Multiply => RgbColor {
            r: ((u16::from(base.r) * u16::from(source.r)) / 255) as u8,
            g: ((u16::from(base.g) * u16::from(source.g)) / 255) as u8,
            b: ((u16::from(base.b) * u16::from(source.b)) / 255) as u8,
        },
        BlendMode::Screen => RgbColor {
            r: 255 - (((255 - u16::from(base.r)) * (255 - u16::from(source.r))) / 255) as u8,
            g: 255 - (((255 - u16::from(base.g)) * (255 - u16::from(source.g))) / 255) as u8,
            b: 255 - (((255 - u16::from(base.b)) * (255 - u16::from(source.b))) / 255) as u8,
        },
    };
    mix(base, target, opacity)
}

fn mix(a: RgbColor, b: RgbColor, amount: f32) -> RgbColor {
    let amount = amount.clamp(0.0, 1.0);
    RgbColor {
        r: (f32::from(a.r) + (f32::from(b.r) - f32::from(a.r)) * amount).round() as u8,
        g: (f32::from(a.g) + (f32::from(b.g) - f32::from(a.g)) * amount).round() as u8,
        b: (f32::from(a.b) + (f32::from(b.b) - f32::from(a.b)) * amount).round() as u8,
    }
}

fn scale(color: RgbColor, amount: f32) -> RgbColor {
    mix(RgbColor::BLACK, color, amount)
}

fn hash01(mut value: u64) -> f32 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51afd7ed558ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ceb9fe1a85ec53);
    value ^= value >> 33;
    (value as u32) as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_layout::{
        CoordinateSpace, KeyAction, LayoutKind, PhysicalKey, SUPPORTED_LAYOUT_SCHEMA_VERSION,
    };

    fn layout() -> Layout {
        Layout {
            schema_version: SUPPORTED_LAYOUT_SCHEMA_VERSION,
            id: "test.layout".into(),
            device_model_id: "test.device".into(),
            name: "Test".into(),
            kind: LayoutKind::Custom,
            coordinate_space: CoordinateSpace {
                width: 100.0,
                height: 20.0,
            },
            keys: (0..3)
                .map(|index| PhysicalKey {
                    id: format!("key-{index}"),
                    label: index.to_string(),
                    source_label: None,
                    section: None,
                    x: index as f32 * 30.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                    rotation: 0.0,
                    rotation_origin: None,
                    matrix: None,
                    grid_position: None,
                    hid_usage: None,
                    device_key_index: Some(index + 1),
                    led_index: Some(index + 1),
                    fn_programmable: true,
                    default_action: KeyAction::Default,
                })
                .collect(),
            source_notes: Vec::new(),
        }
    }

    #[test]
    fn validates_layer_ids_parameters_and_masks() {
        let layout = layout();
        let mut scene = default_scene();
        assert!(scene.validate(&layout).is_ok());
        scene.layers[0].mask = vec!["missing".into()];
        assert!(matches!(
            scene.validate(&layout),
            Err(SceneError::UnknownMaskKey { .. })
        ));
    }

    #[test]
    fn rendering_is_sorted_and_deterministic() {
        let layout = layout();
        let scene = default_scene();
        let input = RenderInput {
            elapsed_seconds: 2.5,
            ..RenderInput::default()
        };
        let first = render(&scene, &layout, &input).unwrap();
        let second = render(&scene, &layout, &input).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.colors.len(), 3);
        assert_eq!(first.colors[0].led_index, 1);
    }

    #[test]
    fn mask_limits_a_layer_to_selected_keys() {
        let layout = layout();
        let mut scene = default_scene();
        scene.layers[0].effect = LiveEffect::Static;
        scene.layers[0].primary = RgbColor { r: 255, g: 0, b: 0 };
        scene.layers[0].mask = vec!["key-1".into()];
        let frame = render(&scene, &layout, &RenderInput::default()).unwrap();
        assert_eq!(frame.colors[0].color, RgbColor::BLACK);
        assert!(frame.colors[1].color.r > 0);
        assert_eq!(frame.colors[2].color, RgbColor::BLACK);
    }
}
