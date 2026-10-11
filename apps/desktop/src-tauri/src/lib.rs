use mechkeymesh_config::{
    ConfigurationScope, DesiredKeymap, DesiredLighting, DeviceModel, KeyMapping,
};
use mechkeymesh_core::{
    DeviceManager, DeviceManagerHandle, DeviceStatus, DriverRegistry, LogEntry, ManagerConfig,
};
use mechkeymesh_driver_api::{
    Capability, DriverManifest, KeyAssignment, KeyboardDriver, KeymapLayer, LightingConfig,
    LightingEffectDescriptor, NormalRemapCapabilities,
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
    normal_remap_capabilities: NormalRemapCapabilities,
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
fn get_desired_normal_keymap(state: State<'_, AppState>) -> Result<DesiredKeymap, String> {
    let scope = configuration_scope(&state, Capability::Keys)?;
    state
        .persistence
        .load_keymap(&scope, KeymapLayer::Normal)
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
fn get_normal_remap_capabilities(state: State<'_, AppState>) -> NormalRemapCapabilities {
    state.normal_remap_capabilities.clone()
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
fn save_normal_keymap(
    state: State<'_, AppState>,
    mappings: Vec<KeyMapping>,
) -> Result<KeymapApplyResult, String> {
    let scope = configuration_scope(&state, Capability::Keys)?;
    let desired = DesiredKeymap::normal(scope, mappings);
    let assignments = resolve_key_assignments(&state, &desired)?;

    state
        .primary_driver
        .validate_keymap(KeymapLayer::Normal, &assignments)
        .map_err(|error| error.to_string())?;

    state
        .persistence
        .save_keymap(&desired)
        .map_err(|error| error.to_string())?;

    match state
        .manager
        .set_keymap(KeymapLayer::Normal, assignments.clone())
    {
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
            let normal_remap_capabilities = primary_driver
                .normal_remap_capabilities()
                .expect("the R6 F108 driver must expose normal-remap capabilities");

            let manager = DeviceManager::start(registry, ManagerConfig::default());

            app.manage(AppState {
                manager,
                persistence,
                layouts,
                driver_layout_ids,
                primary_driver,
                lighting_effects,
                normal_remap_capabilities,
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
            get_desired_normal_keymap,
            get_layout,
            get_primary_layout,
            get_normal_remap_capabilities,
            reconnect,
            cancel_active_operation,
            get_lighting_effects,
            apply_lighting,
            save_normal_keymap
        ])
        .build(tauri::generate_context!())
        .expect("error while building MechKeyMesh")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app_handle.state::<AppState>().manager.shutdown();
            }
        });
}
