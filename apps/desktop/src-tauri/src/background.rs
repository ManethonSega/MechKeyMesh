use super::*;
use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_autostart::ManagerExt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct BackgroundPreferences {
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub active_profile_id: String,
}

impl Default for BackgroundPreferences {
    fn default() -> Self {
        Self {
            minimize_to_tray: true,
            start_minimized: false,
            active_profile_id: "default".into(),
        }
    }
}

pub(crate) struct BackgroundController {
    pub preferences: Mutex<BackgroundPreferences>,
    pub preferences_path: PathBuf,
    pub paused: AtomicBool,
    pub sleeping: AtomicBool,
    pub resume_pending: AtomicBool,
    pub exiting: AtomicBool,
}

impl BackgroundController {
    pub fn new(path: PathBuf) -> Self {
        let preferences = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            preferences: Mutex::new(preferences),
            preferences_path: path,
            paused: AtomicBool::new(false),
            sleeping: AtomicBool::new(false),
            resume_pending: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackgroundStatus {
    preferences: BackgroundPreferences,
    start_with_windows: bool,
    paused: bool,
    sleeping: bool,
}

#[tauri::command]
pub(crate) fn get_background_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<BackgroundStatus, String> {
    Ok(BackgroundStatus {
        preferences: state
            .background
            .preferences
            .lock()
            .expect("preferences lock poisoned")
            .clone(),
        start_with_windows: app
            .autolaunch()
            .is_enabled()
            .map_err(|error| error.to_string())?,
        paused: state.background.paused.load(Ordering::Acquire),
        sleeping: state.background.sleeping.load(Ordering::Acquire),
    })
}

#[tauri::command]
pub(crate) fn save_background_preferences(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut preferences: BackgroundPreferences,
    start_with_windows: bool,
) -> Result<(), String> {
    preferences.active_profile_id = state
        .profiles
        .active_profile_id
        .lock()
        .expect("profile lock poisoned")
        .clone();
    let mut saved = state
        .background
        .preferences
        .lock()
        .expect("preferences lock poisoned");
    let bytes = serde_json::to_vec_pretty(&preferences).map_err(|error| error.to_string())?;
    let temporary = state.background.preferences_path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, &state.background.preferences_path)
        .map_err(|error| error.to_string())?;
    *saved = preferences;
    let autostart = app.autolaunch();
    if start_with_windows {
        autostart.enable()
    } else {
        autostart.disable()
    }
    .map_err(|error| error.to_string())
}

pub(crate) fn remember_active_profile(state: &AppState, profile_id: &str) -> Result<(), String> {
    let mut saved = state
        .background
        .preferences
        .lock()
        .expect("preferences lock poisoned");
    if saved.active_profile_id == profile_id {
        return Ok(());
    }
    let mut updated = saved.clone();
    updated.active_profile_id = profile_id.into();
    let bytes = serde_json::to_vec_pretty(&updated).map_err(|error| error.to_string())?;
    let temporary = state.background.preferences_path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, &state.background.preferences_path)
        .map_err(|error| error.to_string())?;
    *saved = updated;
    Ok(())
}

pub(crate) fn set_paused(state: &AppState, paused: bool) {
    state.background.paused.store(paused, Ordering::Release);
    if paused {
        stop_live_rgb_controller(state);
        stop_music_controller(state);
    } else {
        state
            .background
            .resume_pending
            .store(true, Ordering::Release);
    }
}

#[tauri::command]
pub(crate) fn pause_background_effects(state: State<'_, AppState>, paused: bool) {
    set_paused(&state, paused);
}

pub(crate) fn effects_blocked(state: &AppState) -> bool {
    state.background.paused.load(Ordering::Acquire)
        || state.background.sleeping.load(Ordering::Acquire)
        || state.background.exiting.load(Ordering::Acquire)
}

pub(crate) fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn menu(app: &tauri::AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let state = app.state::<AppState>();
    let show = MenuItem::with_id(app, "show", "Open MechKeyMesh", true, None::<&str>)?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        if state.background.paused.load(Ordering::Acquire) {
            "Resume live effects"
        } else {
            "Pause live effects"
        },
        true,
        None::<&str>,
    )?;
    let reconnect = MenuItem::with_id(app, "reconnect", "Reconnect keyboard", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit MechKeyMesh", true, None::<&str>)?;
    let profiles = Submenu::new(app, "Active profile", true)?;
    let active = state
        .profiles
        .active_profile_id
        .lock()
        .expect("profile lock poisoned")
        .clone();
    for profile in state
        .persistence
        .list_profiles(&state.primary_device_model_id)?
        .into_iter()
        .take(100)
    {
        let label = format!(
            "{}{}",
            if profile.id == active { "✓ " } else { "" },
            profile.name
        );
        profiles.append(&MenuItem::with_id(
            app,
            format!("profile:{}", profile.id),
            label,
            true,
            None::<&str>,
        )?)?;
    }
    Ok(Menu::with_items(
        app,
        &[&show, &pause, &profiles, &reconnect, &exit],
    )?)
}

