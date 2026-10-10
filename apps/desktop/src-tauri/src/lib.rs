use mechkeymesh_core::{DeviceManager, DeviceManagerHandle, DeviceStatus, LogEntry};
use mechkeymesh_driver_api::{KeyboardDriver, LightingConfig};
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
fn reconnect(state: State<'_, AppState>) {
    state.manager.reconnect();
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
    let drivers: Vec<Arc<dyn KeyboardDriver>> = vec![Arc::new(AulaF108ProDriver::new())];
    let manager = DeviceManager::start(drivers);
    let manager_for_shutdown = manager.clone();

    tauri::Builder::default()
        .manage(AppState { manager })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_logs,
            reconnect,
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
