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
        knob_policy: "The knob controls the keyboard screen or Windows volume. User testing confirms the same behavior over USB and 2.4 GHz. MechKeyMesh leaves native controls unchanged.",
        evidence_note: "Battery percentage is reported over 2.4 GHz. The keyboard can charge from external power while using wireless. Charging status, firmware and hardware revision are unavailable to the app. Changes made in the keyboard menu do not update app settings automatically; Save & apply sends your app settings to the keyboard.",
    }
}
