use mechkeymesh_config::{
    ConfigurationScope, DesiredKeymap, DesiredLighting, DesiredPerKeyRgb, DeviceModel, KeyColor,
    KeyMapping,
};
use mechkeymesh_core::{
    DeviceManager, DeviceManagerHandle, DeviceStatus, DriverRegistry, LogEntry, ManagerConfig,
};
use mechkeymesh_driver_api::{
    Capability, DriverManifest, FnLayerDescriptor, KeyAssignment, KeyboardDriver,
    KeymapCapabilities, KeymapLayer, LightingConfig, LightingEffectDescriptor,
    PerKeyRgbAssignment, PerKeyRgbCapabilities, PerKeyRgbConfig, RgbColor,
};
use mechkeymesh_driver_aula_f108pro::AulaF108ProRemapDriver;
use mechkeymesh_layout::{Layout, LayoutRegistry};
use mechkeymesh_persistence::{PersistenceStatus, PersistenceStore};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use tauri::{Manager, State};

struct AppState {
    manager: DeviceManagerHandle,
    persistence: PersistenceStore,
    layouts: LayoutRegistry,
    driver_layout_ids: BTreeMap<String, String>,
    primary_driver: Arc<dyn KeyboardDriver>,
    lighting_effects: Vec<LightingEffectDescriptor>,
    primary_device_model_id: String,
    primary_layout_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LightingApplyResult {
    stored: bool,
    applied: bool,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerKeyRgbApplyResult {
    stored: bool,
    applied: bool,
    color_count: usize,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerKeyRgbHardwareReadback {
    colors: Vec<KeyColor>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeymapApplyResult {
    stored: bool,
    applied: bool,
    assignment_count: usize,
    message: Option<String>,
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> DeviceStatus {
    state.manager.status()
}

#[tauri::command]
fn get_logs(state: State<'_, AppState>) -> Vec<LogEntry> {
    state.manager.logs()
}

#[tauri::command]
fn get_driver_registry(state: State<'_, AppState>) -> Vec<DriverManifest> {
    state.manager.drivers()
}

#[tauri::command]
fn get_persistence_status(state: State<'_, AppState>) -> Result<PersistenceStatus, String> {
    state
        .persistence
        .status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_desired_lighting(state: State<'_, AppState>) -> Result<Option<DesiredLighting>, String> {
    let scope = configuration_scope(&state, Capability::HardwareRgb)?;
    state
        .persistence
        .load_lighting(&scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_desired_per_key_rgb(state: State<'_, AppState>) -> Result<DesiredPerKeyRgb, String> {
    let scope = configuration_scope(&state, Capability::PerKeyRgb)?;
    state
        .persistence
        .load_per_key_rgb(&scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_per_key_rgb_capabilities(
    state: State<'_, AppState>,
) -> Result<PerKeyRgbCapabilities, String> {
    state
        .primary_driver
        .per_key_rgb_capabilities()
        .ok_or_else(|| "active driver does not support stored per-key RGB".to_string())
}

#[tauri::command]
fn get_desired_keymap(
    state: State<'_, AppState>,
    layer: KeymapLayer,
) -> Result<DesiredKeymap, String> {
    let scope = configuration_scope(&state, capability_for_layer(layer))?;
    state
        .persistence
        .load_keymap(&scope, layer)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_layout(state: State<'_, AppState>, layout_id: String) -> Result<Layout, String> {
    state
        .layouts
        .get(&layout_id)
        .cloned()
        .ok_or_else(|| format!("layout not found: {layout_id}"))
}

#[tauri::command]
fn get_primary_layout(state: State<'_, AppState>) -> Result<Layout, String> {
    let layout_id = active_layout_id(&state)?;
    get_layout(state, layout_id)
}

#[tauri::command]
fn get_keymap_capabilities(
    state: State<'_, AppState>,
    layer: KeymapLayer,
) -> Result<KeymapCapabilities, String> {
    state
        .primary_driver
        .keymap_capabilities(layer)
        .ok_or_else(|| format!("active driver does not support {layer:?} remapping"))
}

#[tauri::command]
fn get_fn_layer_descriptor(state: State<'_, AppState>) -> Result<FnLayerDescriptor, String> {
    state
        .primary_driver
        .fn_layer_descriptor()
        .ok_or_else(|| "active driver does not expose Fn-layer settings".to_string())
}

#[tauri::command]
fn reconnect(state: State<'_, AppState>) {
    state.manager.reconnect();
}

#[tauri::command]
fn cancel_active_operation(state: State<'_, AppState>) -> bool {
    state.manager.cancel_active_operation()
}

#[tauri::command]
fn get_lighting_effects(state: State<'_, AppState>) -> Vec<LightingEffectDescriptor> {
    state.lighting_effects.clone()
}

#[tauri::command]
fn apply_lighting(
    state: State<'_, AppState>,
    config: LightingConfig,
) -> Result<LightingApplyResult, String> {
    let scope = configuration_scope(&state, Capability::HardwareRgb)?;
    let desired = DesiredLighting::new(scope, config);

    state
        .persistence
        .save_lighting(&desired)
        .map_err(|error| error.to_string())?;

    match state.manager.set_lighting(config) {
        Ok(()) => Ok(LightingApplyResult {
            stored: true,
            applied: true,
            message: None,
        }),
        Err(error) => Ok(LightingApplyResult {
            stored: true,
            applied: false,
            message: Some(error),
        }),
    }
}

#[tauri::command]
fn save_per_key_rgb(
    state: State<'_, AppState>,
    brightness: u8,
    colors: Vec<KeyColor>,
) -> Result<PerKeyRgbApplyResult, String> {
    let scope = configuration_scope(&state, Capability::PerKeyRgb)?;
    let desired = DesiredPerKeyRgb::new(scope, brightness, colors);
    let config = resolve_per_key_rgb(&state, &desired)?;

    state
        .primary_driver
        .validate_per_key_rgb(&config)
        .map_err(|error| error.to_string())?;

    state
        .persistence
        .save_per_key_rgb(&desired)
        .map_err(|error| error.to_string())?;

    match state.manager.set_per_key_rgb(config.clone()) {
        Ok(()) => Ok(PerKeyRgbApplyResult {
            stored: true,
            applied: true,
            color_count: config.colors.len(),
            message: None,
        }),
        Err(error) => Ok(PerKeyRgbApplyResult {
            stored: true,
            applied: false,
            color_count: config.colors.len(),
            message: Some(error),
        }),
    }
}

#[tauri::command]
fn read_per_key_rgb(
    state: State<'_, AppState>,
) -> Result<PerKeyRgbHardwareReadback, String> {
    let layout_id = active_layout_id(&state)?;
    let layout = state
        .layouts
        .get(&layout_id)
        .ok_or_else(|| format!("layout not found: {layout_id}"))?;

    let mut led_to_key = BTreeMap::new();
    let mut led_indices = Vec::new();
    for key in &layout.keys {
        if let Some(led_index) = key.led_index {
            if led_to_key.insert(led_index, key.id.clone()).is_some() {
                return Err(format!(
                    "layout {} contains duplicate LED index {}",
                    layout.id, led_index
                ));
            }
            led_indices.push(led_index);
        }
    }

    let readback = state.manager.read_per_key_rgb(led_indices)?;
    let colors = readback
        .colors
        .into_iter()
        .filter_map(|assignment| {
            led_to_key
                .get(&assignment.led_index)
                .map(|key_id| KeyColor::new(key_id.clone(), assignment.color))
        })
        .collect();

    Ok(PerKeyRgbHardwareReadback { colors })
}

#[tauri::command]
fn save_keymap(
    state: State<'_, AppState>,
    layer: KeymapLayer,
    mappings: Vec<KeyMapping>,
) -> Result<KeymapApplyResult, String> {
    let scope = configuration_scope(&state, capability_for_layer(layer))?;
    let desired = DesiredKeymap::for_layer(scope, layer, mappings);
    let assignments = resolve_key_assignments(&state, &desired)?;

    state
        .primary_driver
        .validate_keymap(layer, &assignments)
        .map_err(|error| error.to_string())?;

    state
        .persistence
        .save_keymap(&desired)
        .map_err(|error| error.to_string())?;

    match state.manager.set_keymap(layer, assignments.clone()) {
        Ok(()) => Ok(KeymapApplyResult {
            stored: true,
            applied: true,
            assignment_count: assignments.len(),
            message: None,
        }),
        Err(error) => Ok(KeymapApplyResult {
            stored: true,
            applied: false,
            assignment_count: assignments.len(),
            message: Some(error),
        }),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeymapResetResult {
    stored: bool,
    normal_applied: bool,
    fn_applied: bool,
    message: Option<String>,
}

#[tauri::command]
fn reset_key_assignments(state: State<'_, AppState>) -> Result<KeymapResetResult, String> {
    let normal = DesiredKeymap::for_layer(
        configuration_scope(&state, Capability::Keys)?,
        KeymapLayer::Normal,
        vec![],
    );
    let fn_layer = DesiredKeymap::for_layer(
        configuration_scope(&state, Capability::FnLayer)?,
        KeymapLayer::Fn,
        vec![],
    );

    state
        .primary_driver
        .validate_keymap(KeymapLayer::Normal, &[])
        .map_err(|error| error.to_string())?;
    state
        .primary_driver
        .validate_keymap(KeymapLayer::Fn, &[])
        .map_err(|error| error.to_string())?;

    state
        .persistence
        .save_keymaps(&[normal, fn_layer])
        .map_err(|error| error.to_string())?;

    let normal_result = state.manager.set_keymap(KeymapLayer::Normal, vec![]);
    let fn_result = state.manager.set_keymap(KeymapLayer::Fn, vec![]);
    let normal_applied = normal_result.is_ok();
    let fn_applied = fn_result.is_ok();
    let message = match (normal_result, fn_result) {
        (Ok(()), Ok(())) => None,
        (normal, fn_layer) => Some(format!(
            "desired key assignments were reset locally; normal layer: {}; Fn layer: {}",
            normal.err().unwrap_or_else(|| "applied".to_string()),
            fn_layer.err().unwrap_or_else(|| "applied".to_string())
        )),
    };

    Ok(KeymapResetResult {
        stored: true,
        normal_applied,
        fn_applied,
        message,
    })
}

const fn capability_for_layer(layer: KeymapLayer) -> Capability {
    match layer {
        KeymapLayer::Normal => Capability::Keys,
        KeymapLayer::Fn => Capability::FnLayer,
    }
}

fn active_layout_id(state: &AppState) -> Result<String, String> {
    if let Some(device) = state.manager.status().device {
        return state
            .driver_layout_ids
            .get(&device.driver_id)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "active driver {} has no registered layout",
                    device.driver_id
                )
            });
    }

    Ok(state.primary_layout_id.clone())
}

fn resolve_key_assignments(
    state: &AppState,
    desired: &DesiredKeymap,
) -> Result<Vec<KeyAssignment>, String> {
    let layout_id = active_layout_id(state)?;
    let layout = state
        .layouts
        .get(&layout_id)
        .ok_or_else(|| format!("layout not found: {layout_id}"))?;
    let mut seen = HashSet::new();
    let mut assignments = Vec::with_capacity(desired.mappings.len());

    for mapping in &desired.mappings {
        if !seen.insert(mapping.key_id.as_str()) {
            return Err(format!("duplicate key mapping: {}", mapping.key_id));
        }
        let key = layout
            .key(&mapping.key_id)
            .ok_or_else(|| format!("key does not exist in active layout: {}", mapping.key_id))?;
        if desired.layer == KeymapLayer::Fn
            && !key.fn_programmable
            && mapping.action != mechkeymesh_layout::KeyAction::Default
        {
            return Err(format!(
                "key {} is not programmable on the Fn layer",
                mapping.key_id
            ));
        }
        let device_key_index = key
            .device_key_index
            .ok_or_else(|| format!("key {} has no device key index", mapping.key_id))?;
        assignments.push(KeyAssignment {
            device_key_index,
            action: mapping.action.clone(),
        });
    }
    Ok(assignments)
}

fn resolve_per_key_rgb(
    state: &AppState,
    desired: &DesiredPerKeyRgb,
) -> Result<PerKeyRgbConfig, String> {
    let layout_id = active_layout_id(state)?;
    let layout = state
        .layouts
        .get(&layout_id)
        .ok_or_else(|| format!("layout not found: {layout_id}"))?;
    let mut seen_keys = HashSet::new();
    let mut seen_leds = HashSet::new();
    let mut colors = Vec::with_capacity(desired.colors.len());

    for key_color in &desired.colors {
        if !seen_keys.insert(key_color.key_id.as_str()) {
            return Err(format!("duplicate per-key RGB key: {}", key_color.key_id));
        }
        let key = layout
            .key(&key_color.key_id)
            .ok_or_else(|| format!("key does not exist in active layout: {}", key_color.key_id))?;
        let led_index = key
            .led_index
            .ok_or_else(|| format!("key {} has no LED index", key_color.key_id))?;
        if !seen_leds.insert(led_index) {
            return Err(format!(
                "multiple physical keys resolve to LED index {led_index}"
            ));
        }
        if key_color.color != RgbColor::BLACK {
            colors.push(PerKeyRgbAssignment {
                led_index,
                color: key_color.color,
            });
        }
    }

    colors.sort_by_key(|assignment| assignment.led_index);
    Ok(PerKeyRgbConfig {
        brightness: desired.brightness,
        colors,
    })
}

fn configuration_scope(
    state: &AppState,
    capability: Capability,
) -> Result<ConfigurationScope, String> {
    let device_model_id = state
        .manager
        .status()
        .device
        .map(|device| device.device_model_id)
        .unwrap_or_else(|| state.primary_device_model_id.clone());

    let profile = state
        .persistence
        .default_profile(&device_model_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no default profile exists for {device_model_id}"))?;

    Ok(ConfigurationScope::new(
        device_model_id,
        profile.id,
        capability,
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let primary_driver: Arc<dyn KeyboardDriver> = Arc::new(AulaF108ProRemapDriver::new());
            let mut registry = DriverRegistry::new();
            registry.register(primary_driver.clone())?;
            let manifests = registry.manifests();

            let mut layouts = LayoutRegistry::new();
            for layout in registry.layouts() {
                layouts.register(layout)?;
            }

            let driver_layout_ids: BTreeMap<String, String> = manifests
                .iter()
                .filter_map(|manifest| {
                    manifest
                        .layout_ids
                        .first()
                        .map(|layout_id| (manifest.driver_id.clone(), layout_id.clone()))
                })
                .collect();

            let data_dir = app.path().app_data_dir()?;
            let persistence = PersistenceStore::open(data_dir.join("mechkeymesh.sqlite3"))?;

            for manifest in &manifests {
                persistence.ensure_device_model(&DeviceModel::new(
                    manifest.device_model_id.clone(),
                    manifest.vendor.clone(),
                    manifest.device_name.clone(),
                ))?;
                persistence.ensure_default_profile(&manifest.device_model_id)?;
            }

            let primary_manifest = manifests
                .first()
                .expect("at least the AULA F108 Pro driver must be registered");
            let primary_device_model_id = primary_manifest.device_model_id.clone();
            let primary_layout_id = primary_manifest
                .layout_ids
                .first()
                .expect("the primary driver must declare at least one layout")
                .clone();
            let lighting_effects = primary_driver.hardware_lighting_effects();
            assert!(
                primary_driver
                    .keymap_capabilities(KeymapLayer::Normal)
                    .is_some(),
                "the F108 driver must expose normal-layer remap capabilities"
            );
            assert!(
                primary_driver
                    .keymap_capabilities(KeymapLayer::Fn)
                    .is_some(),
                "the R7 F108 driver must expose Fn-layer remap capabilities"
            );
            assert!(
                primary_driver.per_key_rgb_capabilities().is_some(),
                "the R8 F108 driver must expose stored per-key RGB capabilities"
            );

            let manager = DeviceManager::start(registry, ManagerConfig::default());

            app.manage(AppState {
                manager,
                persistence,
                layouts,
                driver_layout_ids,
                primary_driver,
                lighting_effects,
                primary_device_model_id,
                primary_layout_id,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_logs,
            get_driver_registry,
            get_persistence_status,
            get_desired_lighting,
            get_desired_per_key_rgb,
            get_desired_keymap,
            get_layout,
            get_primary_layout,
            get_keymap_capabilities,
            get_fn_layer_descriptor,
            get_per_key_rgb_capabilities,
            reconnect,
            cancel_active_operation,
            get_lighting_effects,
            apply_lighting,
            save_per_key_rgb,
            read_per_key_rgb,
            save_keymap,
            reset_key_assignments
        ])
        .build(tauri::generate_context!())
        .expect("error while building MechKeyMesh")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app_handle.state::<AppState>().manager.shutdown();
            }
        });
}
