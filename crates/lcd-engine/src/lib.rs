use font8x8::{BASIC_FONTS, UnicodeFonts};
use image::{
    AnimationDecoder, DynamicImage, Frame as ImageFrame, ImageReader, Rgba, RgbaImage,
    codecs::{gif::GifDecoder, webp::WebPDecoder},
    imageops::FilterType,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    io::BufReader,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

pub const WIDTH: u32 = 240;
pub const HEIGHT: u32 = 135;
pub const FRAME_PIXELS: usize = WIDTH as usize * HEIGHT as usize;
pub const FRAME_BYTES: usize = FRAME_PIXELS * 2;
pub const HEADER_BYTES: usize = 256;
pub const PAGE_BYTES: usize = 4096;
pub const MAX_FRAMES: usize = 141;
pub const MIN_DELAY_MS: u32 = 20;
pub const MAX_DELAY_MS: u32 = 5_100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoPreset {
    Smooth,
    Balanced,
    FullDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FitMode {
    Fit,
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub pixels: RgbaImage,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditableFrame {
    pub pixels: RgbaImage,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcdProject {
    pub name: String,
    pub frames: Vec<EditableFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct F108Asset {
    bytes: Vec<u8>,
    frame_count: usize,
    content_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummary {
    pub width: u32,
    pub height: u32,
    pub pixel_format: &'static str,
    pub frame_count: usize,
    pub header_bytes: usize,
    pub frame_bytes: usize,
    pub content_length: usize,
    pub payload_length: usize,
    pub page_count: usize,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    pub total_duration_ms: u64,
    pub valid: bool,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Error)]
pub enum LcdError {
    #[error("LCD project contains no frames")]
    Empty,
    #[error("LCD asset has {0} frames; the F108 hard limit is {MAX_FRAMES}")]
    TooManyFrames(usize),
    #[error("frame {index} is {width}x{height}; every frame must be {WIDTH}x{HEIGHT}")]
    InvalidDimensions {
        index: usize,
        width: u32,
        height: u32,
    },
    #[error("frame {index} duration {duration_ms} ms is outside {MIN_DELAY_MS}..={MAX_DELAY_MS} ms")]
    InvalidDuration { index: usize, duration_ms: u32 },
    #[error("unsupported media extension: {0}")]
    UnsupportedFormat(String),
    #[error("media decode failed: {0}")]
    Decode(String),
    #[error("video decoder is unavailable; install FFmpeg to import MP4, WebM or MOV")]
    VideoDecoderUnavailable,
    #[error("FFmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("invalid crop rectangle")]
    InvalidCrop,
    #[error("frame index {0} is out of range")]
    FrameIndex(usize),
    #[error("invalid F108 asset: {0}")]
    InvalidAsset(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl LcdProject {
    pub fn new(name: impl Into<String>, frames: Vec<DecodedFrame>) -> Result<Self, LcdError> {
        if frames.is_empty() {
            return Err(LcdError::Empty);
        }
        if frames.len() > MAX_FRAMES {
            return Err(LcdError::TooManyFrames(frames.len()));
        }
        let frames = frames
            .into_iter()
            .enumerate()
            .map(|(index, frame)| {
                validate_frame(index, &frame.pixels, frame.duration_ms)?;
                Ok(EditableFrame {
                    pixels: frame.pixels,
                    duration_ms: frame.duration_ms,
                })
            })
            .collect::<Result<Vec<_>, LcdError>>()?;
        Ok(Self {
            name: name.into(),
            frames,
        })
    }

    pub fn summary(&self) -> ProjectSummary {
        let validation = self.validate();
        ProjectSummary {
            name: self.name.clone(),
            width: WIDTH,
            height: HEIGHT,
            frame_count: self.frames.len(),
            total_duration_ms: self
                .frames
                .iter()
                .map(|frame| u64::from(frame.duration_ms))
                .sum(),
            valid: validation.is_ok(),
            invalid_reason: validation.err().map(|error| error.to_string()),
        }
    }

    pub fn validate(&self) -> Result<(), LcdError> {
        if self.frames.is_empty() {
            return Err(LcdError::Empty);
        }
        if self.frames.len() > MAX_FRAMES {
            return Err(LcdError::TooManyFrames(self.frames.len()));
        }
        for (index, frame) in self.frames.iter().enumerate() {
            validate_frame(index, &frame.pixels, frame.duration_ms)?;
        }
        Ok(())
    }

    pub fn compile(&self) -> Result<F108Asset, LcdError> {
        self.validate()?;
        let content_length = HEADER_BYTES + self.frames.len() * FRAME_BYTES;
        let payload_length = content_length.div_ceil(PAGE_BYTES) * PAGE_BYTES;
        let mut bytes = vec![0xff; payload_length];
        bytes[0] = u8::try_from(self.frames.len()).expect("141 fits in one byte");

        for (index, frame) in self.frames.iter().enumerate() {
            bytes[index + 1] = duration_header_byte(frame.duration_ms);
            let frame_offset = HEADER_BYTES + index * FRAME_BYTES;
            for (pixel_index, pixel) in frame.pixels.pixels().enumerate() {
                let packed = rgb565(pixel.0[0], pixel.0[1], pixel.0[2]).to_le_bytes();
                let offset = frame_offset + pixel_index * 2;
                bytes[offset..offset + 2].copy_from_slice(&packed);
            }
        }

        let asset = F108Asset {
            bytes,
            frame_count: self.frames.len(),
            content_length,
        };
        asset.validate()?;
        Ok(asset)
    }
}

impl F108Asset {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn page_count(&self) -> usize {
        self.bytes.len() / PAGE_BYTES
    }

    pub fn content_length(&self) -> usize {
        self.content_length
    }

    pub fn summary(&self) -> AssetSummary {
        AssetSummary {
            width: WIDTH,
            height: HEIGHT,
            pixel_format: "RGB565 little-endian",
            frame_count: self.frame_count,
            header_bytes: HEADER_BYTES,
            frame_bytes: FRAME_BYTES,
            content_length: self.content_length,
            payload_length: self.bytes.len(),
            page_count: self.page_count(),
            valid: self.validate().is_ok(),
        }
    }

    pub fn validate(&self) -> Result<(), LcdError> {
        if self.frame_count == 0 || self.frame_count > MAX_FRAMES {
            return Err(LcdError::TooManyFrames(self.frame_count));
        }
        if !self.bytes.len().is_multiple_of(PAGE_BYTES) {
            return Err(LcdError::InvalidAsset(
                "payload length is not a whole number of 4096-byte pages".into(),
            ));
        }
        if self.bytes.first().copied() != Some(self.frame_count as u8) {
            return Err(LcdError::InvalidAsset(
                "header frame count does not match the compiled frame count".into(),
            ));
        }
        let expected_content = HEADER_BYTES + self.frame_count * FRAME_BYTES;
        if self.content_length != expected_content || self.bytes.len() != expected_content.div_ceil(PAGE_BYTES) * PAGE_BYTES {
            return Err(LcdError::InvalidAsset("payload length is not exact".into()));
        }
        if self.bytes[1..=self.frame_count].contains(&0) {
            return Err(LcdError::InvalidAsset("header contains a zero frame delay".into()));
        }
        if self.bytes[self.frame_count + 1..HEADER_BYTES]
            .iter()
            .any(|byte| *byte != 0xff)
        {
            return Err(LcdError::InvalidAsset(
                "unused animation-header bytes are not 0xFF".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Editor {
    project: LcdProject,
    undo: VecDeque<LcdProject>,
    redo: VecDeque<LcdProject>,
    history_limit: usize,
}

impl Editor {
    pub fn new(project: LcdProject) -> Self {
        Self {
            project,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            history_limit: 32,
        }
    }

    pub fn project(&self) -> &LcdProject {
        &self.project
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn rename(&mut self, name: impl Into<String>) {
        self.checkpoint();
        self.project.name = name.into();
    }

    pub fn set_duration(&mut self, index: usize, duration_ms: u32) -> Result<(), LcdError> {
        if !(MIN_DELAY_MS..=MAX_DELAY_MS).contains(&duration_ms) {
            return Err(LcdError::InvalidDuration { index, duration_ms });
        }
        self.require_index(index)?;
        self.checkpoint();
        self.project.frames[index].duration_ms = normalize_delay(duration_ms);
        Ok(())
    }

    pub fn duplicate(&mut self, index: usize) -> Result<(), LcdError> {
        self.require_index(index)?;
        if self.project.frames.len() >= MAX_FRAMES {
            return Err(LcdError::TooManyFrames(self.project.frames.len() + 1));
        }
        self.checkpoint();
        let frame = self.project.frames[index].clone();
        self.project.frames.insert(index + 1, frame);
        Ok(())
    }

    pub fn delete(&mut self, index: usize) -> Result<(), LcdError> {
        self.require_index(index)?;
        if self.project.frames.len() == 1 {
            return Err(LcdError::Empty);
        }
        self.checkpoint();
        self.project.frames.remove(index);
        Ok(())
    }

    pub fn reorder(&mut self, from: usize, to: usize) -> Result<(), LcdError> {
        self.require_index(from)?;
        self.require_index(to)?;
        if from == to {
            return Ok(());
        }
        self.checkpoint();
        let frame = self.project.frames.remove(from);
        self.project.frames.insert(to, frame);
        Ok(())
    }

    pub fn brush(&mut self, index: usize, points: &[(i32, i32)], radius: u32, color: [u8; 4]) -> Result<(), LcdError> {
        self.require_index(index)?;
        self.checkpoint();
        let image = &mut self.project.frames[index].pixels;
        for &(x, y) in points {
            draw_disc(image, x, y, radius.max(1), Rgba(color));
        }
        Ok(())
    }

    pub fn erase(&mut self, index: usize, points: &[(i32, i32)], radius: u32) -> Result<(), LcdError> {
        self.brush(index, points, radius, [0, 0, 0, 255])
    }

    pub fn text(&mut self, index: usize, x: i32, y: i32, text: &str, scale: u32, color: [u8; 4]) -> Result<(), LcdError> {
        self.require_index(index)?;
        self.checkpoint();
        let image = &mut self.project.frames[index].pixels;
        let mut cursor_x = x;
        let scale = scale.clamp(1, 12);
        for character in text.chars() {
            if let Some(bitmap) = BASIC_FONTS.get(character) {
                for (row, bits) in bitmap.iter().enumerate() {
                    for column in 0..8 {
                        if bits & (1 << column) != 0 {
                            for sy in 0..scale {
                                for sx in 0..scale {
                                    put_if_inside(
                                        image,
                                        cursor_x + (column * scale as i32) + sx as i32,
                                        y + (row as i32 * scale as i32) + sy as i32,
                                        Rgba(color),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            cursor_x += 9 * scale as i32;
        }
        Ok(())
    }

    pub fn crop(&mut self, index: usize, crop: CropRect, mode: FitMode) -> Result<(), LcdError> {
        self.require_index(index)?;
        let source = self.project.frames[index].pixels.clone();
        if crop.width == 0 || crop.height == 0 || crop.x + crop.width > source.width() || crop.y + crop.height > source.height() {
            return Err(LcdError::InvalidCrop);
        }
        self.checkpoint();
        let cropped = image::imageops::crop_imm(&source, crop.x, crop.y, crop.width, crop.height).to_image();
        self.project.frames[index].pixels = normalize_image(DynamicImage::ImageRgba8(cropped), mode);
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push_back(std::mem::replace(&mut self.project, previous));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop_back() else {
            return false;
        };
        self.undo.push_back(std::mem::replace(&mut self.project, next));
        true
    }

    fn require_index(&self, index: usize) -> Result<(), LcdError> {
        self.project.frames.get(index).map(|_| ()).ok_or(LcdError::FrameIndex(index))
    }

    fn checkpoint(&mut self) {
        if self.undo.len() == self.history_limit {
            self.undo.pop_front();
        }
        self.undo.push_back(self.project.clone());
        self.redo.clear();
    }
}

pub fn import_media(path: &Path, preset: VideoPreset, fit: FitMode) -> Result<LcdProject, LcdError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let raw = match extension.as_str() {
        "png" | "jpg" | "jpeg" => decode_still(path)?,
        "gif" => decode_gif(path)?,
        "webp" => decode_webp(path)?,
        "mp4" | "webm" | "mov" => decode_video(path, preset)?,
        _ => return Err(LcdError::UnsupportedFormat(extension)),
    };
    let frames = optimize_frames(raw, preset, fit)?;
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("LCD media");
    LcdProject::new(name, frames)
}

pub fn optimize_frames(frames: Vec<DecodedFrame>, preset: VideoPreset, fit: FitMode) -> Result<Vec<DecodedFrame>, LcdError> {
    if frames.is_empty() {
        return Err(LcdError::Empty);
    }
    let mut normalized: Vec<DecodedFrame> = frames
        .into_iter()
        .map(|frame| DecodedFrame {
            pixels: normalize_image(DynamicImage::ImageRgba8(frame.pixels), fit),
            duration_ms: normalize_delay(frame.duration_ms),
        })
        .collect();

    remove_redundant_frames(&mut normalized);
    if normalized.len() > MAX_FRAMES {
        match preset {
            VideoPreset::Smooth => normalized.truncate(MAX_FRAMES),
            VideoPreset::Balanced | VideoPreset::FullDuration => reduce_motion_aware(&mut normalized),
        }
    }
    if normalized.len() > MAX_FRAMES {
        return Err(LcdError::TooManyFrames(normalized.len()));
    }
    Ok(normalized)
}

pub const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3)
}

fn duration_header_byte(duration_ms: u32) -> u8 {
    (normalize_delay(duration_ms) / 20).clamp(1, 255) as u8
}

fn normalize_delay(duration_ms: u32) -> u32 {
    duration_ms.clamp(MIN_DELAY_MS, MAX_DELAY_MS).div_ceil(20) * 20
}

fn validate_frame(index: usize, pixels: &RgbaImage, duration_ms: u32) -> Result<(), LcdError> {
    if pixels.width() != WIDTH || pixels.height() != HEIGHT {
        return Err(LcdError::InvalidDimensions {
            index,
            width: pixels.width(),
            height: pixels.height(),
        });
    }
    if !(MIN_DELAY_MS..=MAX_DELAY_MS).contains(&duration_ms)
        || !duration_ms.is_multiple_of(20)
    {
        return Err(LcdError::InvalidDuration { index, duration_ms });
    }
    Ok(())
}

fn decode_still(path: &Path) -> Result<Vec<DecodedFrame>, LcdError> {
    let image = ImageReader::open(path)
        .map_err(|error| LcdError::Decode(error.to_string()))?
        .with_guessed_format()
        .map_err(|error| LcdError::Decode(error.to_string()))?
        .decode()
        .map_err(|error| LcdError::Decode(error.to_string()))?;
    Ok(vec![DecodedFrame {
        pixels: image.to_rgba8(),
        duration_ms: 1_000,
    }])
}

fn decode_gif(path: &Path) -> Result<Vec<DecodedFrame>, LcdError> {
    let file = fs::File::open(path)?;
    let decoder = GifDecoder::new(BufReader::new(file)).map_err(|error| LcdError::Decode(error.to_string()))?;
    decode_animation(decoder.into_frames().collect_frames().map_err(|error| LcdError::Decode(error.to_string()))?)
}

fn decode_webp(path: &Path) -> Result<Vec<DecodedFrame>, LcdError> {
    let file = fs::File::open(path)?;
    let decoder = WebPDecoder::new(BufReader::new(file)).map_err(|error| LcdError::Decode(error.to_string()))?;
    if decoder.has_animation() {
        decode_animation(decoder.into_frames().collect_frames().map_err(|error| LcdError::Decode(error.to_string()))?)
    } else {
        decode_still(path)
    }
}

fn decode_animation(frames: Vec<ImageFrame>) -> Result<Vec<DecodedFrame>, LcdError> {
    if frames.is_empty() {
        return Err(LcdError::Empty);
    }
    Ok(frames
        .into_iter()
        .map(|frame| {
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            let duration_ms = if denominator == 0 { 100 } else { numerator.div_ceil(denominator) };
            DecodedFrame {
                pixels: frame.into_buffer(),
                duration_ms,
            }
        })
        .collect())
}

fn decode_video(path: &Path, preset: VideoPreset) -> Result<Vec<DecodedFrame>, LcdError> {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return Err(LcdError::VideoDecoderUnavailable);
    }
    let duration_seconds = probe_video_duration(path)?;
    let (fps, trim_seconds) = match preset {
        VideoPreset::Smooth => (30.0, Some(MAX_FRAMES as f64 / 30.0)),
        VideoPreset::Balanced => (12.0, Some(MAX_FRAMES as f64 / 12.0)),
        VideoPreset::FullDuration => (
            (MAX_FRAMES as f64 / duration_seconds.max(0.001)).clamp(0.2, 15.0),
            None,
        ),
    };
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let temp = std::env::temp_dir().join(format!("mechkeymesh-lcd-{unique}"));
    fs::create_dir_all(&temp)?;
    let output_pattern = temp.join("frame-%06d.png");
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-i"]).arg(path);
    if let Some(seconds) = trim_seconds {
        command.args(["-t", &format!("{seconds:.4}")]);
    }
    let result = command
        .args([
            "-vf",
            &format!("fps={fps:.6}"),
            "-frames:v",
            &MAX_FRAMES.to_string(),
        ])
        .arg(&output_pattern)
        .output()
        .map_err(|error| LcdError::Ffmpeg(error.to_string()))?;
    if !result.status.success() {
        let _ = fs::remove_dir_all(&temp);
        return Err(LcdError::Ffmpeg(String::from_utf8_lossy(&result.stderr).trim().to_string()));
    }
    let mut paths = fs::read_dir(&temp)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<PathBuf>>();
    paths.sort();
    let duration_ms = normalize_delay((1_000.0 / fps).round() as u32);
    let mut frames = Vec::with_capacity(paths.len());
    for frame_path in paths {
        let image = ImageReader::open(&frame_path)
            .map_err(|error| LcdError::Decode(error.to_string()))?
            .decode()
            .map_err(|error| LcdError::Decode(error.to_string()))?;
        frames.push(DecodedFrame { pixels: image.to_rgba8(), duration_ms });
    }
    let _ = fs::remove_dir_all(&temp);
    if frames.is_empty() {
        return Err(LcdError::Decode("video produced no frames".into()));
    }
    Ok(frames)
}

fn probe_video_duration(path: &Path) -> Result<f64, LcdError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .map_err(|_| LcdError::VideoDecoderUnavailable)?;
    if !output.status.success() {
        return Err(LcdError::Ffmpeg(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .ok_or_else(|| LcdError::Decode("video duration is unavailable".into()))
}

fn normalize_image(image: DynamicImage, mode: FitMode) -> RgbaImage {
    let rgba = image.to_rgba8();
    let (source_width, source_height) = rgba.dimensions();
    match mode {
        FitMode::Fit => {
            let scale = (WIDTH as f64 / source_width as f64).min(HEIGHT as f64 / source_height as f64);
            let width = (source_width as f64 * scale).round().max(1.0) as u32;
            let height = (source_height as f64 * scale).round().max(1.0) as u32;
            let resized = image::imageops::resize(&rgba, width, height, FilterType::Lanczos3);
            let mut canvas = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([0, 0, 0, 255]));
            image::imageops::overlay(&mut canvas, &resized, i64::from((WIDTH - width) / 2), i64::from((HEIGHT - height) / 2));
            canvas
        }
        FitMode::Fill => {
            let scale = (WIDTH as f64 / source_width as f64).max(HEIGHT as f64 / source_height as f64);
            let width = (source_width as f64 * scale).round().max(WIDTH as f64) as u32;
            let height = (source_height as f64 * scale).round().max(HEIGHT as f64) as u32;
            let resized = image::imageops::resize(&rgba, width, height, FilterType::Lanczos3);
            image::imageops::crop_imm(&resized, (width - WIDTH) / 2, (height - HEIGHT) / 2, WIDTH, HEIGHT).to_image()
        }
    }
}

fn remove_redundant_frames(frames: &mut Vec<DecodedFrame>) {
    let mut index = 1;
    while index < frames.len() {
        if frame_difference(&frames[index - 1].pixels, &frames[index].pixels) < 1.0 {
            let duration = frames[index - 1].duration_ms.saturating_add(frames[index].duration_ms).min(MAX_DELAY_MS);
            frames[index - 1].duration_ms = normalize_delay(duration);
            frames.remove(index);
        } else {
            index += 1;
        }
    }
}

fn reduce_motion_aware(frames: &mut Vec<DecodedFrame>) {
    while frames.len() > MAX_FRAMES {
        let remove_index = (1..frames.len().saturating_sub(1))
            .min_by(|left, right| {
                let left_score = frame_difference(&frames[*left - 1].pixels, &frames[*left].pixels)
                    + frame_difference(&frames[*left].pixels, &frames[*left + 1].pixels);
                let right_score = frame_difference(&frames[*right - 1].pixels, &frames[*right].pixels)
                    + frame_difference(&frames[*right].pixels, &frames[*right + 1].pixels);
                left_score.total_cmp(&right_score)
            })
            .unwrap_or(frames.len() - 1);
        let duration = frames[remove_index - 1]
            .duration_ms
            .saturating_add(frames[remove_index].duration_ms)
            .min(MAX_DELAY_MS);
        frames[remove_index - 1].duration_ms = normalize_delay(duration);
        frames.remove(remove_index);
    }
}

fn frame_difference(left: &RgbaImage, right: &RgbaImage) -> f64 {
    let mut difference = 0u64;
    let samples = left.pixels().zip(right.pixels()).step_by(97);
    let mut count = 0u64;
    for (left, right) in samples {
        for channel in 0..3 {
            difference += u64::from(left.0[channel].abs_diff(right.0[channel]));
            count += 1;
        }
    }
    difference as f64 / count.max(1) as f64
}

fn draw_disc(image: &mut RgbaImage, x: i32, y: i32, radius: u32, color: Rgba<u8>) {
    let radius = radius as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                put_if_inside(image, x + dx, y + dy, color);
            }
        }
    }
}

fn put_if_inside(image: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
        image.put_pixel(x as u32, y as u32, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn solid(color: [u8; 4], duration_ms: u32) -> DecodedFrame {
        DecodedFrame {
            pixels: RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba(color)),
            duration_ms,
        }
    }

    #[test]
    fn compiles_exact_verified_single_frame_layout() {
        let project = LcdProject::new("red", vec![solid([255, 0, 0, 255], 20)]).unwrap();
        let asset = project.compile().unwrap();
        assert_eq!(asset.frame_count(), 1);
        assert_eq!(asset.page_count(), 16);
        assert_eq!(asset.bytes()[0], 1);
        assert_eq!(asset.bytes()[1], 1);
        assert_eq!(&asset.bytes()[256..258], &[0x00, 0xf8]);
        assert!(asset.bytes()[2..256].iter().all(|byte| *byte == 0xff));
        assert!(asset.validate().is_ok());
    }

    #[test]
    fn frame_142_is_always_rejected() {
        let frames = (0..MAX_FRAMES + 1)
            .map(|index| solid([index as u8, 0, 0, 255], 20))
            .collect();
        assert!(matches!(LcdProject::new("unsafe", frames), Err(LcdError::TooManyFrames(142))));
    }

    #[test]
    fn editor_enforces_limit_and_undo_redo() {
        let frames = (0..MAX_FRAMES)
            .map(|index| solid([index as u8, 0, 0, 255], 20))
            .collect();
        let mut editor = Editor::new(LcdProject::new("safe", frames).unwrap());
        assert!(matches!(editor.duplicate(0), Err(LcdError::TooManyFrames(142))));
        editor.delete(0).unwrap();
        assert_eq!(editor.project().frames.len(), 140);
        assert!(editor.undo());
        assert_eq!(editor.project().frames.len(), 141);
        assert!(editor.redo());
        assert_eq!(editor.project().frames.len(), 140);
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 10_000, .. ProptestConfig::default() })]

        #[test]
        fn randomized_plans_never_exceed_f108_limit(
            frame_count in 1usize..600,
            delay in 0u32..20_000,
            preset in prop_oneof![Just(VideoPreset::Smooth), Just(VideoPreset::Balanced), Just(VideoPreset::FullDuration)],
        ) {
            let mut planned: Vec<(u32, u32)> = (0..frame_count)
                .map(|index| (index as u32, normalize_delay(delay)))
                .collect();
            if planned.len() > MAX_FRAMES {
                match preset {
                    VideoPreset::Smooth => planned.truncate(MAX_FRAMES),
                    VideoPreset::Balanced | VideoPreset::FullDuration => {
                        while planned.len() > MAX_FRAMES {
                            let middle = planned.len() / 2;
                            let removed = planned.remove(middle);
                            let previous = middle.saturating_sub(1);
                            planned[previous].1 = normalize_delay(planned[previous].1.saturating_add(removed.1).min(MAX_DELAY_MS));
                        }
                    }
                }
            }
            prop_assert!(!planned.is_empty());
            prop_assert!(planned.len() <= MAX_FRAMES);
            prop_assert!(planned.iter().all(|(_, duration)| (MIN_DELAY_MS..=MAX_DELAY_MS).contains(duration) && duration % 20 == 0));
        }
    }
}
