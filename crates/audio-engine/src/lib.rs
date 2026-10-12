use mechkeymesh_driver_api::{LiveRgbFrame, PerKeyRgbAssignment, RgbColor};
use mechkeymesh_layout::Layout;
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, atomic::AtomicBool, mpsc::SyncSender};
use thiserror::Error;

pub const CAPTURE_SAMPLE_RATE: u32 = 48_000;
pub const CAPTURE_CHANNELS: usize = 2;
pub const FFT_SIZE: usize = 2_048;
pub const FFT_HOP_SIZE: usize = 1_024;
pub const SPECTRUM_BANDS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MusicMode {
    Spectrum,
    Rainbow,
    ReverseRainbow,
    Amplitude,
    Rhythm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundMode {
    Off,
    Solid,
    Palette,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicLightingConfig {
    pub output_device_id: Option<String>,
    pub sensitivity: f32,
    pub smoothing: f32,
    pub falloff: f32,
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub background_mode: BackgroundMode,
    pub brightness: f32,
    pub palette: Vec<RgbColor>,
    pub mode: MusicMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumFrame {
    pub bands: Vec<f32>,
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub amplitude: f32,
}

impl Default for SpectrumFrame {
    fn default() -> Self {
        Self {
            bands: vec![0.0; SPECTRUM_BANDS],
            bass: 0.0,
            mids: 0.0,
            treble: 0.0,
            amplitude: 0.0,
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum MusicError {
    #[error("sensitivity and frequency gains must be finite and between 0 and 4")]
    InvalidGain,
    #[error("smoothing, falloff, and brightness must be finite and between 0 and 1")]
    InvalidUnitValue,
    #[error("music palette must contain between 1 and 16 colors")]
    InvalidPalette,
    #[error("layout key {0} has no LED index")]
    MissingLedIndex(String),
}

impl MusicLightingConfig {
    pub fn validate(&self, layout: &Layout) -> Result<(), MusicError> {
        for value in [self.sensitivity, self.bass, self.mids, self.treble] {
            if !value.is_finite() || !(0.0..=4.0).contains(&value) {
                return Err(MusicError::InvalidGain);
            }
        }
        for value in [self.smoothing, self.falloff, self.brightness] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(MusicError::InvalidUnitValue);
            }
        }
        if self.palette.is_empty() || self.palette.len() > 16 {
            return Err(MusicError::InvalidPalette);
        }
        for key in &layout.keys {
            if key.led_index.is_none() {
                return Err(MusicError::MissingLedIndex(key.id.clone()));
            }
        }
        Ok(())
    }
}

pub fn default_music_config() -> MusicLightingConfig {
    MusicLightingConfig {
        output_device_id: None,
        sensitivity: 1.25,
        smoothing: 0.45,
        falloff: 0.82,
        bass: 1.15,
        mids: 1.0,
        treble: 1.0,
        background_mode: BackgroundMode::Palette,
        brightness: 0.85,
        palette: vec![
            RgbColor {
                r: 0,
                g: 210,
                b: 255,
            },
            RgbColor {
                r: 117,
                g: 67,
                b: 255,
            },
            RgbColor {
                r: 255,
                g: 45,
                b: 149,
            },
            RgbColor {
                r: 255,
                g: 190,
                b: 0,
            },
        ],
        mode: MusicMode::Spectrum,
    }
}

pub struct SpectrumAnalyzer {
    pending: VecDeque<f32>,
    previous: SpectrumFrame,
    window: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrumAnalyzer {
    pub fn new() -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|index| {
                0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (FFT_SIZE - 1) as f32).cos()
            })
            .collect();
        Self {
            pending: VecDeque::with_capacity(FFT_SIZE * 2),
            previous: SpectrumFrame::default(),
            window,
            fft,
        }
    }

    pub fn push_samples(
        &mut self,
        samples: &[f32],
        config: &MusicLightingConfig,
    ) -> Option<SpectrumFrame> {
        self.pending.extend(
            samples
                .iter()
                .copied()
                .map(|sample| sample.clamp(-1.0, 1.0)),
        );
        if self.pending.len() < FFT_SIZE {
            return None;
        }

        let source: Vec<f32> = self.pending.iter().take(FFT_SIZE).copied().collect();
        self.pending.drain(..FFT_HOP_SIZE);
        let amplitude =
            (source.iter().map(|sample| sample * sample).sum::<f32>() / FFT_SIZE as f32).sqrt();
        let mut bins: Vec<Complex<f32>> = source
            .iter()
            .zip(&self.window)
            .map(|(sample, window)| Complex::new(sample * window, 0.0))
            .collect();
        self.fft.process(&mut bins);

        let mut raw_bands = vec![0.0; SPECTRUM_BANDS];
        let minimum_hz = 20.0_f32;
        let maximum_hz = 16_000.0_f32;
        let ratio = (maximum_hz / minimum_hz).powf(1.0 / SPECTRUM_BANDS as f32);
        for (band, value) in raw_bands.iter_mut().enumerate() {
            let low = minimum_hz * ratio.powf(band as f32);
            let high = minimum_hz * ratio.powf((band + 1) as f32);
            let low_bin =
                ((low * FFT_SIZE as f32 / CAPTURE_SAMPLE_RATE as f32).floor() as usize).max(1);
            let high_bin = ((high * FFT_SIZE as f32 / CAPTURE_SAMPLE_RATE as f32).ceil() as usize)
                .min(FFT_SIZE / 2);
            let count = high_bin.saturating_sub(low_bin).max(1);
            let energy = bins[low_bin..high_bin]
                .iter()
                .map(|bin| bin.norm())
                .sum::<f32>()
                / count as f32;
            *value = (energy * 6.0 / FFT_SIZE as f32 * config.sensitivity)
                .sqrt()
                .clamp(0.0, 1.0);
        }

        let mut next = SpectrumFrame {
            bass: band_range_average(&raw_bands, 0, 6) * config.bass,
            mids: band_range_average(&raw_bands, 6, 12) * config.mids,
            treble: band_range_average(&raw_bands, 12, 16) * config.treble,
            amplitude: (amplitude * 4.0 * config.sensitivity).clamp(0.0, 1.0),
            bands: raw_bands,
        };
        for (index, value) in next.bands.iter_mut().enumerate() {
            *value = smooth_value(
                self.previous.bands[index],
                *value,
                config.smoothing,
                config.falloff,
            );
        }
        next.bass = smooth_value(
            self.previous.bass,
            next.bass.clamp(0.0, 1.0),
            config.smoothing,
            config.falloff,
        );
        next.mids = smooth_value(
            self.previous.mids,
            next.mids.clamp(0.0, 1.0),
            config.smoothing,
            config.falloff,
        );
        next.treble = smooth_value(
            self.previous.treble,
            next.treble.clamp(0.0, 1.0),
            config.smoothing,
            config.falloff,
        );
        next.amplitude = smooth_value(
            self.previous.amplitude,
            next.amplitude,
            config.smoothing,
            config.falloff,
        );
        self.previous = next.clone();
        Some(next)
    }
}

fn band_range_average(bands: &[f32], start: usize, end: usize) -> f32 {
    bands[start..end].iter().sum::<f32>() / (end - start) as f32
}

fn smooth_value(previous: f32, target: f32, smoothing: f32, falloff: f32) -> f32 {
    let released = target.max(previous * falloff);
    (previous * smoothing + released * (1.0 - smoothing)).clamp(0.0, 1.0)
}

pub fn render_music(
    config: &MusicLightingConfig,
    layout: &Layout,
    spectrum: &SpectrumFrame,
    elapsed_seconds: f32,
) -> Result<LiveRgbFrame, MusicError> {
    config.validate(layout)?;
    let width = layout.coordinate_space.width.max(1.0);
    let height = layout.coordinate_space.height.max(1.0);
    let mut colors = Vec::with_capacity(layout.keys.len());
    for key in &layout.keys {
        let nx = ((key.x + key.width / 2.0) / width).clamp(0.0, 1.0);
        let ny = ((key.y + key.height / 2.0) / height).clamp(0.0, 1.0);
        let band_index = ((nx * spectrum.bands.len() as f32) as usize)
            .min(spectrum.bands.len().saturating_sub(1));
        let band = spectrum.bands.get(band_index).copied().unwrap_or(0.0);
        let background = background_color(config, nx);
        let foreground = match config.mode {
            MusicMode::Spectrum => scale(palette_color(&config.palette, nx), band),
            MusicMode::Rainbow => {
                let position = (nx + elapsed_seconds * 0.08).fract();
                scale(palette_color(&config.palette, position), band)
            }
            MusicMode::ReverseRainbow => {
                let position = (1.0 - nx + elapsed_seconds * 0.08).fract();
                scale(palette_color(&config.palette, position), band)
            }
            MusicMode::Amplitude => scale(palette_color(&config.palette, ny), spectrum.amplitude),
            MusicMode::Rhythm => {
                let (level, position) = if nx < 1.0 / 3.0 {
                    (spectrum.bass, 0.0)
                } else if nx < 2.0 / 3.0 {
                    (spectrum.mids, 0.5)
                } else {
                    (spectrum.treble, 1.0)
                };
                let pulse = (level * 1.25 + spectrum.bass * 0.25).clamp(0.0, 1.0);
                scale(palette_color(&config.palette, position), pulse)
            }
        };
        colors.push(PerKeyRgbAssignment {
            led_index: key.led_index.expect("music validation checks LED indices"),
            color: scale(max_color(background, foreground), config.brightness),
        });
    }
    colors.sort_by_key(|assignment| assignment.led_index);
    Ok(LiveRgbFrame { colors })
}

fn background_color(config: &MusicLightingConfig, position: f32) -> RgbColor {
    match config.background_mode {
        BackgroundMode::Off => RgbColor::BLACK,
        BackgroundMode::Solid => scale(config.palette[0], 0.08),
        BackgroundMode::Palette => scale(palette_color(&config.palette, position), 0.06),
    }
}

fn palette_color(palette: &[RgbColor], position: f32) -> RgbColor {
    if palette.len() == 1 {
        return palette[0];
    }
    let scaled = position.clamp(0.0, 1.0) * (palette.len() - 1) as f32;
    let left = scaled.floor() as usize;
    let right = (left + 1).min(palette.len() - 1);
    mix(palette[left], palette[right], scaled.fract())
}

fn max_color(a: RgbColor, b: RgbColor) -> RgbColor {
    RgbColor {
        r: a.r.max(b.r),
        g: a.g.max(b.g),
        b: a.b.max(b.b),
    }
}

fn mix(a: RgbColor, b: RgbColor, amount: f32) -> RgbColor {
    RgbColor {
        r: (f32::from(a.r) + (f32::from(b.r) - f32::from(a.r)) * amount).round() as u8,
        g: (f32::from(a.g) + (f32::from(b.g) - f32::from(a.g)) * amount).round() as u8,
        b: (f32::from(a.b) + (f32::from(b.b) - f32::from(a.b)) * amount).round() as u8,
    }
}

fn scale(color: RgbColor, amount: f32) -> RgbColor {
    mix(RgbColor::BLACK, color, amount.clamp(0.0, 1.0))
}

#[cfg(windows)]
pub fn output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    std::thread::spawn(output_devices_windows)
        .join()
        .map_err(|_| "Windows audio device enumeration panicked".to_string())?
}

#[cfg(not(windows))]
pub fn output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    Ok(Vec::new())
}

