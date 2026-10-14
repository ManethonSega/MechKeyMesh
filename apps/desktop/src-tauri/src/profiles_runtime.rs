use super::*;
use mechkeymesh_config::Profile;
use std::hash::{DefaultHasher, Hash, Hasher};

#[tauri::command]
pub(crate) async fn select_profile_application_file() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(|| {
            let Some(path) = rfd::FileDialog::new()
                .set_title("Choose an application for automatic profile switching")
                .add_filter("Windows applications", &["exe"])
                .pick_file()
            else {
                return Ok(None);
            };
            if !path.is_file() {
                return Err("Choose an existing application (.exe) file.".into());
            }
            foreground::application_executable_name(&path).map(Some)
        })
        .await
        .map_err(|error| error.to_string())?
    }
    #[cfg(not(windows))]
    {
        Err("Application browsing is available on Windows.".into())
    }
}

#[tauri::command]
pub(crate) fn get_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String> {
    state
        .persistence
        .list_profiles(&state.primary_device_model_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn create_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Profile, String> {
    let source = active_profile_id(&state);
    state
        .persistence
        .clone_profile(&state.primary_device_model_id, &source, &id, &name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn rename_profile(
    state: State<'_, AppState>,
    profile_id: String,
    name: String,
) -> Result<(), String> {
    state
        .persistence
        .rename_profile(&state.primary_device_model_id, &profile_id, &name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_profile(state: State<'_, AppState>, profile_id: String) -> Result<(), String> {
    if active_profile_id(&state) == profile_id {
        activate_profile_internal(&state, "default")?;
    }
    state
        .persistence
        .delete_profile(&state.primary_device_model_id, &profile_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_application_rules(
    state: State<'_, AppState>,
) -> Result<Vec<ApplicationRule>, String> {
    state
        .persistence
        .list_application_rules(&state.primary_device_model_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn save_application_rule(
    state: State<'_, AppState>,
    mut rule: ApplicationRule,
) -> Result<(), String> {
    rule.device_model_id = state.primary_device_model_id.clone();
    state
        .persistence
        .save_application_rule(&rule)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_application_rule(
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
) -> Result<(), String> {
    state
        .persistence
        .delete_application_rule(&state.primary_device_model_id, &profile_id, &id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_profile_runtime_status(state: State<'_, AppState>) -> ProfileRuntimeStatus {
    ProfileRuntimeStatus {
        active_profile_id: active_profile_id(&state),
        auto_switch: state.profiles.auto_switch.load(Ordering::Acquire),
        foreground_executable: state
            .profiles
            .foreground_executable
            .lock()
            .expect("foreground executable lock poisoned")
            .clone(),
        last_report: state
            .profiles
            .last_report
            .lock()
            .expect("profile report lock poisoned")
            .clone(),
    }
}

#[tauri::command]
pub(crate) fn set_profile_auto_switch(state: State<'_, AppState>, enabled: bool) {
    state.profiles.auto_switch.store(enabled, Ordering::Release);
}

#[tauri::command]
pub(crate) fn activate_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProfileApplyReport, String> {
    activate_profile_internal(&state, &profile_id)
}

#[tauri::command]
pub(crate) fn assign_lcd_asset_to_profile(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let asset =
        mechkeymesh_lcd_engine::F108Asset::from_bytes(bytes).map_err(|error| error.to_string())?;
    let profile_id = active_profile_id(&state);
    let mut host = state
        .persistence
        .load_profile_host_state(&state.primary_device_model_id, &profile_id)
        .map_err(|error| error.to_string())?;
    host.lcd_asset_path = Some(path);
    host.lcd_content_hash = Some(lcd_asset_hash(&asset));
    state
        .persistence
        .save_profile_host_state(&state.primary_device_model_id, &profile_id, &host)
        .map_err(|error| error.to_string())
}

pub(crate) fn activate_profile_internal(
    state: &AppState,
    profile_id: &str,
) -> Result<ProfileApplyReport, String> {
    let _apply_guard = state
        .profiles
        .apply_lock
        .lock()
        .expect("profile apply lock poisoned");
    if !state
        .persistence
        .list_profiles(&state.primary_device_model_id)
        .map_err(|error| error.to_string())?
        .iter()
        .any(|profile| profile.id == profile_id)
    {
        return Err(format!("profile not found: {profile_id}"));
    }

    let capabilities = state
        .manager
        .status()
        .device
        .as_ref()
        .map(|device| device.capabilities.clone())
        .unwrap_or_default();
    let mut report = ProfileApplyReport {
        profile_id: profile_id.into(),
        changed: Vec::new(),
        skipped: Vec::new(),
        errors: Vec::new(),
    };
    let mut applied = state
        .profiles
        .applied
        .lock()
        .expect("profile applied-state lock poisoned");

    let macros = load_hardware_macros_for_profile(state, profile_id)?;
    apply_component(
        "macros",
        json(&macros)?,
        Some(Capability::Macros),
        &capabilities,
        &mut applied,
        &mut report,
        || state.manager.set_macros(macros),
    );

    for (name, layer, capability) in [
        ("mappings", KeymapLayer::Normal, Capability::Keys),
        ("fn-mappings", KeymapLayer::Fn, Capability::FnLayer),
    ] {
        let scope = ConfigurationScope::new(
            state.primary_device_model_id.clone(),
            profile_id,
            capability,
        );
        let desired = state
            .persistence
            .load_keymap(&scope, layer)
            .map_err(|error| error.to_string())?;
        let assignments = resolve_key_assignments(state, &desired)?;
        let digest = json(&assignments)?;
        apply_component(
            name,
            digest,
            Some(capability),
            &capabilities,
            &mut applied,
            &mut report,
            || state.manager.set_keymap(layer, assignments),
        );
    }

    let lighting_scope = ConfigurationScope::new(
        state.primary_device_model_id.clone(),
        profile_id,
        Capability::HardwareRgb,
    );
    if let Some(desired) = state
        .persistence
        .load_lighting(&lighting_scope)
        .map_err(|error| error.to_string())?
    {
        apply_component(
            "hardware-rgb",
            json(&desired.config)?,
            Some(Capability::HardwareRgb),
            &capabilities,
            &mut applied,
            &mut report,
            || state.manager.set_lighting(desired.config),
        );
    }

    let per_key_scope = ConfigurationScope::new(
        state.primary_device_model_id.clone(),
        profile_id,
        Capability::PerKeyRgb,
    );
    let per_key = state
        .persistence
        .load_per_key_rgb(&per_key_scope)
        .map_err(|error| error.to_string())?;
    let per_key_config = resolve_per_key_rgb(state, &per_key)?;
    let per_key_digest = json(&per_key_config)?;
    apply_component(
        "per-key-rgb",
        per_key_digest,
        Some(Capability::PerKeyRgb),
        &capabilities,
        &mut applied,
        &mut report,
        || state.manager.set_per_key_rgb(per_key_config),
    );

    let settings_scope = ConfigurationScope::new(
        state.primary_device_model_id.clone(),
        profile_id,
        Capability::DeviceSettings,
    );
    if let Some(settings) = state
        .persistence
        .load_device_settings(&settings_scope)
        .map_err(|error| error.to_string())?
    {
        apply_component(
            "device-settings",
            json(&settings)?,
            Some(Capability::DeviceSettings),
            &capabilities,
            &mut applied,
            &mut report,
            || state.manager.set_device_settings(settings),
        );
    }

    let host = state
        .persistence
        .load_profile_host_state(&state.primary_device_model_id, profile_id)
        .map_err(|error| error.to_string())?;
    apply_host_state(state, &host, &capabilities, &mut applied, &mut report);

    *state
        .profiles
        .active_profile_id
        .lock()
        .expect("active profile lock poisoned") = profile_id.into();
    if let Err(error) = background::remember_active_profile(state, profile_id) {
        report.errors.push(format!("profile persistence: {error}"));
    }
    *state
        .profiles
        .last_report
        .lock()
        .expect("profile report lock poisoned") = Some(report.clone());
    Ok(report)
}

fn apply_host_state(
    state: &AppState,
    host: &ProfileHostState,
    capabilities: &[Capability],
    applied: &mut BTreeMap<String, String>,
    report: &mut ProfileApplyReport,
) {
    let digest = json(&(
        &host.host_mode,
        &host.live_scene_json,
        &host.music_config_json,
    ))
    .unwrap_or_default();
    if background::effects_blocked(state) {
        applied.remove("host-lighting");
        report.skipped.push("host-lighting: paused".into());
    } else {
        match host.host_mode.as_str() {
            "live-rgb" => match host
                .live_scene_json
                .as_deref()
                .ok_or_else(|| "profile has no live RGB scene".to_string())
                .and_then(|value| {
                    serde_json::from_str::<LiveRgbScene>(value).map_err(|error| error.to_string())
                }) {
                Ok(scene) => apply_component(
                    "host-lighting",
                    digest.clone(),
                    Some(Capability::LiveRgb),
                    capabilities,
                    applied,
                    report,
                    || start_live_rgb_internal(state, scene),
                ),
                Err(error) => report.errors.push(format!("live-effects: {error}")),
            },
            "music" => match host
                .music_config_json
                .as_deref()
                .ok_or_else(|| "profile has no music configuration".to_string())
                .and_then(|value| {
                    serde_json::from_str::<MusicLightingConfig>(value)
                        .map_err(|error| error.to_string())
                }) {
                Ok(config) => apply_component(
                    "host-lighting",
                    digest.clone(),
                    Some(Capability::Music),
                    capabilities,
                    applied,
                    report,
                    || start_music_lighting_internal(state, config),
                ),
                Err(error) => report.errors.push(format!("music: {error}")),
            },
            _ => apply_component(
                "host-lighting",
                digest,
                None,
                capabilities,
                applied,
                report,
                || {
                    stop_live_rgb_controller(state);
                    stop_music_controller(state);
                    Ok(())
                },
            ),
        }
    }

    if let (Some(path), Some(content_hash)) = (&host.lcd_asset_path, &host.lcd_content_hash) {
        if state.manager.lcd_transfer_status().incomplete {
            report
                .skipped
                .push("lcd-media: previous transfer incomplete; manual re-upload required".into());
            return;
        }
        let asset = match std::fs::read(path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                mechkeymesh_lcd_engine::F108Asset::from_bytes(bytes)
                    .map_err(|error| error.to_string())
            }) {
            Ok(asset) => asset,
            Err(error) => {
                report.errors.push(format!("lcd-media: {error}"));
                return;
            }
        };
        let current_hash = lcd_asset_hash(&asset);
        if &current_hash != content_hash {
            report
                .errors
                .push("lcd-media: assigned asset changed; validate and assign it again".into());
            return;
        }
        let lcd_digest = format!("{path}\n{current_hash}");
        apply_component(
            "lcd-media",
            lcd_digest,
            Some(Capability::Display),
            capabilities,
            applied,
            report,
            || {
                state.manager.upload_lcd(LcdUpload {
                    width: mechkeymesh_lcd_engine::WIDTH,
                    height: mechkeymesh_lcd_engine::HEIGHT,
                    pixel_format: LcdPixelFormat::Rgb565LittleEndian,
                    frame_count: asset.frame_count(),
                    header_bytes: mechkeymesh_lcd_engine::HEADER_BYTES,
                    content_length: asset.content_length(),
                    payload: asset.bytes().to_vec(),
                })
            },
        );
    }
}

fn lcd_asset_hash(asset: &mechkeymesh_lcd_engine::F108Asset) -> String {
    let mut hasher = DefaultHasher::new();
    asset.bytes().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn apply_component(
    name: &str,
    digest: String,
    capability: Option<Capability>,
    capabilities: &[Capability],
    applied: &mut BTreeMap<String, String>,
    report: &mut ProfileApplyReport,
    operation: impl FnOnce() -> Result<(), String>,
) {
    if applied.get(name) == Some(&digest) {
        report.skipped.push(format!("{name}: unchanged"));
        return;
    }
    if capability.is_some_and(|required| !capabilities.contains(&required)) {
        report
            .skipped
            .push(format!("{name}: unavailable on active connection"));
        return;
    }
    match operation() {
        Ok(()) => {
            applied.insert(name.into(), digest);
            report.changed.push(name.into());
        }
        Err(error) => report.errors.push(format!("{name}: {error}")),
    }
}

fn active_profile_id(state: &AppState) -> String {
    state
        .profiles
        .active_profile_id
        .lock()
        .expect("active profile lock poisoned")
        .clone()
}

fn json(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

pub(crate) fn monitor_profiles(app_handle: tauri::AppHandle) {
    let _ = thread::Builder::new()
        .name("mechkeymesh-profile-monitor".into())
        .spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(500));
                let state = app_handle.state::<AppState>();
                if state.background.exiting.load(Ordering::Acquire) {
                    break;
                }
                if state.background.sleeping.load(Ordering::Acquire) || state.manager.is_suspended()
                {
                    continue;
                }
                let status = state.manager.status();
                if status.state != mechkeymesh_core::DeviceState::Ready {
                    continue;
                }
                let resume_pending = state
                    .background
                    .resume_pending
                    .swap(false, Ordering::AcqRel);
                let device_instance = state
                    .manager
                    .status()
                    .device
                    .map(|device| device.instance_id);
                let reconnected = {
                    let mut last = state
                        .profiles
                        .last_device_instance
                        .lock()
                        .expect("profile device lock poisoned");
                    if *last != device_instance {
                        *last = device_instance.clone();
                        true
                    } else {
                        false
                    }
                };
                if reconnected && device_instance.is_some() {
                    state
                        .profiles
                        .applied
                        .lock()
                        .expect("profile applied-state lock poisoned")
                        .clear();
                    let _ = activate_profile_internal(&state, &active_profile_id(&state));
                }

                if resume_pending && !reconnected {
                    state
                        .profiles
                        .applied
                        .lock()
                        .expect("profile applied-state lock poisoned")
                        .remove("host-lighting");
                    let _ = activate_profile_internal(&state, &active_profile_id(&state));
                }

                let executable = foreground::executable_name();
                *state
                    .profiles
                    .foreground_executable
                    .lock()
                    .expect("foreground executable lock poisoned") = executable.clone();
                if !state.profiles.auto_switch.load(Ordering::Acquire) {
                    continue;
                }
                let Some(executable) = executable else {
                    continue;
                };
                let Ok(rules) = state
                    .persistence
                    .list_application_rules(&state.primary_device_model_id)
                else {
                    continue;
                };
                let target = rules
                    .iter()
                    .find(|rule| {
                        rule.enabled
                            && foreground::pattern_matches(&rule.executable_pattern, &executable)
                    })
                    .map(|rule| rule.profile_id.clone())
                    .unwrap_or_else(|| "default".into());
                if target != active_profile_id(&state) {
                    let _ = activate_profile_internal(&state, &target);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_components_emit_no_second_write_and_failure_does_not_cache() {
        let mut applied = BTreeMap::new();
        let mut report = ProfileApplyReport {
            profile_id: "test".into(),
            changed: Vec::new(),
            skipped: Vec::new(),
            errors: Vec::new(),
        };
        let mut writes = 0;
        for _ in 0..2 {
            apply_component(
                "rgb",
                "same".into(),
                None,
                &[],
                &mut applied,
                &mut report,
                || {
                    writes += 1;
                    Ok(())
                },
            );
        }
        assert_eq!(writes, 1);
        apply_component(
            "rgb",
            "new".into(),
            None,
            &[],
            &mut applied,
            &mut report,
            || Err("failed".into()),
        );
        assert_eq!(applied.get("rgb").map(String::as_str), Some("same"));
        assert_eq!(report.errors.len(), 1);
    }
}
