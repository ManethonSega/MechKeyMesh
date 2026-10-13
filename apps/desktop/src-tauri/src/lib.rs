use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{DynamicImage, ImageFormat};
use mechkeymesh_audio_engine::{
    AudioOutputDevice, MusicLightingConfig, SpectrumAnalyzer, SpectrumFrame, default_music_config,
    output_devices, render_music, run_loopback_capture,
};
use mechkeymesh_config::{
    ConfigurationScope, DesiredKeymap, DesiredLighting, DesiredPerKeyRgb, DeviceModel, KeyColor,
    KeyMapping, MacroEvent, MacroMouseButton, MacroPlayback, MacroProgram, MacroSimulation,
};
use mechkeymesh_core::{
    DeviceManager, DeviceManagerHandle, DeviceStatus, DriverRegistry, LcdTransferStatus,
    LiveRgbTransportStatus, LogEntry, ManagerConfig,
};
use mechkeymesh_driver_api::{
    Capability, DeviceClock, DeviceSettings, DeviceSettingsDescriptor, DriverManifest,
    FnLayerDescriptor, HardwareMacroEvent, HardwareMacroMouseButton, HardwareMacroProgram,
    KeyAssignment, KeyboardDriver, KeymapCapabilities, KeymapLayer, LcdPixelFormat, LcdUpload,
    LightingConfig, LightingEffectDescriptor, LiveRgbCapabilities, MacroBankDescriptor,
    PerKeyRgbAssignment, PerKeyRgbCapabilities, PerKeyRgbConfig, RgbColor,
};
use mechkeymesh_driver_aula_f108pro::AulaF108ProRemapDriver;
use mechkeymesh_layout::{KeyAction, Layout, LayoutRegistry};
use mechkeymesh_lcd_engine::{
    CropRect, Editor as LcdEditor, FitMode as LcdFitMode, ProjectSummary as LcdProjectSummary,
    VideoPreset as LcdVideoPreset, import_media,
};
use mechkeymesh_lighting_engine::{
    LiveRgbScene, ReactivePulse, RenderInput, default_scene, render as render_live_rgb,
};
use mechkeymesh_persistence::{
    ApplicationRule, PersistenceStatus, PersistenceStore, ProfileHostState,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{RecvTimeoutError, sync_channel},
};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Manager, State};

mod background;
mod foreground;
mod native_observations;
mod power;
mod profiles_runtime;

use profiles_runtime::*;

struct AppState {
    manager: DeviceManagerHandle,
    persistence: Arc<PersistenceStore>,
    layouts: LayoutRegistry,
    driver_layout_ids: BTreeMap<String, String>,
    primary_driver: Arc<dyn KeyboardDriver>,
    lighting_effects: Vec<LightingEffectDescriptor>,
    primary_device_model_id: String,
    primary_layout_id: String,
    live_rgb: LiveRgbController,
    music: MusicController,
    lcd: Mutex<Option<LcdEditor>>,
    profiles: ProfileController,
    background: background::BackgroundController,
}