#[cfg(windows)]
fn output_devices_windows() -> Result<Vec<AudioOutputDevice>, String> {
    use wasapi::{DeviceEnumerator, Direction, initialize_mta};

    initialize_mta().map_err(|error| error.to_string())?;
    let enumerator = DeviceEnumerator::new().map_err(|error| error.to_string())?;
    let default_id = enumerator
        .get_default_device(&Direction::Render)
        .and_then(|device| device.get_id())
        .ok();
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|error| error.to_string())?;
    let mut devices = Vec::new();
    for device in &collection {
        let device = device.map_err(|error| error.to_string())?;
        let id = device.get_id().map_err(|error| error.to_string())?;
        devices.push(AudioOutputDevice {
            is_default: default_id.as_deref() == Some(id.as_str()),
            name: device
                .get_friendlyname()
                .unwrap_or_else(|_| "Unnamed output device".into()),
            id,
        });
    }
    devices.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(devices)
}

#[cfg(windows)]
pub fn run_loopback_capture(
    output_device_id: Option<String>,
    stop: Arc<AtomicBool>,
    samples: SyncSender<Vec<f32>>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};

    initialize_mta().map_err(|error| error.to_string())?;
    let enumerator = DeviceEnumerator::new().map_err(|error| error.to_string())?;
    let device = match output_device_id {
        Some(id) => enumerator
            .get_device(&id)
            .map_err(|error| format!("selected output device is unavailable: {error}"))?,
        None => enumerator
            .get_default_device(&Direction::Render)
            .map_err(|error| format!("default output device is unavailable: {error}"))?,
    };
    let mut audio_client = device
        .get_iaudioclient()
        .map_err(|error| error.to_string())?;
    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        CAPTURE_SAMPLE_RATE as usize,
        CAPTURE_CHANNELS,
        None,
    );
    let (_, minimum_period) = audio_client
        .get_device_period()
        .map_err(|error| error.to_string())?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: minimum_period,
    };
    audio_client
        .initialize_client(&format, &Direction::Capture, &mode)
        .map_err(|error| error.to_string())?;
    let event = audio_client
        .set_get_eventhandle()
        .map_err(|error| error.to_string())?;
    let capture = audio_client
        .get_audiocaptureclient()
        .map_err(|error| error.to_string())?;
    let mut bytes = VecDeque::new();
    audio_client
        .start_stream()
        .map_err(|error| error.to_string())?;

    while !stop.load(Ordering::Acquire) {
        if event.wait_for_event(100).is_err() {
            continue;
        }
        capture
            .read_from_device_to_deque(&mut bytes)
            .map_err(|error| error.to_string())?;
        let chunk_bytes = FFT_HOP_SIZE * CAPTURE_CHANNELS * size_of::<f32>();
        while bytes.len() >= chunk_bytes {
            let chunk: Vec<u8> = bytes.drain(..chunk_bytes).collect();
            let mono = chunk
                .chunks_exact(CAPTURE_CHANNELS * size_of::<f32>())
                .map(|frame| {
                    let left = f32::from_le_bytes(frame[0..4].try_into().expect("four bytes"));
                    let right = f32::from_le_bytes(frame[4..8].try_into().expect("four bytes"));
                    (left + right) * 0.5
                })
                .collect();
            match samples.try_send(mono) {
                Ok(()) | Err(std::sync::mpsc::TrySendError::Full(_)) => {}
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                    stop.store(true, Ordering::Release);
                }
            }
        }
    }
    audio_client
        .stop_stream()
        .map_err(|error| error.to_string())
}

