use mechkeymesh_config::{ConfigurationScope, DesiredLighting, DeviceModel};
use mechkeymesh_core::{
    DeviceManager, DeviceManagerHandle, DeviceStatus, DriverRegistry, LogEntry, ManagerConfig,
};
use mechkeymesh_driver_api::{Capability, DriverManifest, LightingConfig};
use mechkeymesh_driver_aula_f108pro::{f108pro_layout, AulaF108ProDriver, LIGHTING_MODES};
use mechkeymesh_layout::{Layout, LayoutRegistry};
use mechkeymesh_persistence::{PersistenceStatus, PersistenceStore};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Manager, State};

struct AppState {
    manager: DeviceManagerHandle,
    persistence: PersistenceStore,
    layouts: LayoutRegistry,
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
    let scope = lighting_scope(&state)?;
    state
        .persistence
        .load_lighting(&scope)
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
    get_layout(state, state.primary_layout_id.clone())
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
fn get_lighting_modes() -> Vec<String> {
    LIGHTING_MODES
        .iter()
        .map(|mode| (*mode).to_string())
        .collect()
}

#[tauri::command]
fn apply_lighting(
    state: State<'_, AppState>,
    config: LightingConfig,
) -> Result<LightingApplyResult, String> {
    let scope = lighting_scope(&state)?;
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

fn lighting_scope(state: &AppState) -> Result<ConfigurationScope, String> {
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
        Capability::HardwareRgb,
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let mut registry = DriverRegistry::new();
            registry.register(Arc::new(AulaF108ProDriver::new()))?;
            let manifests = registry.manifests();

            let mut layouts = LayoutRegistry::new();
            layouts.register(f108pro_layout())?;

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

            let manager = DeviceManager::start(registry, ManagerConfig::default());

            app.manage(AppState {
                manager,
                persistence,
                layouts,
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
            get_layout,
            get_primary_layout,
            reconnect,
            cancel_active_operation,
            get_lighting_modes,
            apply_lighting
        ])
        .build(tauri::generate_context!())
        .expect("error while building MechKeyMesh")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app_handle.state::<AppState>().manager.shutdown();
            }
        });
}