#[derive(Default)]
struct ProfileController {
    apply_lock: Mutex<()>,
    active_profile_id: Mutex<String>,
    auto_switch: AtomicBool,
    foreground_executable: Mutex<Option<String>>,
    last_device_instance: Mutex<Option<String>>,
    applied: Mutex<BTreeMap<String, String>>,
    last_report: Mutex<Option<ProfileApplyReport>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileApplyReport {
    profile_id: String,
    changed: Vec<String>,
    skipped: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileRuntimeStatus {
    active_profile_id: String,
    auto_switch: bool,
    foreground_executable: Option<String>,
    last_report: Option<ProfileApplyReport>,
}

#[derive(Default)]
struct LiveRgbController {
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    stop: Mutex<Option<Arc<AtomicBool>>>,
    pulses: Arc<Mutex<Vec<(String, Instant)>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

#[derive(Default)]
struct MusicController {
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    stop: Mutex<Option<Arc<AtomicBool>>>,
    last_error: Arc<Mutex<Option<String>>>,
    spectrum: Arc<Mutex<SpectrumFrame>>,
    frames_analyzed: Arc<AtomicU64>,
    frames_rendered: Arc<AtomicU64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRgbRuntimeStatus {
    running: bool,
    last_error: Option<String>,
    transport: LiveRgbTransportStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MusicRuntimeStatus {
    running: bool,
    last_error: Option<String>,
    spectrum: SpectrumFrame,
    frames_analyzed: u64,
    frames_rendered: u64,
    transport: LiveRgbTransportStatus,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MacroBankApplyResult {
    applied: bool,
    macro_count: usize,
    event_count: usize,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceSettingsApplyResult {
    stored: bool,
    applied: bool,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LcdEditorState {
    project: LcdProjectSummary,
    durations_ms: Vec<u32>,
    can_undo: bool,
    can_redo: bool,
}

#[tauri::command]
fn select_lcd_media_file() -> Option<String> {
    #[cfg(windows)]
    {
        rfd::FileDialog::new()
            .set_title("Choose LCD media")
            .add_filter(
                "Supported media",
                &["png", "jpg", "jpeg", "gif", "webp", "mp4", "webm", "mov"],
            )
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
            .add_filter("Videos", &["mp4", "webm", "mov"])
            .pick_file()
            .map(|path| path.to_string_lossy().into_owned())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcdPoint {
    x: i32,
    y: i32,
}

#[tauri::command]
fn import_lcd_media(
    state: State<'_, AppState>,
    path: String,
    preset: LcdVideoPreset,
    fit: LcdFitMode,
) -> Result<LcdEditorState, String> {
    let project =
        import_media(&PathBuf::from(path), preset, fit).map_err(|error| error.to_string())?;
    let editor = LcdEditor::new(project);
    let result = lcd_editor_state(&editor);
    *state.lcd.lock().expect("LCD editor lock poisoned") = Some(editor);
    Ok(result)
}

#[tauri::command]
fn get_lcd_editor_state(state: State<'_, AppState>) -> Option<LcdEditorState> {
    state
        .lcd
        .lock()
        .expect("LCD editor lock poisoned")
        .as_ref()
        .map(lcd_editor_state)
}

#[tauri::command]
fn get_lcd_frame_preview(state: State<'_, AppState>, index: usize) -> Result<String, String> {
    let guard = state.lcd.lock().expect("LCD editor lock poisoned");
    let editor = guard
        .as_ref()
        .ok_or_else(|| "no LCD project is open".to_string())?;
    let frame = editor
        .project()
        .frames
        .get(index)
        .ok_or_else(|| format!("frame index {index} is out of range"))?;
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(frame.pixels.clone())
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        BASE64.encode(png.into_inner())
    ))
}

#[tauri::command]
fn set_lcd_frame_duration(
    state: State<'_, AppState>,
    index: usize,
    duration_ms: u32,
) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        editor
            .set_duration(index, duration_ms)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn duplicate_lcd_frame(state: State<'_, AppState>, index: usize) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        editor.duplicate(index).map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn delete_lcd_frame(state: State<'_, AppState>, index: usize) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        editor.delete(index).map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn reorder_lcd_frame(
    state: State<'_, AppState>,
    from: usize,
    to: usize,
) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        editor.reorder(from, to).map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn draw_lcd_brush(
    state: State<'_, AppState>,
    index: usize,
    points: Vec<LcdPoint>,
    radius: u32,
    color: [u8; 4],
) -> Result<LcdEditorState, String> {
    let points = points
        .into_iter()
        .map(|point| (point.x, point.y))
        .collect::<Vec<_>>();
    edit_lcd(&state, |editor| {
        editor
            .brush(index, &points, radius, color)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn erase_lcd(
    state: State<'_, AppState>,
    index: usize,
    points: Vec<LcdPoint>,
    radius: u32,
) -> Result<LcdEditorState, String> {
    let points = points
        .into_iter()
        .map(|point| (point.x, point.y))
        .collect::<Vec<_>>();
    edit_lcd(&state, |editor| {
        editor
            .erase(index, &points, radius)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn add_lcd_text(
    state: State<'_, AppState>,
    index: usize,
    x: i32,
    y: i32,
    text: String,
    scale: u32,
    color: [u8; 4],
) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        editor
            .text(index, x, y, &text, scale, color)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn crop_lcd_frame(
    state: State<'_, AppState>,
    index: usize,
    crop: CropRect,
    fit: LcdFitMode,
) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        editor
            .crop(index, crop, fit)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn undo_lcd_edit(state: State<'_, AppState>) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        if editor.undo() {
            Ok(())
        } else {
            Err("nothing to undo".into())
        }
    })
}

#[tauri::command]
fn redo_lcd_edit(state: State<'_, AppState>) -> Result<LcdEditorState, String> {
    edit_lcd(&state, |editor| {
        if editor.redo() {
            Ok(())
        } else {
            Err("nothing to redo".into())
        }
    })
}

#[tauri::command]
fn export_lcd_asset(
    state: State<'_, AppState>,
    path: String,
) -> Result<mechkeymesh_lcd_engine::AssetSummary, String> {
    let guard = state.lcd.lock().expect("LCD editor lock poisoned");
    let editor = guard
        .as_ref()
        .ok_or_else(|| "no LCD project is open".to_string())?;
    let asset = editor
        .project()
        .compile()
        .map_err(|error| error.to_string())?;
    std::fs::write(path, asset.bytes()).map_err(|error| error.to_string())?;
    Ok(asset.summary())
}

#[tauri::command]
fn get_lcd_transfer_status(state: State<'_, AppState>) -> LcdTransferStatus {
    state.manager.lcd_transfer_status()
}

#[tauri::command]
fn upload_lcd_asset(state: State<'_, AppState>) -> Result<(), String> {
    require_active_capability(&state, Capability::Display)?;
    let upload = {
        let guard = state.lcd.lock().expect("LCD editor lock poisoned");
        let editor = guard
            .as_ref()
            .ok_or_else(|| "no LCD project is open".to_string())?;
        let asset = editor
            .project()
            .compile()
            .map_err(|error| error.to_string())?;
        LcdUpload {
            width: mechkeymesh_lcd_engine::WIDTH,
            height: mechkeymesh_lcd_engine::HEIGHT,
            pixel_format: LcdPixelFormat::Rgb565LittleEndian,
            frame_count: asset.frame_count(),
            header_bytes: mechkeymesh_lcd_engine::HEADER_BYTES,
            content_length: asset.content_length(),
            payload: asset.bytes().to_vec(),
        }
    };
    state.manager.upload_lcd(upload)
}

#[tauri::command]
fn sync_lcd_clock(state: State<'_, AppState>, clock: DeviceClock) -> Result<(), String> {
    require_active_capability(&state, Capability::Clock)?;
    state.manager.sync_clock(clock)
}

fn edit_lcd(
    state: &State<'_, AppState>,
    operation: impl FnOnce(&mut LcdEditor) -> Result<(), String>,
) -> Result<LcdEditorState, String> {
    let mut guard = state.lcd.lock().expect("LCD editor lock poisoned");
    let editor = guard
        .as_mut()
        .ok_or_else(|| "no LCD project is open".to_string())?;
    operation(editor)?;
    Ok(lcd_editor_state(editor))
}

fn lcd_editor_state(editor: &LcdEditor) -> LcdEditorState {
    LcdEditorState {
        project: editor.project().summary(),
        durations_ms: editor
            .project()
            .frames
            .iter()
            .map(|frame| frame.duration_ms)
            .collect(),
        can_undo: editor.can_undo(),
        can_redo: editor.can_redo(),
    }
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> DeviceStatus {
    state.manager.status()
}

#[tauri::command]
fn get_live_rgb_defaults() -> LiveRgbScene {
    default_scene()
}

#[tauri::command]
fn get_live_rgb_capabilities() -> LiveRgbCapabilities {
    LiveRgbCapabilities {
        maximum_frames_per_second: 30,
        maximum_leds: 112,
        transport_buffered_frames: 1,
    }
}

#[tauri::command]
fn get_live_rgb_status(state: State<'_, AppState>) -> LiveRgbRuntimeStatus {
    let running = state
        .live_rgb
        .stop
        .lock()
        .expect("live RGB stop lock poisoned")
        .as_ref()
        .is_some_and(|stop| !stop.load(Ordering::Acquire));
    LiveRgbRuntimeStatus {
        running,
        last_error: state
            .live_rgb
            .last_error
            .lock()
            .expect("live RGB error lock poisoned")
            .clone(),
        transport: state.manager.live_rgb_status(),
    }
}

#[tauri::command]
fn get_music_defaults() -> MusicLightingConfig {
    default_music_config()
}

#[tauri::command]
fn get_audio_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    output_devices()
}

#[tauri::command]
fn get_music_status(state: State<'_, AppState>) -> MusicRuntimeStatus {
    let running = state
        .music
        .stop
        .lock()
        .expect("music stop lock poisoned")
        .as_ref()
        .is_some_and(|stop| !stop.load(Ordering::Acquire));
    MusicRuntimeStatus {
        running,
        last_error: state
            .music
            .last_error
            .lock()
            .expect("music error lock poisoned")
            .clone(),
        spectrum: state
            .music
            .spectrum
            .lock()
            .expect("music spectrum lock poisoned")
            .clone(),
        frames_analyzed: state.music.frames_analyzed.load(Ordering::Relaxed),
        frames_rendered: state.music.frames_rendered.load(Ordering::Relaxed),
        transport: state.manager.live_rgb_status(),
    }
}

#[tauri::command]
fn start_live_rgb(state: State<'_, AppState>, scene: LiveRgbScene) -> Result<(), String> {
    start_live_rgb_internal(&state, scene.clone())?;
    save_active_host_mode(&state, "live-rgb", Some(&scene), None)
}

fn start_live_rgb_internal(state: &AppState, scene: LiveRgbScene) -> Result<(), String> {
    if background::effects_blocked(state) {
        return Err(
            "Live effects are paused. Resume them from the tray or Background settings.".into(),
        );
    }
    let status = state.manager.status();
    let device = status
        .device
        .ok_or_else(|| "connect the F108 Pro over USB before starting live RGB".to_string())?;
    if !device.capabilities.contains(&Capability::LiveRgb) {
        return Err("live RGB is unavailable on the active connection".into());
    }

    let layout_id = active_layout_id(state)?;
    let layout = state
        .layouts
        .get(&layout_id)
        .cloned()
        .ok_or_else(|| format!("active layout {layout_id} is unavailable"))?;
    scene.validate(&layout).map_err(|error| error.to_string())?;

    stop_music_controller(state);

    stop_live_rgb_controller(state);
    settle_effect_workers(state);

    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut current = state
            .live_rgb
            .stop
            .lock()
            .expect("live RGB stop lock poisoned");
        if let Some(previous) = current.replace(stop.clone()) {
            previous.store(true, Ordering::Release);
        }
    }
    state.manager.stop_live_rgb();
    *state
        .live_rgb
        .last_error
        .lock()
        .expect("live RGB error lock poisoned") = None;

    let manager = state.manager.clone();
    let pulses = state.live_rgb.pulses.clone();
    let last_error = state.live_rgb.last_error.clone();
    let worker = thread::Builder::new()
        .name("mechkeymesh-live-rgb".into())
        .spawn(move || {
            let started = Instant::now();
            let frame_period = Duration::from_secs_f64(1.0 / f64::from(scene.frames_per_second));
            while !stop.load(Ordering::Acquire) {
                let frame_started = Instant::now();
                let reactive_pulses = {
                    let mut pulses = pulses.lock().expect("live RGB pulse lock poisoned");
                    pulses.retain(|(_, instant)| instant.elapsed() <= Duration::from_millis(1500));
                    pulses
                        .iter()
                        .map(|(key_id, instant)| ReactivePulse {
                            key_id: key_id.clone(),
                            age_seconds: instant.elapsed().as_secs_f32(),
                        })
                        .collect()
                };
                let input = RenderInput {
                    elapsed_seconds: started.elapsed().as_secs_f32(),
                    reactive_pulses,
                    ..RenderInput::default()
                };
                let result = render_live_rgb(&scene, &layout, &input)
                    .map_err(|error| error.to_string())
                    .and_then(|frame| manager.submit_live_rgb(frame));
                if let Err(error) = result {
                    *last_error.lock().expect("live RGB error lock poisoned") = Some(error);
                    stop.store(true, Ordering::Release);
                    manager.stop_live_rgb();
                    break;
                }
                let remaining = frame_period.saturating_sub(frame_started.elapsed());
                if !remaining.is_zero() {
                    thread::sleep(remaining);
                }
            }
        })
        .map_err(|error| format!("failed to start live RGB renderer: {error}"))?;
    *state
        .live_rgb
        .worker
        .lock()
        .expect("live RGB worker lock poisoned") = Some(worker);
    Ok(())
}

#[tauri::command]
fn stop_live_rgb(state: State<'_, AppState>) {
    stop_live_rgb_controller(&state);
    let _ = save_active_host_mode(&state, "off", None, None);
}

fn stop_live_rgb_controller(state: &AppState) {
    if let Some(stop) = state
        .live_rgb
        .stop
        .lock()
        .expect("live RGB stop lock poisoned")
        .take()
    {
        stop.store(true, Ordering::Release);
    }
    state.manager.stop_live_rgb();
}

#[tauri::command]
fn start_music_lighting(
    state: State<'_, AppState>,
    config: MusicLightingConfig,
) -> Result<(), String> {
    start_music_lighting_internal(&state, config.clone())?;
    save_active_host_mode(&state, "music", None, Some(&config))
}

fn start_music_lighting_internal(
    state: &AppState,
    config: MusicLightingConfig,
) -> Result<(), String> {
    if background::effects_blocked(state) {
        return Err(
            "Live effects are paused. Resume them from the tray or Background settings.".into(),
        );
    }
    let status = state.manager.status();
    let device = status.device.ok_or_else(|| {
        "connect the F108 Pro over USB before starting music lighting".to_string()
    })?;
    if !device.capabilities.contains(&Capability::Music) {
        return Err("music lighting is unavailable on the active connection".into());
    }
    let layout_id = active_layout_id(state)?;
    let layout = state
        .layouts
        .get(&layout_id)
        .cloned()
        .ok_or_else(|| format!("active layout {layout_id} is unavailable"))?;
    config
        .validate(&layout)
        .map_err(|error| error.to_string())?;

    stop_live_rgb_controller(state);
    stop_music_controller(state);
    settle_effect_workers(state);
    let stop = Arc::new(AtomicBool::new(false));
    *state.music.stop.lock().expect("music stop lock poisoned") = Some(stop.clone());
    *state
        .music
        .last_error
        .lock()
        .expect("music error lock poisoned") = None;
    *state
        .music
        .spectrum
        .lock()
        .expect("music spectrum lock poisoned") = SpectrumFrame::default();
    state.music.frames_analyzed.store(0, Ordering::Relaxed);
    state.music.frames_rendered.store(0, Ordering::Relaxed);

    let manager = state.manager.clone();
    let last_error = state.music.last_error.clone();
    let latest_spectrum = state.music.spectrum.clone();
    let frames_analyzed = state.music.frames_analyzed.clone();
    let frames_rendered = state.music.frames_rendered.clone();
    let worker = thread::Builder::new()
        .name("mechkeymesh-music-lighting".into())
        .spawn(move || {
            let (sample_tx, sample_rx) = sync_channel(1);
            let capture_stop = stop.clone();
            let capture_error = last_error.clone();
            let capture_device = config.output_device_id.clone();
            let capture_handle = thread::Builder::new()
                .name("mechkeymesh-wasapi-loopback".into())
                .spawn(move || {
                    if let Err(error) =
                        run_loopback_capture(capture_device, capture_stop.clone(), sample_tx)
                    {
                        *capture_error.lock().expect("music error lock poisoned") = Some(error);
                        capture_stop.store(true, Ordering::Release);
                    }
                });
            if let Err(error) = capture_handle {
                *last_error.lock().expect("music error lock poisoned") =
                    Some(format!("failed to start Windows audio capture: {error}"));
                stop.store(true, Ordering::Release);
                return;
            }

            let mut analyzer = SpectrumAnalyzer::new();
            let started = Instant::now();
            let frame_period = Duration::from_millis(33);
            let mut last_frame = Instant::now()
                .checked_sub(frame_period)
                .unwrap_or_else(Instant::now);
            while !stop.load(Ordering::Acquire) {
                let samples = match sample_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(samples) => samples,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                let Some(spectrum) = analyzer.push_samples(&samples, &config) else {
                    continue;
                };
                frames_analyzed.fetch_add(1, Ordering::Relaxed);
                *latest_spectrum
                    .lock()
                    .expect("music spectrum lock poisoned") = spectrum.clone();
                if last_frame.elapsed() < frame_period {
                    continue;
                }
                let result =
                    render_music(&config, &layout, &spectrum, started.elapsed().as_secs_f32())
                        .map_err(|error| error.to_string())
                        .and_then(|frame| manager.submit_live_rgb(frame));
                if let Err(error) = result {
                    *last_error.lock().expect("music error lock poisoned") = Some(error);
                    stop.store(true, Ordering::Release);
                    manager.stop_live_rgb();
                    break;
                }
                frames_rendered.fetch_add(1, Ordering::Relaxed);
                last_frame = Instant::now();
            }
            stop.store(true, Ordering::Release);
            if let Ok(capture_handle) = capture_handle {
                let _ = capture_handle.join();
            }
        })
        .map_err(|error| format!("failed to start music lighting: {error}"))?;
    *state
        .music
        .worker
        .lock()
        .expect("music worker lock poisoned") = Some(worker);
    Ok(())
}

#[tauri::command]
fn stop_music_lighting(state: State<'_, AppState>) {
    stop_music_controller(&state);
    state.manager.stop_live_rgb();
    let _ = save_active_host_mode(&state, "off", None, None);
}

fn stop_music_controller(state: &AppState) {
    if let Some(stop) = state
        .music
        .stop
        .lock()
        .expect("music stop lock poisoned")
        .take()
    {
        stop.store(true, Ordering::Release);
    }
}

fn settle_effect_workers(state: &AppState) {
    // A stopped renderer cannot race its replacement or retain a WASAPI capture.
    // Not called from the Windows power callback, where only cancellation is signalled.
    for worker in [&state.live_rgb.worker, &state.music.worker] {
        if let Some(handle) = worker.lock().expect("effect worker lock poisoned").take() {
            let _ = handle.join();
        }
    }
}

fn save_active_host_mode(
    state: &AppState,
    mode: &str,
    live_scene: Option<&LiveRgbScene>,
    music_config: Option<&MusicLightingConfig>,
) -> Result<(), String> {
    let profile_id = state
        .profiles
        .active_profile_id
        .lock()
        .expect("active profile lock poisoned")
        .clone();
    let mut host = state
        .persistence
        .load_profile_host_state(&state.primary_device_model_id, &profile_id)
        .map_err(|error| error.to_string())?;
    host.host_mode = mode.into();
    if let Some(scene) = live_scene {
        host.live_scene_json =
            Some(serde_json::to_string(scene).map_err(|error| error.to_string())?);
    }
    if let Some(config) = music_config {
        host.music_config_json =
            Some(serde_json::to_string(config).map_err(|error| error.to_string())?);
    }
    state
        .persistence
        .save_profile_host_state(&state.primary_device_model_id, &profile_id, &host)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_live_rgb_key(state: State<'_, AppState>, key_id: String) -> Result<(), String> {
    let layout_id = active_layout_id(&state)?;
    let layout = state
        .layouts
        .get(&layout_id)
        .ok_or_else(|| format!("active layout {layout_id} is unavailable"))?;
    if layout.key(&key_id).is_none() {
        return Err(format!("layout has no key {key_id}"));
    }
    state
        .live_rgb
        .pulses
        .lock()
        .expect("live RGB pulse lock poisoned")
        .push((key_id, Instant::now()));
    Ok(())
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
fn get_macros(state: State<'_, AppState>) -> Result<Vec<MacroProgram>, String> {
    let scope = configuration_scope(&state, Capability::Macros)?;
    state
        .persistence
        .load_macros(&scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_macro(state: State<'_, AppState>, program: MacroProgram) -> Result<MacroProgram, String> {
    program.validate().map_err(|error| error.to_string())?;
    let scope = configuration_scope(&state, Capability::Macros)?;
    state
        .persistence
        .save_macro(&scope, &program)
        .map_err(|error| error.to_string())?;
    Ok(program)
}

#[tauri::command]
fn delete_macro(state: State<'_, AppState>, macro_id: String) -> Result<bool, String> {
    let scope = configuration_scope(&state, Capability::Macros)?;
    state
        .persistence
        .delete_macro(&scope, &macro_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn simulate_macro(program: MacroProgram) -> Result<MacroSimulation, String> {
    program.simulation().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_macro_bank_descriptor(state: State<'_, AppState>) -> Result<MacroBankDescriptor, String> {
    require_active_capability(&state, Capability::Macros)?;
    state
        .primary_driver
        .macro_bank_descriptor()
        .ok_or_else(|| "active driver does not support hardware macros".to_string())
}

#[tauri::command]
fn get_device_settings_descriptor(
    state: State<'_, AppState>,
) -> Result<DeviceSettingsDescriptor, String> {
    state
        .primary_driver
        .device_settings_descriptor()
        .ok_or_else(|| "active driver does not support device settings".to_string())
}

#[tauri::command]
fn get_desired_device_settings(
    state: State<'_, AppState>,
) -> Result<Option<DeviceSettings>, String> {
    let scope = configuration_scope(&state, Capability::DeviceSettings)?;
    state
        .persistence
        .load_device_settings(&scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_device_settings(
    state: State<'_, AppState>,
    settings: DeviceSettings,
) -> Result<DeviceSettingsApplyResult, String> {
    state
        .primary_driver
        .validate_device_settings(&settings)
        .map_err(|error| error.to_string())?;
    let scope = configuration_scope(&state, Capability::DeviceSettings)?;
    state
        .persistence
        .save_device_settings(&scope, &settings)
        .map_err(|error| error.to_string())?;

    match state.manager.set_device_settings(settings) {
        Ok(()) => Ok(DeviceSettingsApplyResult {
            stored: true,
            applied: true,
            message: None,
        }),
        Err(error) => Ok(DeviceSettingsApplyResult {
            stored: true,
            applied: false,
            message: Some(error),
        }),
    }
}

#[tauri::command]
fn upload_macros(state: State<'_, AppState>) -> Result<MacroBankApplyResult, String> {
    require_active_capability(&state, Capability::Macros)?;
    let programs = load_hardware_macros(&state)?;
    state
        .primary_driver
        .validate_macro_bank(&programs)
        .map_err(|error| error.to_string())?;
    let event_count = programs.iter().map(|program| program.events.len()).sum();

    match state.manager.set_macros(programs.clone()) {
        Ok(()) => Ok(MacroBankApplyResult {
            applied: true,
            macro_count: programs.len(),
            event_count,
            message: None,
        }),
        Err(error) => Ok(MacroBankApplyResult {
            applied: false,
            macro_count: programs.len(),
            event_count,
            message: Some(error),
        }),
    }
}

#[tauri::command]
fn get_per_key_rgb_capabilities(
    state: State<'_, AppState>,
) -> Result<PerKeyRgbCapabilities, String> {
    require_active_capability(&state, Capability::PerKeyRgb)?;
    let mut capabilities = state
        .primary_driver
        .per_key_rgb_capabilities()
        .ok_or_else(|| "active driver does not support stored per-key RGB".to_string())?;
    if state
        .manager
        .status()
        .device
        .is_some_and(|device| device.connection == mechkeymesh_driver_api::Connection::Wireless24G)
    {
        capabilities.readback_supported = false;
    }
    Ok(capabilities)
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
fn read_per_key_rgb(state: State<'_, AppState>) -> Result<PerKeyRgbHardwareReadback, String> {
    require_active_capability(&state, Capability::PerKeyRgb)?;
    if state
        .manager
        .status()
        .device
        .is_some_and(|device| device.connection == mechkeymesh_driver_api::Connection::Wireless24G)
    {
        return Err(
            "stored RGB readback is not supported over the verified 2.4 GHz protocol".into(),
        );
    }
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
    let macro_slots: BTreeMap<String, u8> =
        load_hardware_macros_for_profile(state, &desired.scope.profile_id)?
            .into_iter()
            .filter(|program| !program.events.is_empty())
            .map(|program| (program.id, program.slot))
            .collect();
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
        let action = match &mapping.action {
            KeyAction::Macro { macro_id, .. } => {
                let slot = macro_slots.get(macro_id).copied().ok_or_else(|| {
                    format!("macro assignment references missing macro: {macro_id}")
                })?;
                KeyAction::Macro {
                    macro_id: macro_id.clone(),
                    hardware_slot: Some(slot),
                }
            }
            action => action.clone(),
        };
        assignments.push(KeyAssignment {
            device_key_index,
            action,
        });
    }
    Ok(assignments)
}

fn load_hardware_macros(state: &AppState) -> Result<Vec<HardwareMacroProgram>, String> {
    let scope = configuration_scope(state, Capability::Macros)?;
    load_hardware_macros_for_scope(state, &scope)
}

fn load_hardware_macros_for_profile(
    state: &AppState,
    profile_id: &str,
) -> Result<Vec<HardwareMacroProgram>, String> {
    let scope = ConfigurationScope::new(
        state.primary_device_model_id.clone(),
        profile_id,
        Capability::Macros,
    );
    load_hardware_macros_for_scope(state, &scope)
}

fn load_hardware_macros_for_scope(
    state: &AppState,
    scope: &ConfigurationScope,
) -> Result<Vec<HardwareMacroProgram>, String> {
    let programs = state
        .persistence
        .load_macros(scope)
        .map_err(|error| error.to_string())?;
    let slots: BTreeMap<String, Option<u16>> = state
        .persistence
        .load_macro_hardware_slots(scope)
        .map_err(|error| error.to_string())?
        .into_iter()
        .collect();
    let mut hardware = programs
        .into_iter()
        .map(|program| {
            let slot = slots.get(&program.id).copied().flatten().ok_or_else(|| {
                format!(
                    "macro {} has no free F108 hardware slot; the device supports 100 slots",
                    program.name
                )
            })?;
            let slot = u8::try_from(slot)
                .map_err(|_| format!("macro {} has an invalid hardware slot", program.name))?;
            to_hardware_macro(program, slot)
        })
        .collect::<Result<Vec<_>, _>>()?;
    hardware.sort_by_key(|program| program.slot);
    Ok(hardware)
}

fn to_hardware_macro(program: MacroProgram, slot: u8) -> Result<HardwareMacroProgram, String> {
    if program.playback != MacroPlayback::Once {
        return Err(format!(
            "macro {} uses a playback mode that is not verified for F108 hardware",
            program.name
        ));
    }
    let events = program
        .events
        .into_iter()
        .map(|event| match event {
            MacroEvent::KeyDown { usage } => HardwareMacroEvent::KeyDown { usage },
            MacroEvent::KeyUp { usage } => HardwareMacroEvent::KeyUp { usage },
            MacroEvent::MouseDown { button } => HardwareMacroEvent::MouseDown {
                button: to_hardware_mouse_button(button),
            },
            MacroEvent::MouseUp { button } => HardwareMacroEvent::MouseUp {
                button: to_hardware_mouse_button(button),
            },
            MacroEvent::Delay { milliseconds } => HardwareMacroEvent::Delay { milliseconds },
        })
        .collect();
    Ok(HardwareMacroProgram {
        id: program.id,
        slot,
        events,
    })
}

const fn to_hardware_mouse_button(button: MacroMouseButton) -> HardwareMacroMouseButton {
    match button {
        MacroMouseButton::Left => HardwareMacroMouseButton::Left,
        MacroMouseButton::Right => HardwareMacroMouseButton::Right,
        MacroMouseButton::Middle => HardwareMacroMouseButton::Middle,
        MacroMouseButton::Button4 => HardwareMacroMouseButton::Button4,
        MacroMouseButton::Button5 => HardwareMacroMouseButton::Button5,
    }
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

    let profile_id = state
        .profiles
        .active_profile_id
        .lock()
        .expect("active profile lock poisoned")
        .clone();

    Ok(ConfigurationScope::new(
        device_model_id,
        profile_id,
        capability,
    ))
}

fn require_active_capability(state: &AppState, capability: Capability) -> Result<(), String> {
    let Some(device) = state.manager.status().device else {
        return Ok(());
    };
    if device.capabilities.contains(&capability) {
        Ok(())
    } else {
        Err(format!(
            "{} is not supported over the active {:?} connection",
            capability.storage_key(),
            device.connection
        ))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            background::show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .on_window_event(background::window_event)
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
            let persistence = Arc::new(PersistenceStore::open(
                data_dir.join("mechkeymesh.sqlite3"),
            )?);

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
            assert!(
                primary_driver.macro_bank_descriptor().is_some(),
                "the R10 F108 driver must expose hardware macro limits"
            );
            assert!(
                primary_driver.device_settings_descriptor().is_some(),
                "the R11 F108 driver must expose device settings metadata"
            );

            let manager = DeviceManager::start(registry, ManagerConfig::default());

            let background =
                background::BackgroundController::new(data_dir.join("background.json"));
            let saved_profile_id = background
                .preferences
                .lock()
                .expect("preferences lock poisoned")
                .active_profile_id
                .clone();
            let initial_profile_id = if persistence
                .list_profiles(&primary_device_model_id)?
                .iter()
                .any(|profile| profile.id == saved_profile_id)
            {
                saved_profile_id
            } else {
                "default".into()
            };

            app.manage(AppState {
                manager,
                persistence,
                layouts,
                driver_layout_ids,
                primary_driver,
                lighting_effects,
                primary_device_model_id,
                primary_layout_id,
                live_rgb: LiveRgbController::default(),
                music: MusicController::default(),
                lcd: Mutex::new(None),
                profiles: ProfileController {
                    active_profile_id: Mutex::new(initial_profile_id),
                    ..ProfileController::default()
                },
                background,
            });
            background::setup(app.handle())?;
            power::register(app.handle())?;
            monitor_profiles(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            native_observations::get_native_observations,
            background::get_background_status,
            background::save_background_preferences,
            background::pause_background_effects,
            import_lcd_media,
            get_lcd_editor_state,
            select_lcd_media_file,
            get_lcd_frame_preview,
            set_lcd_frame_duration,
            duplicate_lcd_frame,
            delete_lcd_frame,
            reorder_lcd_frame,
            draw_lcd_brush,
            erase_lcd,
            add_lcd_text,
            crop_lcd_frame,
            undo_lcd_edit,
            redo_lcd_edit,
            export_lcd_asset,
            get_lcd_transfer_status,
            upload_lcd_asset,
            sync_lcd_clock,
            get_profiles,
            create_profile,
            rename_profile,
            delete_profile,
            get_application_rules,
            select_profile_application_file,
            save_application_rule,
            delete_application_rule,
            get_profile_runtime_status,
            set_profile_auto_switch,
            activate_profile,
            assign_lcd_asset_to_profile,
            get_live_rgb_defaults,
            get_live_rgb_capabilities,
            get_live_rgb_status,
            get_music_defaults,
            get_audio_output_devices,
            get_music_status,
            start_live_rgb,
            stop_live_rgb,
            preview_live_rgb_key,
            start_music_lighting,
            stop_music_lighting,
            get_logs,
            get_driver_registry,
            get_persistence_status,
            get_desired_lighting,
            get_desired_per_key_rgb,
            get_macros,
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
            save_macro,
            delete_macro,
            simulate_macro,
            get_macro_bank_descriptor,
            upload_macros,
            get_device_settings_descriptor,
            get_desired_device_settings,
            save_device_settings,
            save_keymap,
            reset_key_assignments
        ])
        .build(tauri::generate_context!())
        .expect("error while building MechKeyMesh")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let state = app_handle.state::<AppState>();
                state.background.exiting.store(true, Ordering::Release);
                power::unregister(app_handle);
                state.manager.cancel_active_operation();
                if let Some(stop) = state
                    .live_rgb
                    .stop
                    .lock()
                    .expect("live RGB stop lock poisoned")
                    .take()
                {
                    stop.store(true, Ordering::Release);
                }
                stop_music_controller(&state);
                settle_effect_workers(&state);
                state.manager.shutdown();
            }
        });
}