#[cfg(not(windows))]
pub fn run_loopback_capture(
    _output_device_id: Option<String>,
    _stop: Arc<AtomicBool>,
    _samples: SyncSender<Vec<f32>>,
) -> Result<(), String> {
    Err("Windows output-loopback capture is available only on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_layout::{
        CoordinateSpace, KeyAction, LayoutKind, PhysicalKey, SUPPORTED_LAYOUT_SCHEMA_VERSION,
    };

    fn layout() -> Layout {
        Layout {
            schema_version: SUPPORTED_LAYOUT_SCHEMA_VERSION,
            id: "test.layout".into(),
            device_model_id: "test.device".into(),
            name: "Test".into(),
            kind: LayoutKind::Custom,
            coordinate_space: CoordinateSpace {
                width: 100.0,
                height: 20.0,
            },
            keys: (0..4)
                .map(|index| PhysicalKey {
                    id: format!("key-{index}"),
                    label: index.to_string(),
                    source_label: None,
                    section: None,
                    x: index as f32 * 25.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                    rotation: 0.0,
                    rotation_origin: None,
                    matrix: None,
                    grid_position: None,
                    hid_usage: None,
                    device_key_index: Some(index + 1),
                    led_index: Some(index + 1),
                    fn_programmable: true,
                    default_action: KeyAction::Default,
                })
                .collect(),
            source_notes: Vec::new(),
        }
    }

    fn sine(frequency: f32) -> Vec<f32> {
        (0..FFT_SIZE)
            .map(|index| {
                (std::f32::consts::TAU * frequency * index as f32 / CAPTURE_SAMPLE_RATE as f32)
                    .sin()
                    * 0.8
            })
            .collect()
    }

    #[test]
    fn fft_separates_bass_mids_and_treble() {
        let config = default_music_config();
        let mut bass_analyzer = SpectrumAnalyzer::new();
        let bass = bass_analyzer.push_samples(&sine(100.0), &config).unwrap();
        assert!(bass.bass > bass.treble);

        let mut treble_analyzer = SpectrumAnalyzer::new();
        let treble = treble_analyzer
            .push_samples(&sine(8_000.0), &config)
            .unwrap();
        assert!(treble.treble > treble.bass);
    }

    #[test]
    fn falloff_prevents_an_instant_drop_to_zero() {
        let config = default_music_config();
        let mut analyzer = SpectrumAnalyzer::new();
        analyzer.push_samples(&sine(440.0), &config).unwrap();
        let transition = analyzer
            .push_samples(&vec![0.0; FFT_HOP_SIZE], &config)
            .unwrap();
        let quiet = analyzer
            .push_samples(&vec![0.0; FFT_HOP_SIZE], &config)
            .unwrap();
        assert!(quiet.amplitude > 0.0);
        assert!(quiet.amplitude < transition.amplitude);
    }

    #[test]
    fn every_mode_renders_a_complete_sorted_frame() {
        let layout = layout();
        let spectrum = SpectrumFrame {
            bands: vec![0.5; SPECTRUM_BANDS],
            bass: 0.8,
            mids: 0.5,
            treble: 0.2,
            amplitude: 0.6,
        };
        for mode in [
            MusicMode::Spectrum,
            MusicMode::Rainbow,
            MusicMode::ReverseRainbow,
            MusicMode::Amplitude,
            MusicMode::Rhythm,
        ] {
            let mut config = default_music_config();
            config.mode = mode;
            let frame = render_music(&config, &layout, &spectrum, 1.0).unwrap();
            assert_eq!(frame.colors.len(), layout.keys.len());
            assert_eq!(frame.colors[0].led_index, 1);
        }
    }

    #[test]
    fn rejects_out_of_range_controls_and_empty_palettes() {
        let layout = layout();
        let mut config = default_music_config();
        config.sensitivity = 4.1;
        assert_eq!(config.validate(&layout), Err(MusicError::InvalidGain));

        config = default_music_config();
        config.smoothing = f32::NAN;
        assert_eq!(config.validate(&layout), Err(MusicError::InvalidUnitValue));

        config = default_music_config();
        config.palette.clear();
        assert_eq!(config.validate(&layout), Err(MusicError::InvalidPalette));
    }
}
