use mechkeymesh_core::{
    DeviceManager, DeviceManagerHandle, DeviceStatus, DriverRegistry, LogEntry, ManagerConfig,
};
use mechkeymesh_driver_api::{DriverManifest, LightingConfig};
use mechkeymesh_driver_aula_f108pro::{AulaF108ProDriver, LIGHTING_MODES};
use std::sync::Arc;
use tauri::State;

struct AppState {
    manager: DeviceManagerHandle,
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
fn apply_lighting(state: State<'_, AppState>, config: LightingConfig) -> Result<(), String> {
    state.manager.set_lighting(config)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut registry = DriverRegistry::new();
    registry
        .register(Arc::new(AulaF108ProDriver::new()))
        .expect("failed to register AULA F108 Pro driver");

    let manager = DeviceManager::start(registry, ManagerConfig::default());
    let manager_for_shutdown = manager.clone();

    tauri::Builder::default()
        .manage(AppState { manager })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_logs,
            get_driver_registry,
            reconnect,
            cancel_active_operation,
            get_lighting_modes,
            apply_lighting
        ])
        .build(tauri::generate_context!())
        .expect("error while building MechKeyMesh")
        .run(move |_app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                manager_for_shutdown.shutdown();
            }
        });
}
