use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeObservations {
    connection: Option<mechkeymesh_driver_api::Connection>,
    battery_percent: Option<u8>,
    charging: Option<bool>,
    firmware: Option<String>,
    hardware_revision: Option<String>,
    knob_policy: &'static str,
    evidence_note: &'static str,
}

#[tauri::command]
pub(crate) fn get_native_observations(state: State<'_, AppState>) -> NativeObservations {
    let status = state.manager.status();
    let device = match status.state {
        mechkeymesh_core::DeviceState::Ready | mechkeymesh_core::DeviceState::Busy => status.device,
        _ => None,
    };
    NativeObservations {
        connection: device.as_ref().map(|device| device.connection),
        battery_percent: device.as_ref().and_then(|device| device.battery_percent),
        charging: None,
        firmware: device.and_then(|device| device.firmware),
        hardware_revision: None,
        knob_policy: "Native firmware control. MechKeyMesh does not intercept or remap the knob.",
        evidence_note: "Battery percentage is available through the mapped 2.4 GHz command. Charging, firmware, hardware revision and menu readback have no verified decoder. No unknown commands are sent.",
    }
}