pub(crate) fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Guaranteed visible tray icon even when the bundle has no default window icon.
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let offset = (y * 32 + x) * 4;
            let bright = (6..26).contains(&x)
                && (6..26).contains(&y)
                && (!(10..=21).contains(&x) || !(10..=21).contains(&y));
            pixels[offset..offset + 4].copy_from_slice(if bright {
                &[33, 230, 232, 255]
            } else {
                &[8, 19, 39, 255]
            });
        }
    }
    TrayIconBuilder::with_id("main-tray")
        .icon(tauri::image::Image::new_owned(pixels, 32, 32))
        .tooltip("MechKeyMesh")
        .menu(&menu(app)?)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "show" => show_main(app),
                "exit" => {
                    let state = app.state::<AppState>();
                    state.background.exiting.store(true, Ordering::Release);
                    state.manager.cancel_active_operation();
                    stop_live_rgb_controller(&state);
                    stop_music_controller(&state);
                    app.exit(0);
                }
                "pause" => {
                    let state = app.state::<AppState>();
                    set_paused(&state, !state.background.paused.load(Ordering::Acquire));
                }
                "reconnect" => app.state::<AppState>().manager.reconnect(),
                _ => {
                    if let Some(profile_id) = id.strip_prefix("profile:") {
                        let app = app.clone();
                        let profile_id = profile_id.to_string();
                        // USB transactions never block the native menu/event loop.
                        tauri::async_runtime::spawn_blocking(move || {
                            let state = app.state::<AppState>();
                            // Manual tray selection takes priority over automatic rules.
                            state.profiles.auto_switch.store(false, Ordering::Release);
                            let _ = activate_profile_internal(&state, &profile_id);
                        });
                    }
                }
            }
        })
        .build(app)?;
    let preferences = app
        .state::<AppState>()
        .background
        .preferences
        .lock()
        .expect("preferences lock poisoned")
        .clone();
    if preferences.start_minimized || std::env::args().any(|arg| arg == "--minimized") {
        // Window starts hidden; no startup flash.
    } else {
        show_main(app);
    }
    let app = app.clone();
    thread::spawn(move || {
        let mut last_signature = String::new();
        loop {
            thread::sleep(Duration::from_secs(2));
            let state = app.state::<AppState>();
            if state.background.exiting.load(Ordering::Acquire) {
                break;
            }
            let profiles = state
                .persistence
                .list_profiles(&state.primary_device_model_id)
                .unwrap_or_default();
            let signature = format!(
                "{:?}|{}|{}",
                profiles,
                state
                    .profiles
                    .active_profile_id
                    .lock()
                    .expect("profile lock poisoned"),
                state.background.paused.load(Ordering::Acquire)
            );
            if signature == last_signature {
                continue;
            }
            last_signature = signature;
            let main = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let (Some(tray), Ok(menu)) = (main.tray_by_id("main-tray"), menu(&main)) {
                    let _ = tray.set_menu(Some(menu));
                }
            });
        }
    });
    Ok(())
}

pub(crate) fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let Some(state) = window.app_handle().try_state::<AppState>() else {
        return;
    };
    if state.background.exiting.load(Ordering::Acquire) {
        return;
    }
    if !state
        .background
        .preferences
        .lock()
        .expect("preferences lock poisoned")
        .minimize_to_tray
    {
        return;
    }
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            if window.hide().is_ok() {
                api.prevent_close();
            }
        }
        tauri::WindowEvent::Resized(_) => {
            if window.is_minimized().unwrap_or(false) {
                let _ = window.hide();
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_preferences_keep_safe_defaults() {
        let preferences: BackgroundPreferences =
            serde_json::from_str("{\"startMinimized\":true}").unwrap();
        assert!(preferences.minimize_to_tray);
        assert!(preferences.start_minimized);
        assert_eq!(preferences.active_profile_id, "default");
    }
}
