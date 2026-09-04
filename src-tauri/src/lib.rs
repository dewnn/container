use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use silero_vad_rust::load_silero_vad;
use std::{
    collections::HashMap,
    fmt::Write as _,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// FFmpeg, FFprobe and taskkill are background workers. On Windows, spawning a
// console program from the GUI without this flag briefly opens a CMD window.
// stdout/stderr remain available, so progress and error reporting are intact.
fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    command
}

fn allow_asset_file(app: &AppHandle, path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("Preview file was not found.".into());
    }
    app.asset_protocol_scope()
        .allow_file(path)
        .map_err(|error| format!("Preview access could not be granted: {error}"))
}

#[derive(Default)]
struct JobState {
    cancelled: AtomicBool,
    pid: Mutex<Option<u32>>,
    vad_cache: Mutex<Option<VadCache>>,
}

struct VadCache {
    path: PathBuf,
    modified: Option<std::time::SystemTime>,
    len: u64,
    scores: Arc<Vec<f32>>,
    samples: Arc<Vec<i16>>,
}

#[derive(Debug, Serialize)]
struct AudioTrackInfo {
    index: u64,
    codec: String,
    channels: Option<u64>,
    channel_layout: Option<String>,
    language: Option<String>,
    bitrate: Option<f64>,
    is_default: bool,
}

#[derive(Debug, Serialize)]
struct MediaInfo {
    path: String,
    name: String,
    kind: String,
    duration: Option<f64>,
    width: Option<u64>,
    height: Option<u64>,
    fps: Option<f64>,
    codec: String,
    audio_codec: Option<String>,
    audio_tracks: Vec<AudioTrackInfo>,
    pixel_format: Option<String>,
    bits_per_raw_sample: Option<u64>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    color_space: Option<String>,
    bitrate: Option<f64>,
    size: u64,
    start_timecode: Option<String>,
}

#[derive(Debug, Serialize)]
struct FfmpegStatus {
    ready: bool,
    ffmpeg_version: Option<String>,
    ffprobe_version: Option<String>,
}

async fn component_version(program: &str) -> Option<String> {
    let output = hidden_command(program)
        .arg("-version")
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
}

#[tauri::command]
async fn ffmpeg_status() -> FfmpegStatus {
    let (ffmpeg_version, ffprobe_version) =
        tokio::join!(component_version("ffmpeg"), component_version("ffprobe"));
    FfmpegStatus {
        ready: ffmpeg_version.is_some() && ffprobe_version.is_some(),
        ffmpeg_version,
        ffprobe_version,
    }
}

#[derive(Debug, Deserialize)]
struct OperationRequest {
    input: String,
    operation: String,
    params: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct TextLayer {
    text: String,
    x: f64,
    y: f64,
    size: f64,
    color: String,
    opacity: f64,
    #[serde(default)]
    font_path: String,
    #[serde(default)]
    outline: f64,
    #[serde(default)]
    outline_color: String,
    #[serde(default)]
    shadow: f64,
    #[serde(default)]
    shadow_color: String,
    #[serde(default)]
    background: bool,
    #[serde(default)]
    background_color: String,
    #[serde(default)]
    background_opacity: f64,
    #[serde(default)]
    background_padding: f64,
}

#[derive(Debug, Clone, Serialize)]
struct FontOption {
    name: String,
    path: String,
}

#[tauri::command]
async fn list_system_fonts(app: AppHandle) -> Result<Vec<FontOption>, String> {
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let system_fonts = PathBuf::from(&windows).join("Fonts");
    let user_fonts =
        dirs::data_local_dir().map(|path| path.join("Microsoft").join("Windows").join("Fonts"));
    let mut fonts = Vec::new();

    #[cfg(target_os = "windows")]
    for key in [
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
        r"HKCU\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
    ] {
        if let Ok(output) = hidden_command("reg").args(["query", key]).output().await {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let Some((raw_name, raw_path)) = line
                    .split_once("REG_EXPAND_SZ")
                    .or_else(|| line.split_once("REG_SZ"))
                else {
                    continue;
                };
                let raw_path = raw_path.trim().replace("%WINDIR%", &windows);
                let supplied = PathBuf::from(&raw_path);
                let path = if supplied.is_absolute() {
                    supplied
                } else {
                    let user_candidate = user_fonts.as_ref().map(|dir| dir.join(&supplied));
                    if user_candidate.as_ref().is_some_and(|path| path.is_file()) {
                        user_candidate.unwrap()
                    } else {
                        system_fonts.join(supplied)
                    }
                };
                if path.is_file() {
                    let name = raw_name
                        .trim()
                        .replace(" (TrueType)", "")
                        .replace(" (OpenType)", "");
                    fonts.push(FontOption {
                        name,
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }

    if fonts.is_empty() {
        for directory in std::iter::once(system_fonts).chain(user_fonts) {
            let Ok(entries) = std::fs::read_dir(directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                if !matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "ttf" | "otf" | "ttc"
                ) {
                    continue;
                }
                fonts.push(FontOption {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
    fonts.sort_by_key(|font| font.name.to_ascii_lowercase());
    fonts.dedup_by(|left, right| left.name.eq_ignore_ascii_case(&right.name));
    for font in &fonts {
        allow_asset_file(&app, Path::new(&font.path))?;
    }
    Ok(fonts)
}

#[derive(Debug, Serialize)]
struct JobResult {
    output: String,
    elapsed: f64,
}

#[derive(Debug, Deserialize)]
struct QualityAnalysisRequest {
    input: String,
    goal: String,
    sample_duration: f64,
}

#[derive(Debug, Clone, Serialize)]
struct QualityCandidate {
    crf: u64,
    vmaf: f64,
    estimated_size_mb: f64,
    rating: String,
}

#[derive(Debug, Serialize)]
struct QualityAnalysis {
    recommended_crf: u64,
    target_vmaf: f64,
    candidates: Vec<QualityCandidate>,
    sample_count: usize,
    sampled_seconds: f64,
    elapsed: f64,
}

#[derive(Clone, Serialize)]
struct ProgressEvent {
    percent: f64,
    time: f64,
    speed: String,
    frame: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeepInterval {
    start: f64,
    end: f64,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct AutoCutRequest {
    input: String,
    analysis_input: Option<String>,
    threshold: f64,
    min_silence: f64,
    min_speech: f64,
    #[serde(default)]
    padding: Option<f64>,
    #[serde(default)]
    minimum_pause: Option<f64>,
    #[serde(default)]
    keep_before_speech: Option<f64>,
    #[serde(default)]
    keep_after_speech: Option<f64>,
    #[serde(default)]
    boundary_refinement: Option<bool>,
}

#[derive(Debug, Serialize)]
struct AutoCutAnalysis {
    cuts: Vec<KeepInterval>,
    waveform: Vec<f32>,
    duration: f64,
    boundary_refinement: bool,
    overlaps_before_normalization: usize,
}

#[derive(Debug, Serialize)]
struct AutoCutRecommendation {
    threshold: f64,
    min_silence: f64,
    min_speech: f64,
    minimum_pause: f64,
    keep_before_speech: f64,
    keep_after_speech: f64,
    noise_floor_db: f64,
    speech_level_db: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct AutoCutPreset {
    id: &'static str,
    minimum_pause: f64,
    keep_before_speech: f64,
    keep_after_speech: f64,
}

const AUTOCUT_PRESETS: [AutoCutPreset; 3] = [
    AutoCutPreset {
        id: "natural",
        minimum_pause: 0.500,
        keep_before_speech: 0.150,
        keep_after_speech: 0.250,
    },
    AutoCutPreset {
        id: "balanced",
        minimum_pause: 0.350,
        keep_before_speech: 0.100,
        keep_after_speech: 0.180,
    },
    AutoCutPreset {
        id: "tight",
        minimum_pause: 0.225,
        keep_before_speech: 0.075,
        keep_after_speech: 0.130,
    },
];

#[derive(Debug, Clone, Copy)]
struct AutoCutEditSettings {
    minimum_pause: f64,
    keep_before_speech: f64,
    keep_after_speech: f64,
    boundary_refinement: bool,
}

#[derive(Debug, Deserialize)]
struct AutoCutExportRequest {
    input: String,
    cuts: Vec<KeepInterval>,
    format: String,
    quality: String,
    resolution: String,
    linked_tracks: Vec<LinkedTrackRequest>,
}

#[derive(Debug, Deserialize)]
struct LinkedTrackRequest {
    path: String,
    offset: f64,
}

fn parse_rate(text: Option<&str>) -> Option<f64> {
    let text = text?;
    let mut pieces = text.split('/');
    let numerator = pieces.next()?.parse::<f64>().ok()?;
    let denominator = pieces.next().unwrap_or("1").parse::<f64>().ok()?;
    (denominator != 0.0).then_some(numerator / denominator)
}

fn parse_number(params: &HashMap<String, String>, key: &str) -> Result<f64, String> {
    params
        .get(key)
        .ok_or_else(|| format!("Missing parameter: {key}"))?
        .parse::<f64>()
        .map_err(|_| format!("Invalid number for {key}"))
}

fn param<'a>(params: &'a HashMap<String, String>, key: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("Missing parameter: {key}"))
}

fn enabled(params: &HashMap<String, String>, key: &str) -> bool {
    params.get(key).is_some_and(|value| value == "true")
}

fn check_range(value: f64, min: f64, max: f64, name: &str) -> Result<f64, String> {
    if value.is_finite() && value >= min && value <= max {
        Ok(value)
    } else {
        Err(format!("{name} must be between {min} and {max}."))
    }
}

fn quality_sample_positions(duration: f64, sample_duration: f64) -> Vec<f64> {
    let last = (duration - sample_duration).max(0.0);
    let raw = if duration <= sample_duration * 1.5 {
        vec![0.0]
    } else if duration <= sample_duration * 4.0 {
        vec![0.0, last]
    } else {
        vec![0.0, last / 2.0, last]
    };
    let mut positions = Vec::new();
    for value in raw {
        if positions
            .iter()
            .all(|existing: &f64| (*existing - value).abs() > 0.05)
        {
            positions.push(value);
        }
    }
    positions
}

fn quality_dimensions(info: &MediaInfo) -> Result<(u64, u64), String> {
    let source_w = info.width.ok_or("Video width is unavailable.")?;
    let source_h = info.height.ok_or("Video height is unavailable.")?;
    if source_w == 0 || source_h == 0 {
        return Err("Video dimensions are invalid.".into());
    }
    let scale = (1280.0 / source_w as f64)
        .min(720.0 / source_h as f64)
        .min(1.0);
    let even = |value: f64| ((value.floor() as u64).max(2) / 2) * 2;
    Ok((even(source_w as f64 * scale), even(source_h as f64 * scale)))
}

fn quality_target(goal: &str) -> Result<f64, String> {
    match goal {
        "high" => Ok(95.0),
        "balanced" => Ok(92.0),
        "small" => Ok(88.0),
        _ => Err("Invalid quality goal.".into()),
    }
}

fn quality_rating(vmaf: f64) -> &'static str {
    if vmaf >= 95.0 {
        "excellent"
    } else if vmaf >= 90.0 {
        "very_good"
    } else if vmaf >= 85.0 {
        "good"
    } else {
        "heavy_loss"
    }
}

fn recommend_quality_crf(candidates: &[QualityCandidate], target: f64) -> Option<u64> {
    candidates
        .iter()
        .filter(|item| item.vmaf >= target)
        .map(|item| item.crf)
        .max()
        .or_else(|| {
            candidates
                .iter()
                .max_by(|a, b| a.vmaf.total_cmp(&b.vmaf))
                .map(|item| item.crf)
        })
}

#[tauri::command]
async fn probe_media(path: String) -> Result<MediaInfo, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err("Input file was not found.".into());
    }
    let output = hidden_command("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(&input)
        .output()
        .await
        .map_err(|error| format!("ffprobe could not start: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let data: Value = serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let streams = data["streams"].as_array().ok_or("No media stream found.")?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video");
    let audio = streams
        .iter()
        .find(|stream| stream["codec_type"] == "audio");
    let audio_tracks = streams
        .iter()
        .filter(|stream| stream["codec_type"] == "audio")
        .map(|stream| AudioTrackInfo {
            index: stream["index"].as_u64().unwrap_or(0),
            codec: stream["codec_name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            channels: stream["channels"].as_u64(),
            channel_layout: stream["channel_layout"].as_str().map(str::to_string),
            language: stream["tags"]["language"].as_str().map(str::to_string),
            bitrate: stream["bit_rate"]
                .as_str()
                .and_then(|value| value.parse().ok()),
            is_default: stream["disposition"]["default"].as_u64() == Some(1),
        })
        .collect();
    let extension = input
        .extension()
        .and_then(|part| part.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let image_extensions = ["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"];
    let kind = if image_extensions.contains(&extension.as_str()) {
        "image"
    } else if video.is_some() {
        "video"
    } else if audio.is_some() {
        "audio"
    } else {
        return Err("Unsupported media file.".into());
    };
    let primary =
        if kind == "audio" { audio } else { video }.ok_or("Primary media stream missing.")?;
    let duration = data["format"]["duration"]
        .as_str()
        .and_then(|value| value.parse().ok())
        .or_else(|| {
            primary["duration"]
                .as_str()
                .and_then(|value| value.parse().ok())
        });
    let bitrate = primary["bit_rate"]
        .as_str()
        .and_then(|value| value.parse().ok())
        .or_else(|| {
            data["format"]["bit_rate"]
                .as_str()
                .and_then(|value| value.parse().ok())
        });
    let size = input.metadata().map_err(|error| error.to_string())?.len();
    let start_timecode = video
        .and_then(|stream| stream["tags"]["timecode"].as_str())
        .or_else(|| data["format"]["tags"]["timecode"].as_str())
        .map(str::to_string);
    Ok(MediaInfo {
        path: input.to_string_lossy().to_string(),
        name: input
            .file_name()
            .and_then(|part| part.to_str())
            .unwrap_or("media")
            .to_string(),
        kind: kind.to_string(),
        duration,
        width: primary["width"].as_u64(),
        height: primary["height"].as_u64(),
        fps: parse_rate(primary["r_frame_rate"].as_str()),
        codec: primary["codec_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        audio_codec: audio
            .and_then(|stream| stream["codec_name"].as_str())
            .map(str::to_string),
        audio_tracks,
        pixel_format: video
            .and_then(|stream| stream["pix_fmt"].as_str())
            .map(str::to_string),
        bits_per_raw_sample: video
            .and_then(|stream| stream["bits_per_raw_sample"].as_str())
            .and_then(|value| value.parse().ok()),
        color_transfer: video
            .and_then(|stream| stream["color_transfer"].as_str())
            .map(str::to_string),
        color_primaries: video
            .and_then(|stream| stream["color_primaries"].as_str())
            .map(str::to_string),
        color_space: video
            .and_then(|stream| stream["color_space"].as_str())
            .map(str::to_string),
        bitrate,
        size,
        start_timecode,
    })
}

#[tauri::command]
async fn available_encoders() -> Vec<String> {
    let candidates = [
        "libx264",
        "h264_amf",
        "h264_nvenc",
        "h264_qsv",
        "libx265",
        "hevc_amf",
        "hevc_nvenc",
        "libvpx-vp9",
        "libsvtav1",
        "av1_nvenc",
        "av1_amf",
        "av1_qsv",
    ];
    let mut checks = tokio::task::JoinSet::new();
    for encoder in candidates {
        checks.spawn(async move {
            let mut command = hidden_command("ffmpeg");
            command.args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:r=1:d=1",
                "-frames:v",
                "1",
                "-c:v",
                encoder,
                "-f",
                "null",
                "-",
            ]);
            // Capability probes are expected to fail for GPUs that are not
            // installed. Keep those normal failures out of the user's console.
            command.stdout(Stdio::null()).stderr(Stdio::null());
            #[cfg(target_os = "windows")]
            command.creation_flags(0x08000000);
            let works = tokio::time::timeout(std::time::Duration::from_secs(8), command.status())
                .await
                .ok()
                .and_then(Result::ok)
                .is_some_and(|status| status.success());
            (encoder.to_string(), works)
        });
    }
    let mut available = Vec::new();
    while let Some(result) = checks.join_next().await {
        if let Ok((encoder, true)) = result {
            available.push(encoder);
        }
    }
    available
}

#[tauri::command]
async fn hash_file(path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let input = PathBuf::from(path);
        if !input.is_file() {
            return Err("Input file was not found.".into());
        }
        let mut file = std::fs::File::open(input).map_err(|error| error.to_string())?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn collect_media_files(
    folder: &Path,
    recursive: bool,
    files: &mut Vec<String>,
) -> Result<(), String> {
    const EXTENSIONS: &[&str] = &[
        "mp4", "mkv", "mov", "avi", "webm", "m4v", "mp3", "wav", "m4a", "aac", "flac", "opus",
        "ogg", "jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff",
    ];
    for entry in std::fs::read_dir(folder).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_media_files(&path, true, files)?;
        } else if path.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| EXTENSIONS.contains(&value.to_ascii_lowercase().as_str()))
        {
            files.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

#[tauri::command]
async fn list_media_files(folder: String, recursive: bool) -> Result<Vec<String>, String> {
    let folder = PathBuf::from(folder);
    if !folder.is_dir() {
        return Err("Folder was not found.".into());
    }
    let mut files = Vec::new();
    collect_media_files(&folder, recursive, &mut files)?;
    files.sort();
    files.dedup();
    Ok(files)
}

fn parse_smpte(value: Option<&str>, fps: f64) -> f64 {
    let Some(text) = value else {
        return 0.0;
    };
    let drop_frame = text.contains(';');
    let parts: Vec<u64> = text
        .replace(';', ":")
        .split(':')
        .filter_map(|v| v.parse().ok())
        .collect();
    if parts.len() != 4 || fps <= 0.0 {
        return 0.0;
    }
    let (hours, minutes, seconds, frames) = (parts[0], parts[1], parts[2], parts[3]);
    if drop_frame && ((fps - 29.97).abs() < 0.1 || (fps - 59.94).abs() < 0.1) {
        let nominal = if fps > 50.0 { 60 } else { 30 };
        let drop = if nominal == 60 { 4 } else { 2 };
        let total_minutes = hours * 60 + minutes;
        let dropped = drop * (total_minutes - total_minutes / 10);
        return (((hours * 3600 + minutes * 60 + seconds) * nominal + frames)
            .saturating_sub(dropped)) as f64
            / fps;
    }
    hours as f64 * 3600.0 + minutes as f64 * 60.0 + seconds as f64 + frames as f64 / fps
}

fn safe_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or("media")
        .chars()
        .map(|character| {
            if "<>:\"/\\|?*".contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect()
}

fn category(operation: &str) -> &str {
    match operation {
        "transform" | "ratio" | "resize" => "transform",
        "upscale" => "upscale",
        "fps" | "interpolation" | "frame_blend" | "dedupe" | "speed" | "cfr" => "motion",
        "compression" | "smart_quality" | "bitrate" | "discord_compressor" | "potatoify" => {
            "quality"
        }
        "text" => "text",
        "color" | "noise" | "negate" | "deep_fry" | "corruption" => "effects",
        "remove_audio" | "extract_audio" | "replace_audio" | "distortion" | "audio_convert" => {
            "audio"
        }
        "image_ratio" | "image_potatoify" => "image",
        "proxy" => "proxy",
        "autocut" | "smartcut" => "smartcut",
        _ => "export",
    }
}

fn scaled_height(info: &MediaInfo, requested_height: u64) -> (u64, u64) {
    let source_width = info.width.unwrap_or(1920).max(2);
    let source_height = info.height.unwrap_or(1080).max(2);
    let target_height = requested_height.min(source_height).max(2) / 2 * 2;
    let target_width =
        (((source_width as f64 * target_height as f64 / source_height as f64).round() as u64) / 2
            * 2)
        .max(2);
    (target_width, target_height)
}

fn unique_output(input: &Path, operation: &str, extension: &str) -> Result<PathBuf, String> {
    let input_parent = input
        .parent()
        .ok_or("Input folder could not be resolved.")?;
    #[cfg(test)]
    let output_root = input_parent.to_path_buf();
    #[cfg(not(test))]
    let output_root = dirs::download_dir().unwrap_or_else(|| input_parent.to_path_buf());
    let folder = output_root
        .join("CONTAINER Output")
        .join(category(operation));
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let base = format!("{}_{}", safe_stem(input), operation);
    let mut output = folder.join(format!("{base}.{extension}"));
    let mut counter = 1;
    while output.exists() {
        output = folder.join(format!("{base} ({counter}).{extension}"));
        counter += 1;
    }
    Ok(output)
}

async fn waveform_for(input: &Path) -> Result<Vec<f32>, String> {
    let output = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", "200", "-f", "f32le", "pipe:1"])
        .output()
        .await
        .map_err(|e| format!("FFmpeg waveform analysis could not start: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let samples: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]).abs().min(1.0))
        .collect();
    if samples.len() <= 1200 {
        return Ok(samples);
    }
    let chunk = (samples.len() as f64 / 1200.0).ceil() as usize;
    Ok(samples
        .chunks(chunk)
        .map(|values| values.iter().copied().fold(0.0_f32, f32::max))
        .collect())
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return -60.0;
    }
    sorted[((sorted.len() - 1) as f64 * fraction.clamp(0.0, 1.0)).round() as usize]
}

fn median(values: &mut [f64], fallback: f64) -> f64 {
    if values.is_empty() {
        return fallback;
    }
    values.sort_by(f64::total_cmp);
    percentile(values, 0.5)
}

fn recommend_from_levels(levels: &[f64], block_seconds: f64) -> AutoCutRecommendation {
    let mut sorted = levels.to_vec();
    sorted.sort_by(f64::total_cmp);
    let noise = percentile(&sorted, 0.20);
    let speech = percentile(&sorted, 0.82);
    let spread = (speech - noise).max(6.0);
    let level_threshold = (noise + spread * 0.42).clamp(-55.0, -18.0).round();
    let active: Vec<bool> = levels
        .iter()
        .map(|level| *level >= level_threshold)
        .collect();
    let mut speech_runs = Vec::new();
    let mut silence_runs = Vec::new();
    let mut index = 0;
    while index < active.len() {
        let state = active[index];
        let start = index;
        while index < active.len() && active[index] == state {
            index += 1;
        }
        let length = (index - start) as f64 * block_seconds;
        if state {
            speech_runs.push(length);
        } else if start > 0 && index < active.len() {
            silence_runs.push(length);
        }
    }
    let typical_gap = median(&mut silence_runs, 0.35);
    let typical_speech = median(&mut speech_runs, 0.4);
    let min_silence = (typical_gap * 0.70).clamp(0.18, 0.80);
    let min_speech = (typical_speech * 0.25).clamp(0.10, 0.30);
    // Auto remains conservative: level analysis may choose detector values,
    // but it never turns a clean noise floor into hyper-aggressive editing.
    let minimum_pause = (typical_gap * 0.80).clamp(0.35, 0.65);
    AutoCutRecommendation {
        threshold: (0.65 - spread * 0.005).clamp(0.35, 0.65),
        min_silence: (min_silence * 100.0).round() / 100.0,
        min_speech: (min_speech * 100.0).round() / 100.0,
        minimum_pause: (minimum_pause * 100.0).round() / 100.0,
        keep_before_speech: 0.10,
        keep_after_speech: 0.18,
        noise_floor_db: (noise * 10.0).round() / 10.0,
        speech_level_db: (speech * 10.0).round() / 10.0,
    }
}

async fn cached_vad_data(
    state: &JobState,
    input: &Path,
) -> Result<(Arc<Vec<f32>>, Arc<Vec<i16>>), String> {
    let metadata = std::fs::metadata(input).map_err(|e| e.to_string())?;
    let path = std::fs::canonicalize(input).unwrap_or_else(|_| input.to_path_buf());
    let modified = metadata.modified().ok();
    if let Some(cache) = state
        .vad_cache
        .lock()
        .map_err(|_| "VAD cache lock failed")?
        .as_ref()
    {
        if cache.path == path && cache.len == metadata.len() && cache.modified == modified {
            return Ok((cache.scores.clone(), cache.samples.clone()));
        }
    }
    let output = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-f", "s16le", "pipe:1"])
        .output()
        .await
        .map_err(|e| format!("FFmpeg VAD audio extraction could not start: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let samples = Arc::new(
        output
            .stdout
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    );
    let samples_for_scoring = samples.clone();
    let scores = Arc::new(
        tokio::task::spawn_blocking(move || -> Result<Vec<f32>, String> {
            let mut detector =
                load_silero_vad().map_err(|e| format!("Silero V6 could not initialize: {e}"))?;
            let mut scores = Vec::with_capacity(samples_for_scoring.len().div_ceil(512));
            for chunk in samples_for_scoring.chunks(512) {
                let mut frame = vec![0.0_f32; 512];
                for (output, sample) in frame.iter_mut().zip(chunk) {
                    *output = *sample as f32 / 32768.0;
                }
                let probability = detector
                    .forward_chunk(&frame, 16_000)
                    .map_err(|e| format!("Silero V6 inference failed: {e}"))?;
                scores.push(probability[[0, 0]]);
            }
            Ok(scores)
        })
        .await
        .map_err(|e| e.to_string())??,
    );
    *state
        .vad_cache
        .lock()
        .map_err(|_| "VAD cache lock failed")? = Some(VadCache {
        path,
        modified,
        len: metadata.len(),
        scores: scores.clone(),
        samples: samples.clone(),
    });
    Ok((scores, samples))
}

#[cfg(test)]
async fn cached_vad_scores(state: &JobState, input: &Path) -> Result<Arc<Vec<f32>>, String> {
    cached_vad_data(state, input)
        .await
        .map(|(scores, _)| scores)
}

fn keeps_from_vad_scores(
    scores: &[f32],
    threshold: f64,
    min_silence: f64,
    min_speech: f64,
    padding: f64,
    duration: f64,
) -> Vec<KeepInterval> {
    const CHUNK_SECONDS: f64 = 512.0 / 16000.0;
    if scores.is_empty() || duration <= 0.0 {
        return Vec::new();
    }

    // Keep this state machine in parity with cobanov/autocut's vad.rs. The
    // model scores are deliberately not smoothed and there is no attack
    // debounce: speech enters on the first score at threshold and exits below
    // threshold - 0.15. min_silence then merges short non-speech runs, and
    // min_speech is applied only after that merge.
    let release = (threshold - 0.15).max(0.05) as f32;
    let debug_enabled = std::env::var("CONTAINER_VAD_DEBUG")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mut debug_log = String::new();
    if debug_enabled {
        let _ = writeln!(
            debug_log,
            "CONTAINER Silero VAD parity trace\nthreshold={threshold:.3} release={release:.3} min_silence={min_silence:.3}s min_speech={min_speech:.3}s padding={padding:.3}s duration={duration:.3}s chunks={} chunk_seconds={CHUNK_SECONDS:.3}",
            scores.len()
        );
    }

    let mut speaking = false;
    let mut flags = Vec::with_capacity(scores.len());
    for (index, &score) in scores.iter().enumerate() {
        let event = if !speaking && score >= threshold as f32 {
            speaking = true;
            " ENTER_SPEECH"
        } else if speaking && score < release {
            speaking = false;
            " EXIT_SPEECH"
        } else {
            ""
        };
        flags.push(speaking);
        if debug_enabled {
            let _ = writeln!(
                debug_log,
                "FRAME index={index} sample={} time={:.3}s probability={score:.6} speech={}{}",
                index * 512,
                index as f64 * CHUNK_SECONDS,
                speaking,
                event
            );
        }
    }

    let mut regions = Vec::new();
    let mut candidate_start = None;
    for (index, is_speech) in flags.iter().copied().enumerate() {
        match (candidate_start, is_speech) {
            (None, true) => {
                candidate_start = Some(index);
                if debug_enabled {
                    let _ = writeln!(debug_log, "CANDIDATE_START chunk={index}");
                }
            }
            (Some(start), false) => {
                regions.push((start, index));
                candidate_start = None;
                if debug_enabled {
                    let _ = writeln!(debug_log, "CANDIDATE_END chunks={start}..{index}");
                }
            }
            _ => {}
        }
    }
    if let Some(start) = candidate_start {
        regions.push((start, flags.len()));
        if debug_enabled {
            let _ = writeln!(
                debug_log,
                "CANDIDATE_END chunks={start}..{} EOF",
                flags.len()
            );
        }
    }

    let min_gap = vad_chunks_for_seconds(min_silence, CHUNK_SECONDS);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in regions {
        if let Some(last) = merged.last_mut() {
            let gap = start.saturating_sub(last.1);
            // AutoCut uses a strict comparison: a silence must be at least
            // min_silence long to split two speech regions.
            if gap < min_gap {
                if debug_enabled {
                    let _ = writeln!(
                        debug_log,
                        "MERGE previous={}..{} next={start}..{end} gap_chunks={gap}",
                        last.0, last.1
                    );
                }
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let min_len = vad_chunks_for_seconds(min_speech, CHUNK_SECONDS);
    let mut keeps: Vec<KeepInterval> = merged
        .into_iter()
        .filter_map(|(start, end)| {
            let length = end.saturating_sub(start);
            let peak = scores[start..end]
                .iter()
                .copied()
                .fold(0.0_f32, f32::max);
            // Preserve compact, high-confidence interjections (at least 64 ms)
            // while still rejecting isolated one-frame clicks/spikes.
            let strong_short_speech = length >= 2
                && peak >= ((threshold as f32 + 0.30).min(0.90));
            if length < min_len && !strong_short_speech {
                if debug_enabled {
                    let _ = writeln!(
                        debug_log,
                        "REJECT_SHORT chunks={start}..{end} length_chunks={length} peak={peak:.4} required_chunks={min_len}",
                    );
                }
                return None;
            }
            let padded_start = (start as f64 * CHUNK_SECONDS - padding).max(0.0);
            let padded_end = (end as f64 * CHUNK_SECONDS + padding).min(duration);
            if debug_enabled {
                let _ = writeln!(
                    debug_log,
                    "PADDING chunks={start}..{end} raw={:.3}..{:.3} padded={padded_start:.3}..{padded_end:.3}",
                    start as f64 * CHUNK_SECONDS,
                    end as f64 * CHUNK_SECONDS
                );
            }
            Some(KeepInterval {
                start: padded_start,
                end: padded_end,
                enabled: true,
            })
        })
        .collect();

    let mut normalized: Vec<KeepInterval> = Vec::new();
    for keep in keeps.drain(..) {
        if let Some(last) = normalized.last_mut() {
            if keep.start <= last.end {
                if debug_enabled {
                    let _ = writeln!(
                        debug_log,
                        "MERGE_PADDED previous={:.3}..{:.3} next={:.3}..{:.3}",
                        last.start, last.end, keep.start, keep.end
                    );
                }
                last.end = last.end.max(keep.end);
                continue;
            }
        }
        normalized.push(keep)
    }

    if debug_enabled {
        for (index, keep) in normalized.iter().enumerate() {
            let _ = writeln!(
                debug_log,
                "FINAL_KEEP index={} start={:.3} end={:.3} duration={:.3}",
                index + 1,
                keep.start,
                keep.end,
                keep.end - keep.start
            );
        }
        let path = std::env::var_os("CONTAINER_VAD_DEBUG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("container-vad-debug.log"));
        if let Err(error) = std::fs::write(&path, debug_log) {
            eprintln!(
                "Could not write VAD debug trace to {}: {error}",
                path.display()
            );
        } else {
            eprintln!("VAD debug trace written to {}", path.display());
        }
    }
    normalized
}

fn vad_chunks_for_seconds(seconds: f64, chunk_seconds: f64) -> usize {
    (seconds.max(0.0) / chunk_seconds).ceil() as usize
}

fn refine_boundary(samples: &[i16], sample: usize, direction: i8) -> usize {
    const SAMPLE_RATE: usize = 16_000;
    const SEARCH: usize = SAMPLE_RATE * 120 / 1000;
    const WINDOW: usize = SAMPLE_RATE * 10 / 1000;
    if samples.is_empty() || sample == 0 || sample >= samples.len() {
        return sample.min(samples.len());
    }
    let rms = |start: usize| {
        let end = (start + WINDOW).min(samples.len());
        if end <= start {
            return 0.0;
        }
        let energy = samples[start..end]
            .iter()
            .map(|value| {
                let normalized = *value as f64 / 32768.0;
                normalized * normalized
            })
            .sum::<f64>();
        (energy / (end - start) as f64).sqrt()
    };
    let reference = if direction < 0 {
        rms(sample.min(samples.len().saturating_sub(WINDOW)))
    } else {
        rms(sample.saturating_sub(WINDOW))
    };
    let (from, to) = if direction < 0 {
        (sample.saturating_sub(SEARCH), sample)
    } else {
        (sample, (sample + SEARCH).min(samples.len()))
    };
    let mut quietest = (sample, f64::MAX);
    for position in (from..to).step_by(WINDOW.max(1)) {
        let energy = rms(position);
        if energy < quietest.1 {
            quietest = (position, energy);
        }
    }
    // Background music/gameplay may never become quiet. In that case VAD is
    // the safer boundary and refinement deliberately does nothing.
    if quietest.1 <= 0.006 || quietest.1 <= reference * 0.65 {
        if direction < 0 {
            quietest.0.min(sample)
        } else {
            (quietest.0 + WINDOW).max(sample).min(samples.len())
        }
    } else {
        sample
    }
}

fn natural_keeps_from_vad_scores(
    scores: &[f32],
    samples: &[i16],
    threshold: f64,
    min_silence: f64,
    min_speech: f64,
    settings: AutoCutEditSettings,
    duration: f64,
) -> (Vec<KeepInterval>, usize) {
    const SAMPLE_RATE: f64 = 16_000.0;
    let mut speech =
        keeps_from_vad_scores(scores, threshold, min_silence, min_speech, 0.0, duration);
    let minimum_pause_samples = (settings.minimum_pause * SAMPLE_RATE).round() as usize;
    let mut stable: Vec<(usize, usize)> = Vec::with_capacity(speech.len());
    for region in speech.drain(..) {
        let start = (region.start * SAMPLE_RATE).round().max(0.0) as usize;
        let end = (region.end * SAMPLE_RATE).round().max(0.0) as usize;
        if let Some(previous) = stable.last_mut() {
            if start.saturating_sub(previous.1) < minimum_pause_samples {
                previous.1 = previous.1.max(end);
                continue;
            }
        }
        stable.push((start, end));
    }
    let mut padded = Vec::with_capacity(stable.len());
    for (mut start, mut end) in stable {
        if settings.boundary_refinement {
            start = refine_boundary(samples, start, -1);
            end = refine_boundary(samples, end, 1);
        }
        padded.push(KeepInterval {
            start: (start as f64 / SAMPLE_RATE - settings.keep_before_speech).max(0.0),
            end: (end as f64 / SAMPLE_RATE + settings.keep_after_speech).min(duration),
            enabled: true,
        });
    }
    let overlaps = padded
        .windows(2)
        .filter(|pair| pair[1].start <= pair[0].end)
        .count();
    (normalize_keep_intervals(&padded, duration), overlaps)
}

#[tauri::command]
async fn recommend_autocut_settings(path: String) -> Result<AutoCutRecommendation, String> {
    let input = PathBuf::from(path);
    if !input.is_file() {
        return Err("Analysis media was not found.".into());
    }
    let output = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(&input)
        .args(["-vn", "-ac", "1", "-ar", "1000", "-f", "f32le", "pipe:1"])
        .output()
        .await
        .map_err(|e| format!("FFmpeg auto analysis could not start: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let samples: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    if samples.len() < 100 {
        return Err("Not enough audio to calculate automatic settings.".into());
    }
    let levels: Vec<f64> = samples
        .chunks(20)
        .map(|block| {
            let mean = block
                .iter()
                .map(|sample| (*sample as f64).powi(2))
                .sum::<f64>()
                / block.len() as f64;
            20.0 * mean.sqrt().max(0.000001).log10()
        })
        .collect();
    Ok(recommend_from_levels(&levels, 0.02))
}

#[tauri::command]
async fn compute_autocut_waveform(path: String) -> Result<Vec<f32>, String> {
    let input = PathBuf::from(path);
    if !input.is_file() {
        return Err("Media file was not found.".into());
    }
    waveform_for(&input).await
}

#[tauri::command]
async fn compute_video_filmstrip(path: String) -> Result<String, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err("Media file was not found.".into());
    }
    let info = probe_media(path).await?;
    if info.kind != "video" {
        return Err("Filmstrip preview requires a video file.".into());
    }
    let duration = info.duration.unwrap_or(1.0).max(0.1);
    // Seek before each input so a long/high-FPS recording is not decoded from
    // start to finish merely to obtain a few representative thumbnails.
    const TILES: usize = 7;
    let mut command = hidden_command("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error"]);
    for index in 0..TILES {
        let at = duration * (index as f64 + 0.5) / TILES as f64;
        command.args(["-ss", &format!("{at:.6}"), "-i"]).arg(&input);
    }
    let mut filters = Vec::with_capacity(TILES + 1);
    for index in 0..TILES {
        filters.push(format!("[{index}:v]scale=160:90:force_original_aspect_ratio=decrease,pad=160:90:(ow-iw)/2:(oh-ih)/2:black[v{index}]"));
    }
    let inputs = (0..TILES)
        .map(|index| format!("[v{index}]"))
        .collect::<String>();
    filters.push(format!("{inputs}hstack=inputs={TILES}[strip]"));
    let filter = filters.join(";");
    let output = command
        .args([
            "-filter_complex",
            &filter,
            "-map",
            "[strip]",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "pipe:1",
        ])
        .output()
        .await
        .map_err(|error| format!("FFmpeg filmstrip could not start: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    if output.stdout.is_empty() {
        return Err("FFmpeg did not produce a filmstrip preview.".into());
    }
    Ok(format!(
        "data:image/png;base64,{}",
        BASE64.encode(output.stdout)
    ))
}

#[tauri::command]
fn autocut_presets() -> Vec<AutoCutPreset> {
    AUTOCUT_PRESETS.to_vec()
}

fn autocut_edit_settings(request: &AutoCutRequest) -> AutoCutEditSettings {
    // Compatibility path for pre-asymmetric clients/saved state: the old
    // padding value becomes both sides and minimum-pause does not alter the
    // already validated min-silence segmentation.
    let legacy_padding = request.padding.unwrap_or(0.12);
    AutoCutEditSettings {
        minimum_pause: request.minimum_pause.unwrap_or(request.min_silence),
        keep_before_speech: request.keep_before_speech.unwrap_or(legacy_padding),
        keep_after_speech: request.keep_after_speech.unwrap_or(legacy_padding),
        boundary_refinement: request.boundary_refinement.unwrap_or(false),
    }
}

#[tauri::command]
async fn analyze_autocut(
    state: State<'_, JobState>,
    request: AutoCutRequest,
) -> Result<AutoCutAnalysis, String> {
    let info = probe_media(request.input.clone()).await?;
    if info.kind != "video" {
        return Err("SmartCut requires a video file.".into());
    }
    let duration = info.duration.ok_or("Video duration could not be read.")?;
    check_range(request.threshold, 0.05, 0.95, "Threshold")?;
    check_range(request.min_silence, 0.03, 30.0, "Minimum silence")?;
    check_range(request.min_speech, 0.03, 30.0, "Minimum speech")?;
    let edit = autocut_edit_settings(&request);
    check_range(edit.minimum_pause, 0.03, 30.0, "Minimum pause")?;
    check_range(edit.keep_before_speech, 0.0, 5.0, "Keep before speech")?;
    check_range(edit.keep_after_speech, 0.0, 5.0, "Keep after speech")?;
    let input = PathBuf::from(request.analysis_input.as_deref().unwrap_or(&request.input));
    if !input.is_file() {
        return Err("Analysis audio file was not found.".into());
    }
    let analysis_info = probe_media(input.to_string_lossy().to_string()).await?;
    if analysis_info.audio_codec.is_none() && analysis_info.kind != "audio" {
        return Err("Silence detection requires an audio stream.".into());
    }
    let (scores, samples) = cached_vad_data(&state, &input).await?;
    let (cuts, overlaps_before_normalization) = natural_keeps_from_vad_scores(
        &scores,
        &samples,
        request.threshold,
        request.min_silence,
        request.min_speech,
        edit,
        duration,
    );
    Ok(AutoCutAnalysis {
        cuts,
        waveform: waveform_for(&input).await?,
        duration,
        boundary_refinement: edit.boundary_refinement,
        overlaps_before_normalization,
    })
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn normalize_keep_intervals(cuts: &[KeepInterval], duration: f64) -> Vec<KeepInterval> {
    const TIMESTAMP_EPSILON: f64 = 0.001;
    let mut values: Vec<_> = cuts
        .iter()
        .filter(|c| c.enabled)
        .map(|c| KeepInterval {
            start: c.start.clamp(0.0, duration),
            end: c.end.clamp(0.0, duration),
            enabled: true,
        })
        .filter(|c| c.end - c.start >= 0.01)
        .collect();
    values.sort_by(|a, b| a.start.total_cmp(&b.start));
    let debug = std::env::var("CONTAINER_AUTOCUT_DEBUG")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mut normalized: Vec<KeepInterval> = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        if let Some(previous) = normalized.last_mut() {
            let gap = value.start - previous.end;
            if debug {
                eprintln!(
                    "AUTOCUT REGION input={} start={:.6} end={:.6} duration={:.6} previous_end={:.6} gap={:.6} overlap={:.6}",
                    index + 1,
                    value.start,
                    value.end,
                    value.end - value.start,
                    previous.end,
                    gap,
                    (-gap).max(0.0)
                );
            }
            if value.start <= previous.end + TIMESTAMP_EPSILON {
                previous.end = previous.end.max(value.end);
                continue;
            }
        } else if debug {
            eprintln!(
                "AUTOCUT REGION input=1 start={:.6} end={:.6} duration={:.6}",
                value.start,
                value.end,
                value.end - value.start
            );
        }
        normalized.push(value);
    }
    debug_assert!(normalized
        .windows(2)
        .all(|pair| pair[0].end < pair[1].start));
    normalized
}

fn enabled_cuts(cuts: &[KeepInterval], duration: f64) -> Result<Vec<KeepInterval>, String> {
    let normalized = normalize_keep_intervals(cuts, duration);
    if normalized.is_empty() {
        Err("At least one enabled cut is required.".into())
    } else {
        Ok(normalized)
    }
}

fn autocut_filter_graph(cuts: &[KeepInterval], has_audio: bool, resolution: &str) -> String {
    let mut graph = String::new();
    if cuts.len() > 1 {
        let video_outputs = (0..cuts.len())
            .map(|index| format!("[vsrc{index}]"))
            .collect::<String>();
        let _ = writeln!(graph, "[0:v]split={}{video_outputs};", cuts.len());
        if has_audio {
            let audio_outputs = (0..cuts.len())
                .map(|index| format!("[asrc{index}]"))
                .collect::<String>();
            let _ = writeln!(graph, "[0:a]asplit={}{audio_outputs};", cuts.len());
        }
    }
    for (index, cut) in cuts.iter().enumerate() {
        let video_source = if cuts.len() == 1 {
            "[0:v]".to_string()
        } else {
            format!("[vsrc{index}]")
        };
        let _ = writeln!(
            graph,
            "{video_source}trim=start={:.6}:end={:.6},setpts=PTS-STARTPTS[v{index}];",
            cut.start, cut.end
        );
        if has_audio {
            let audio_source = if cuts.len() == 1 {
                "[0:a]".to_string()
            } else {
                format!("[asrc{index}]")
            };
            let _ = writeln!(
                graph,
                "{audio_source}atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS[a{index}];",
                cut.start, cut.end
            );
        }
    }
    let inputs = (0..cuts.len())
        .map(|index| {
            if has_audio {
                format!("[v{index}][a{index}]")
            } else {
                format!("[v{index}]")
            }
        })
        .collect::<String>();
    let video_label = if resolution == "source" {
        "vout"
    } else {
        "vconcat"
    };
    let _ = writeln!(
        graph,
        "{inputs}concat=n={}:v=1:a={}[{video_label}]{};",
        cuts.len(),
        usize::from(has_audio),
        if has_audio { "[aout]" } else { "" }
    );
    if resolution != "source" {
        let _ = writeln!(graph, "[vconcat]scale=-2:{resolution}[vout];");
    }
    graph
}

async fn export_autocut_inner(
    app: Option<&AppHandle>,
    state: &JobState,
    request: AutoCutExportRequest,
) -> Result<JobResult, String> {
    state.cancelled.store(false, Ordering::Relaxed);
    let info = probe_media(request.input.clone()).await?;
    let duration = info.duration.ok_or("Video duration could not be read.")?;
    let cuts = enabled_cuts(&request.cuts, duration)?;
    let input = PathBuf::from(&request.input);
    if request.format.eq_ignore_ascii_case("fcpxml") {
        let output = unique_output(&input, "smartcut", "fcpxml")?;
        let fps_value = info.fps.unwrap_or(30.0);
        let fps = fps_value.round().max(1.0) as u64;
        let reference_tc = parse_smpte(info.start_timecode.as_deref(), fps_value);
        let frame_duration = if (fps_value - 29.97).abs() < 0.1 {
            "1001/30000s".to_string()
        } else if (fps_value - 59.94).abs() < 0.1 {
            "1001/60000s".to_string()
        } else if (fps_value - 23.976).abs() < 0.1 {
            "1001/24000s".to_string()
        } else {
            format!("1/{fps}s")
        };
        let tc_format = if info
            .start_timecode
            .as_deref()
            .is_some_and(|tc| tc.contains(';'))
        {
            "DF"
        } else {
            "NDF"
        };
        let uri = format!(
            "file:///{}",
            request.input.replace('\\', "/").replace(' ', "%20")
        );
        let mut resources = format!("<format id=\"r1\" name=\"FFVideoFormat\" frameDuration=\"{frame_duration}\"/><asset id=\"r2\" name=\"{}\" src=\"{}\" start=\"{reference_tc:.6}s\" duration=\"{duration:.6}s\" hasVideo=\"1\" hasAudio=\"{}\"/>", xml_escape(&info.name), xml_escape(&uri), if info.audio_codec.is_some() { 1 } else { 0 });
        let mut prepared = Vec::new();
        let mut next_video_lane = 1_i32;
        let mut next_audio_lane = -1_i32;
        for (track_index, track) in request.linked_tracks.iter().enumerate() {
            let linked_info = probe_media(track.path.clone()).await?;
            let linked_duration = linked_info.duration.unwrap_or(duration);
            let linked_tc = parse_smpte(linked_info.start_timecode.as_deref(), fps_value);
            let id = format!("r{}", track_index + 3);
            let linked_uri = format!(
                "file:///{}",
                track.path.replace('\\', "/").replace(' ', "%20")
            );
            resources.push_str(&format!("<asset id=\"{id}\" name=\"{}\" src=\"{}\" start=\"{linked_tc:.6}s\" duration=\"{linked_duration:.6}s\" hasVideo=\"{}\" hasAudio=\"{}\"/>", xml_escape(&linked_info.name), xml_escape(&linked_uri), if linked_info.kind == "video" { 1 } else { 0 }, if linked_info.kind == "audio" || linked_info.audio_codec.is_some() { 1 } else { 0 }));
            let lane = if linked_info.kind == "video" {
                let value = next_video_lane;
                next_video_lane += 1;
                value
            } else {
                let value = next_audio_lane;
                next_audio_lane -= 1;
                value
            };
            prepared.push((
                id,
                linked_info,
                linked_duration,
                linked_tc,
                track.offset,
                lane,
            ));
        }
        let mut cursor = 0.0;
        let mut clips = String::new();
        for (index, cut) in cuts.iter().enumerate() {
            let len = cut.end - cut.start;
            let mut connected = String::new();
            for (id, linked_info, linked_duration, linked_tc, offset, lane) in &prepared {
                let visible_from = cut.start.max(*offset);
                let visible_to = cut.end.min(*offset + *linked_duration);
                if visible_to > visible_from {
                    let clip_offset = cursor + (visible_from - cut.start);
                    let source_start = linked_tc + (visible_from - offset);
                    connected.push_str(&format!("<asset-clip name=\"{}\" ref=\"{id}\" lane=\"{lane}\" offset=\"{clip_offset:.6}s\" start=\"{source_start:.6}s\" duration=\"{:.6}s\" tcFormat=\"{tc_format}\"/>", xml_escape(&linked_info.name), visible_to-visible_from));
                }
            }
            if connected.is_empty() {
                clips.push_str(&format!("<asset-clip name=\"Cut {}\" ref=\"r2\" offset=\"{cursor:.6}s\" start=\"{:.6}s\" duration=\"{len:.6}s\" tcFormat=\"{tc_format}\"/>", index+1,reference_tc+cut.start));
            } else {
                clips.push_str(&format!("<asset-clip name=\"Cut {}\" ref=\"r2\" offset=\"{cursor:.6}s\" start=\"{:.6}s\" duration=\"{len:.6}s\" tcFormat=\"{tc_format}\">{connected}</asset-clip>", index+1,reference_tc+cut.start));
            }
            cursor += len;
        }
        let xml=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.11\"><resources>{resources}</resources><library><event name=\"CONTAINER SmartCut\"><project name=\"{}\"><sequence format=\"r1\" duration=\"{cursor:.6}s\" tcFormat=\"{tc_format}\"><spine>{clips}</spine></sequence></project></event></library></fcpxml>",xml_escape(&safe_stem(&input)));
        std::fs::write(&output, xml).map_err(|e| e.to_string())?;
        if let Some(app) = app {
            allow_asset_file(app, &output)?;
        }
        return Ok(JobResult {
            output: output.to_string_lossy().into(),
            elapsed: 0.0,
        });
    }
    let output = unique_output(&input, "smartcut", "mp4")?;
    let graph = autocut_filter_graph(&cuts, info.audio_codec.is_some(), &request.resolution);
    if std::env::var_os("CONTAINER_AUTOCUT_DEBUG").is_some() {
        eprintln!("AUTOCUT FILTER GRAPH\n{graph}");
    }
    let crf = match request.quality.as_str() {
        "high" => "18",
        "small" => "26",
        _ => "22",
    };
    let audio_bitrate = match request.quality.as_str() {
        "high" => "192k",
        "small" => "96k",
        _ => "128k",
    };
    let kept_total: f64 = cuts.iter().map(|c| c.end - c.start).sum();
    let started = Instant::now();
    let mut command = hidden_command("ffmpeg");
    command
        .args(["-hide_banner", "-y", "-i"])
        .arg(&input)
        .arg("-filter_complex")
        .arg(&graph)
        .args(["-map", "[vout]"])
        .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", crf]);
    if info.audio_codec.is_some() {
        command.args(["-map", "[aout]", "-c:a", "aac", "-b:a", audio_bitrate]);
    } else {
        command.arg("-an");
    }
    command
        .args(["-movflags", "+faststart", "-progress", "pipe:1", "-nostats"])
        .arg(&output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|e| format!("FFmpeg could not start: {e}"))?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = child.id();
    let stdout = child.stdout.take().ok_or("FFmpeg progress unavailable.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("FFmpeg diagnostics unavailable.")?;
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut all = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                all.push(line)
            }
        }
        all
    });
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        if state.cancelled.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            break;
        }
        if let Some(value) = line.strip_prefix("out_time_us=") {
            let seconds = value.parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
            if let Some(app) = app {
                let _ = app.emit(
                    "container-progress",
                    ProgressEvent {
                        percent: (seconds / kept_total * 100.0).clamp(0.0, 99.9),
                        time: started.elapsed().as_secs_f64(),
                        speed: "—".into(),
                        frame: "—".into(),
                        status: "exporting smartcut".into(),
                    },
                );
            }
        }
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = None;
    let errors = stderr_task.await.unwrap_or_default();
    if state.cancelled.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&output);
        return Err("Job cancelled.".into());
    }
    if !status.success() {
        let _ = std::fs::remove_file(&output);
        return Err(errors
            .into_iter()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n"));
    }
    if let Some(app) = app {
        allow_asset_file(app, &output)?;
    }
    Ok(JobResult {
        output: output.to_string_lossy().into(),
        elapsed: started.elapsed().as_secs_f64(),
    })
}

#[tauri::command]
async fn export_autocut(
    app: AppHandle,
    state: State<'_, JobState>,
    request: AutoCutExportRequest,
) -> Result<JobResult, String> {
    export_autocut_inner(Some(&app), &state, request).await
}

fn atempo(speed: f64) -> String {
    let mut filters = Vec::new();
    let mut current = speed;
    while current > 2.0 {
        filters.push("atempo=2.0".to_string());
        current /= 2.0;
    }
    while current < 0.5 {
        filters.push("atempo=0.5".to_string());
        current /= 0.5;
    }
    filters.push(format!("atempo={current:.6}"));
    filters.join(",")
}

fn drawtext_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'")
        .replace(',', "\\,")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn audio_format(format: &str) -> Result<(&'static str, Vec<String>), String> {
    match format {
        "aac" => Ok((
            "m4a",
            vec!["-c:a".into(), "aac".into(), "-b:a".into(), "320k".into()],
        )),
        "mp3" => Ok((
            "mp3",
            vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "320k".into(),
            ],
        )),
        "wav" => Ok(("wav", vec!["-c:a".into(), "pcm_s32le".into()])),
        "flac" => Ok(("flac", vec!["-c:a".into(), "flac".into()])),
        "opus" => Ok((
            "opus",
            vec![
                "-c:a".into(),
                "libopus".into(),
                "-b:a".into(),
                "160k".into(),
            ],
        )),
        _ => Err("Unsupported audio format.".into()),
    }
}

fn append_audio_routing(
    args: &mut Vec<String>,
    info: &MediaInfo,
    params: &HashMap<String, String>,
    reencode: bool,
    default_mode: &str,
) -> Result<(), String> {
    if info.audio_tracks.is_empty() {
        args.push("-an".into());
        return Ok(());
    }
    let mode = params
        .get("audio_mode")
        .map(String::as_str)
        .unwrap_or(default_mode);
    match mode {
        "none" => args.push("-an".into()),
        "main" => args.extend(["-map".into(), "0:a:0?".into()]),
        "all" => args.extend(["-map".into(), "0:a?".into()]),
        "selected" => {
            let requested = params
                .get("audio_track")
                .ok_or("Choose an audio track.")?
                .parse::<u64>()
                .map_err(|_| "Invalid audio track.")?;
            if !info
                .audio_tracks
                .iter()
                .any(|track| track.index == requested)
            {
                return Err("The selected audio track is not present in this file.".into());
            }
            args.extend(["-map".into(), format!("0:{requested}?")]);
        }
        "merge" => {
            let inputs = info
                .audio_tracks
                .iter()
                .map(|track| format!("[0:{}]", track.index))
                .collect::<String>();
            args.extend([
                "-filter_complex".into(),
                format!(
                    "{inputs}amix=inputs={}:duration=longest:dropout_transition=2:normalize=1[aout]",
                    info.audio_tracks.len()
                ),
                "-map".into(),
                "[aout]".into(),
            ]);
        }
        _ => return Err("Invalid audio-track mode.".into()),
    }
    if mode != "none" {
        if reencode || mode == "merge" {
            args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
        } else {
            args.extend(["-c:a".into(), "copy".into()]);
        }
    }
    Ok(())
}

async fn build_command(
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<(Vec<String>, PathBuf), String> {
    let input = PathBuf::from(&request.input);
    let p = &request.params;
    let op = request.operation.as_str();
    let mut args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        request.input.clone(),
    ];
    let extension: String;

    match op {
        "transform" => {
            if info.kind != "video" && info.kind != "image" {
                return Err("Transform requires a video or image file.".into());
            }
            let crop_mode = param(p, "crop_mode")?;
            let mut filters = Vec::new();
            match param(p, "rotate")? {
                "0" => {}
                "90" => filters.push("transpose=clock".into()),
                "180" => filters.push("hflip,vflip".into()),
                "270" => filters.push("transpose=cclock".into()),
                _ => return Err("Invalid rotation.".into()),
            }
            // Flip after rotation so Horizontal and Vertical always describe
            // the axes visible to the user in the preview.
            if param(p, "flip_h")? == "true" {
                filters.push("hflip".into());
            }
            if param(p, "flip_v")? == "true" {
                filters.push("vflip".into());
            }
            if crop_mode != "off" {
                let x = check_range(parse_number(p, "crop_x")?, 0.0, 99.0, "Crop X")?;
                let y = check_range(parse_number(p, "crop_y")?, 0.0, 99.0, "Crop Y")?;
                let w = check_range(parse_number(p, "crop_w")?, 1.0, 100.0, "Crop width")?;
                let h = check_range(parse_number(p, "crop_h")?, 1.0, 100.0, "Crop height")?;
                if x + w > 100.001 || y + h > 100.001 {
                    return Err("Crop rectangle must stay inside the video.".into());
                }
                filters.push(format!(
                    "crop=trunc(iw*{w}/100/2)*2:trunc(ih*{h}/100/2)*2:trunc(iw*{x}/100/2)*2:trunc(ih*{y}/100/2)*2"
                ));
            }
            match param(p, "size_mode")? {
                "source" => {}
                "height" => {
                    let size = check_range(parse_number(p, "size")?, 2.0, 7680.0, "Size")? as u64;
                    filters.push(format!("scale=-2:trunc({size}/2)*2:flags=lanczos"));
                }
                "width" => {
                    let size = check_range(parse_number(p, "size")?, 2.0, 7680.0, "Size")? as u64;
                    filters.push(format!("scale=trunc({size}/2)*2:-2:flags=lanczos"));
                }
                "exact" => {
                    let width =
                        check_range(parse_number(p, "output_width")?, 2.0, 7680.0, "Width")? as u64;
                    let height =
                        check_range(parse_number(p, "output_height")?, 2.0, 7680.0, "Height")?
                            as u64;
                    filters.push(format!(
                        "scale=trunc({width}/2)*2:trunc({height}/2)*2:flags=lanczos"
                    ));
                }
                _ => return Err("Invalid output size mode.".into()),
            }
            if !filters.is_empty() {
                args.extend(["-vf".into(), filters.join(",")]);
            }
            if info.kind == "image" {
                args.extend(["-frames:v".into(), "1".into()]);
                extension = match param(p, "format")? {
                    "png" => {
                        args.extend(["-c:v".into(), "png".into()]);
                        "png".into()
                    }
                    "jpg" | "jpeg" => {
                        args.extend(["-c:v".into(), "mjpeg".into(), "-q:v".into(), "2".into()]);
                        "jpg".into()
                    }
                    "webp" => {
                        args.extend([
                            "-c:v".into(),
                            "libwebp".into(),
                            "-lossless".into(),
                            "1".into(),
                        ]);
                        "webp".into()
                    }
                    _ => return Err("Invalid image output format.".into()),
                };
            } else {
                args.extend([
                    "-c:v".into(),
                    "libx264".into(),
                    "-qp".into(),
                    "0".into(),
                    "-preset".into(),
                    "veryfast".into(),
                    "-c:a".into(),
                    "copy".into(),
                ]);
                extension = "mp4".into();
            }
        }
        "ratio" => {
            let ratio = param(p, "ratio")?;
            let (rw, rh) = match ratio {
                "1:1" => (1, 1),
                "4:5" => (4, 5),
                "9:16" => (9, 16),
                "16:9" => (16, 9),
                "4:3" => (4, 3),
                _ => return Err("Invalid ratio.".into()),
            };
            args.extend(["-vf".into(), format!("crop='if(gt(iw/ih,{rw}/{rh}),trunc(ih*{rw}/{rh}/2)*2,iw)':'if(gt(iw/ih,{rw}/{rh}),ih,trunc(iw*{rh}/{rw}/2)*2)'"), "-c:v".into(), "libx264".into(), "-crf".into(), "16".into(), "-preset".into(), "veryfast".into(), "-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
            extension = "mp4".into();
        }
        "image_ratio" => {
            if info.kind != "image" {
                return Err("Social Ratio / Crop requires an image file.".into());
            }
            let ratio = param(p, "ratio")?;
            let (rw, rh) = match ratio {
                "1:1" => (1, 1),
                "4:5" => (4, 5),
                "9:16" => (9, 16),
                "16:9" => (16, 9),
                "191:100" => (191, 100),
                "2:3" => (2, 3),
                "3:2" => (3, 2),
                "4:3" => (4, 3),
                _ => return Err("Invalid image ratio.".into()),
            };
            args.extend([
                "-vf".into(),
                format!("crop='if(gt(iw/ih,{rw}/{rh}),floor(ih*{rw}/{rh}),iw)':'if(gt(iw/ih,{rw}/{rh}),ih,floor(iw*{rh}/{rw}))'"),
                "-frames:v".into(),
                "1".into(),
            ]);
            extension = match param(p, "format")? {
                "png" => {
                    args.extend(["-c:v".into(), "png".into()]);
                    "png".into()
                }
                "jpg" | "jpeg" => {
                    args.extend(["-c:v".into(), "mjpeg".into(), "-q:v".into(), "2".into()]);
                    "jpg".into()
                }
                _ => return Err("Invalid image output format.".into()),
            };
        }
        "upscale" => {
            if info.kind != "video" {
                return Err("Upscale requires a video file.".into());
            }
            let width = info.width.ok_or("Source video width is unavailable.")?;
            let height = info.height.ok_or("Source video height is unavailable.")?;
            let target_edge =
                check_range(parse_number(p, "target_edge")?, 720.0, 4320.0, "Resolution")? as u64;
            if target_edge <= width.min(height) {
                return Err("Upscale target must be larger than the source resolution.".into());
            }
            let target_long =
                (width.max(height) as f64 / width.min(height) as f64 * target_edge as f64).ceil();
            if target_long > 7680.0 {
                return Err("Upscale output would exceed the supported 7680-pixel limit.".into());
            }
            let filter = if width >= height {
                format!("scale=-2:trunc({target_edge}/2)*2:flags=lanczos,setsar=1")
            } else {
                format!("scale=trunc({target_edge}/2)*2:-2:flags=lanczos,setsar=1")
            };
            args.extend([
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                "0:a?".into(),
                "-vf".into(),
                filter,
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "14".into(),
                "-preset".into(),
                "slow".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
            let audio_copy_safe = info
                .audio_tracks
                .iter()
                .all(|track| ["aac", "mp3", "ac3", "eac3", "alac"].contains(&track.codec.as_str()));
            if info.audio_codec.is_some() {
                if audio_copy_safe {
                    args.extend(["-c:a".into(), "copy".into()]);
                } else {
                    args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
                }
            }
            args.extend(["-movflags".into(), "+faststart".into()]);
            extension = "mp4".into();
        }
        "resize" => {
            let size = check_range(parse_number(p, "size")?, 2.0, 7680.0, "Size")? as u64;
            let crf = check_range(parse_number(p, "crf")?, 0.0, 30.0, "CRF")?;
            let filter = if param(p, "axis")? == "width" {
                format!("scale=trunc({size}/2)*2:-2")
            } else {
                format!("scale=-2:trunc({size}/2)*2")
            };
            args.extend([
                "-vf".into(),
                filter,
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                crf.to_string(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "fps" | "cfr" => {
            let fps = check_range(parse_number(p, "fps")?, 1.0, 2400.0, "FPS")?;
            let crf = p
                .get("crf")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(16.0);
            args.extend([
                "-vf".into(),
                format!("fps={fps}"),
                "-fps_mode".into(),
                "cfr".into(),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                crf.to_string(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "interpolation" => {
            let fps = check_range(parse_number(p, "fps")?, 60.0, 2400.0, "FPS")?;
            if info.fps.is_some_and(|current| fps <= current)
                || (fps / 60.0 - (fps / 60.0).round()).abs() > 1e-8
            {
                return Err(
                    "Interpolation FPS must be above input FPS and a multiple of 60.".into(),
                );
            }
            args.extend([
                "-vf".into(),
                format!("minterpolate=fps={fps}:mi_mode=blend"),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "ultrafast".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "frame_blend" => {
            let fps = check_range(parse_number(p, "fps")?, 1.0, 2399.0, "FPS")?;
            let current = info.fps.ok_or("Input FPS is unknown.")?;
            if fps >= current {
                return Err("Frame Blending target must be below input FPS.".into());
            }
            let frames = ((current / fps).round() as u64).clamp(2, 128);
            args.extend([
                "-vf".into(),
                format!("tmix=frames={frames},fps={fps}"),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "dedupe" => {
            let profile = param(p, "profile")?;
            let filter = match profile {
                "safe" => "mpdecimate",
                "strong" => "mpdecimate=hi=8000:lo=4000:frac=0.50",
                _ => return Err("Invalid duplicate-frame detection profile.".into()),
            };
            args.extend([
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                "0:a?".into(),
                "-vf".into(),
                filter.into(),
                "-fps_mode".into(),
                "vfr".into(),
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-crf".into(),
                "18".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "speed" => {
            let speed = check_range(parse_number(p, "speed")?, 0.05, 100.0, "Speed")?;
            let mode = p.get("speed_mode").map(String::as_str).unwrap_or("synced");
            if mode == "lossless_video" {
                args = vec![
                    "-hide_banner".into(),
                    "-loglevel".into(),
                    "error".into(),
                    "-y".into(),
                    "-itsscale".into(),
                    format!("{:.10}", 1.0 / speed),
                    "-i".into(),
                    request.input.clone(),
                    "-map".into(),
                    "0:v:0".into(),
                    "-c:v".into(),
                    "copy".into(),
                    "-an".into(),
                ];
                extension = "mkv".into();
            } else if mode == "synced" {
                let crf = check_range(parse_number(p, "crf")?, 0.0, 30.0, "CRF")?;
                args.extend(["-filter:v".into(), format!("setpts=PTS/{speed}")]);
                if info.audio_codec.is_some() {
                    args.extend([
                        "-filter:a".into(),
                        atempo(speed),
                        "-c:a".into(),
                        "aac".into(),
                        "-b:a".into(),
                        "192k".into(),
                    ]);
                }
                args.extend([
                    "-c:v".into(),
                    "libx264".into(),
                    "-crf".into(),
                    crf.to_string(),
                    "-preset".into(),
                    "veryfast".into(),
                ]);
                extension = "mp4".into();
            } else {
                return Err("Invalid speed mode.".into());
            }
        }
        "compression" => {
            let crf = check_range(parse_number(p, "crf")?, 0.0, 30.0, "CRF")?;
            let preset = param(p, "preset")?;
            if !["ultrafast", "veryfast", "medium", "slow"].contains(&preset) {
                return Err("Invalid preset.".into());
            }
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                crf.to_string(),
                "-preset".into(),
                preset.into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "bitrate" => {
            let mbps = check_range(parse_number(p, "mbps")?, 0.05, 500.0, "Bitrate")?;
            let kbps = (mbps * 1000.0).round() as u64;
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-b:v".into(),
                format!("{kbps}k"),
                "-maxrate".into(),
                format!("{}k", (kbps as f64 * 1.35) as u64),
                "-bufsize".into(),
                format!("{}k", kbps * 2),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "potatoify" => {
            let profile = p.get("profile").map(String::as_str).unwrap_or("custom");
            let (fps, vb, ab, shrink) = match profile {
                "decent" => (24.0, 3.0, 2.0, 2.0),
                "bad" => (18.0, 6.0, 5.0, 3.0),
                "terrible" => (12.0, 11.0, 10.0, 5.0),
                "unbearable" => (6.0, 18.0, 18.0, 10.0),
                "random" => {
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as u64;
                    (
                        (5 + seed % 26) as f64,
                        (3 + (seed / 29) % 18) as f64,
                        (3 + (seed / 53) % 18) as f64,
                        (2 + (seed / 97) % 11) as f64,
                    )
                }
                "custom" => (
                    check_range(parse_number(p, "fps")?, 1.0, 120.0, "FPS")?,
                    check_range(
                        parse_number(p, "video_badness")?,
                        1.0,
                        20.0,
                        "Video badness",
                    )?,
                    check_range(
                        parse_number(p, "audio_badness")?,
                        1.0,
                        20.0,
                        "Audio badness",
                    )?,
                    check_range(parse_number(p, "shrink")?, 1.0, 20.0, "Scale")?,
                ),
                _ => return Err("Invalid Potatoify quality profile.".into()),
            };
            let w = ((info.width.unwrap_or(1280) as f64 / shrink) as u64 / 2 * 2).max(2);
            let h = ((info.height.unwrap_or(720) as f64 / shrink) as u64 / 2 * 2).max(2);
            let bitrate = ((w * h) as f64 * fps / vb).max(1000.0) as u64;
            args.extend([
                "-vf".into(),
                format!("scale={w}:{h}:flags=neighbor,fps={fps}"),
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "ultrafast".into(),
                "-b:v".into(),
                bitrate.to_string(),
                "-maxrate".into(),
                bitrate.to_string(),
                "-bufsize".into(),
                (bitrate * 2).to_string(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                format!("{}k", (256.0 / ab).clamp(16.0, 192.0) as u64),
            ]);
            extension = "mp4".into();
        }
        "text" => {
            let layers: Vec<TextLayer> = serde_json::from_str(param(p, "layers")?)
                .map_err(|error| format!("Invalid text layers: {error}"))?;
            if layers.is_empty() {
                return Err("Add at least one text layer.".into());
            }
            let mut text_filters = Vec::with_capacity(layers.len());
            for layer in layers {
                if layer.text.trim().is_empty() {
                    return Err("Text layers cannot be empty.".into());
                }
                let size = check_range(layer.size, 8.0, 600.0, "Font size")?;
                let opacity = check_range(layer.opacity, 0.0, 100.0, "Opacity")? / 100.0;
                let x = check_range(layer.x, 0.0, 100.0, "Text X")? / 100.0;
                let y = check_range(layer.y, 0.0, 100.0, "Text Y")? / 100.0;
                let font = if layer.font_path.is_empty() {
                    PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
                        .join("Fonts")
                        .join("impact.ttf")
                } else {
                    PathBuf::from(&layer.font_path)
                };
                if !font.is_file() {
                    return Err("The selected font is no longer available.".into());
                }
                let outline = check_range(layer.outline, 0.0, 20.0, "Outline")?;
                let shadow = check_range(layer.shadow, 0.0, 30.0, "Shadow")?;
                let outline_color = if layer.outline_color.is_empty() {
                    "#000000"
                } else {
                    &layer.outline_color
                };
                let shadow_color = if layer.shadow_color.is_empty() {
                    "#000000"
                } else {
                    &layer.shadow_color
                };
                let mut filter = format!("drawtext=fontfile='{}':text='{}':fontcolor={}:alpha={opacity:.4}:fontsize={size}:x=w*{x:.6}-text_w/2:y=h*{y:.6}-text_h/2:borderw={outline:.0}:bordercolor={}:shadowx={shadow:.0}:shadowy={shadow:.0}:shadowcolor={}@{:.4}",drawtext_escape(&font.to_string_lossy()),drawtext_escape(&layer.text),drawtext_escape(&layer.color),drawtext_escape(outline_color),drawtext_escape(shadow_color),opacity*0.75);
                if layer.background {
                    let background_opacity =
                        check_range(layer.background_opacity, 0.0, 100.0, "Background opacity")?
                            / 100.0;
                    let background_padding =
                        check_range(layer.background_padding, 0.0, 80.0, "Background padding")?;
                    let background_color = if layer.background_color.is_empty() {
                        "#000000"
                    } else {
                        &layer.background_color
                    };
                    filter.push_str(&format!(":box=1:boxcolor={}@{background_opacity:.4}:boxborderw={background_padding:.0}",drawtext_escape(background_color)));
                }
                text_filters.push(filter);
            }
            args.extend([
                "-vf".into(),
                text_filters.join(","),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "color" => {
            let mut filters = Vec::new();
            let mut eq = Vec::new();
            if enabled(p, "brightness_enabled") {
                eq.push(format!(
                    "brightness={}",
                    check_range(parse_number(p, "brightness")?, -100.0, 100.0, "Brightness")?
                        / 100.0
                ));
            }
            if enabled(p, "contrast_enabled") {
                eq.push(format!(
                    "contrast={}",
                    check_range(parse_number(p, "contrast")?, 0.0, 200.0, "Contrast")? / 100.0
                ));
            }
            if enabled(p, "saturation_enabled") {
                eq.push(format!(
                    "saturation={}",
                    check_range(parse_number(p, "saturation")?, 0.0, 200.0, "Saturation")? / 100.0
                ));
            }
            if enabled(p, "gamma_enabled") {
                eq.push(format!(
                    "gamma={}",
                    check_range(parse_number(p, "gamma")?, 10.0, 300.0, "Gamma")? / 100.0
                ));
            }
            if !eq.is_empty() {
                filters.push(format!("eq={}", eq.join(":")));
            }
            if enabled(p, "hue_enabled") {
                filters.push(format!(
                    "hue=h={}",
                    check_range(parse_number(p, "hue")?, -180.0, 180.0, "Hue")?
                ));
            }
            if enabled(p, "temperature_enabled") {
                filters.push(format!(
                    "colortemperature=temperature={}",
                    check_range(
                        parse_number(p, "temperature")?,
                        1000.0,
                        40000.0,
                        "Temperature"
                    )?
                ));
            }
            if enabled(p, "sharpen_enabled") {
                filters.push(format!(
                    "unsharp=5:5:{:.3}:5:5:0",
                    check_range(parse_number(p, "sharpen")?, 0.0, 100.0, "Sharpen")? * 0.02
                ));
            }
            if enabled(p, "blur_enabled") {
                filters.push(format!(
                    "gblur=sigma={:.3}",
                    check_range(parse_number(p, "blur")?, 0.0, 100.0, "Blur")? / 25.0
                ));
            }
            match p.get("denoise").map(String::as_str).unwrap_or("off") {
                "off" => {}
                "low" => filters.push("hqdn3d=1.5:1.5:6:6".into()),
                "medium" => filters.push("hqdn3d=3:3:8:8".into()),
                "high" => filters.push("hqdn3d=6:6:12:12".into()),
                _ => return Err("Invalid denoise mode.".into()),
            }
            if enabled(p, "deband_enabled") {
                let value = check_range(parse_number(p, "deband")?, 0.0, 100.0, "Deband")? / 1250.0;
                filters.push(format!(
                    "deband=1thr={value:.5}:2thr={value:.5}:3thr={value:.5}:4thr={value:.5}"
                ));
            }
            if enabled(p, "vignette_enabled") {
                let value =
                    check_range(parse_number(p, "vignette")?, 0.0, 100.0, "Vignette")? / 100.0;
                filters.push(format!("vignette=angle=PI/4*{value:.4}"));
            }
            if p.get("grayscale").is_some_and(|value| value == "on") {
                filters.push("hue=s=0".into());
            }
            match p.get("deinterlace").map(String::as_str).unwrap_or("off") {
                "off" => {}
                "auto" => filters.push("yadif=deint=interlaced".into()),
                "on" => filters.push("yadif=deint=all".into()),
                _ => return Err("Invalid interlace mode.".into()),
            }
            if filters.is_empty() {
                return Err("Enable at least one video filter.".into());
            }
            args.extend([
                "-vf".into(),
                filters.join(","),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
            ]);
            extension = "mp4".into();
        }
        "noise" => {
            let value = check_range(parse_number(p, "amount")?, 1.0, 100.0, "Noise")?;
            args.extend([
                "-vf".into(),
                format!("noise=alls={value}:allf=t+u"),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
            ]);
            extension = "mp4".into();
        }
        "negate" => {
            args.extend([
                "-vf".into(),
                "negate".into(),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
            ]);
            extension = "mp4".into();
        }
        "deep_fry" => {
            let level = check_range(parse_number(p, "level")?, 1.0, 10.0, "Level")?;
            let c = 1.0 + level * 0.24;
            let s = 1.0 + level * 0.20;
            let b = (level * 0.018).min(0.22);
            let n = (level * 5.0).min(100.0);
            let sharp = (0.5 + level * 0.42).min(5.0);
            let shift = (level * 1.5).round().max(1.0);
            let filter=format!("eq=contrast={c:.3}:saturation={s:.3}:brightness={b:.3},unsharp=5:5:{sharp:.2}:5:5:0,noise=alls={n}:allf=t+u,chromashift=cbh={shift}:crh=-{shift}");
            args.extend([
                "-vf".into(),
                filter,
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "veryfast".into(),
                "-c:a".into(),
                "aac".into(),
            ]);
            extension = "mp4".into();
        }
        "corruption" => {
            let level = check_range(parse_number(p, "level")?, 1.0, 10.0, "Severity")?.round();
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-crf".into(),
                "20".into(),
                "-bsf:v".into(),
                format!("noise=amount={level}"),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            extension = "mp4".into();
        }
        "encode" => {
            let encoder = param(p, "encoder")?;
            if ![
                "libx264",
                "h264_amf",
                "h264_nvenc",
                "h264_qsv",
                "libx265",
                "hevc_amf",
                "hevc_nvenc",
                "libvpx-vp9",
                "libsvtav1",
                "av1_nvenc",
                "av1_amf",
                "av1_qsv",
            ]
            .contains(&encoder)
            {
                return Err("Unsupported encoder.".into());
            }
            let quality = check_range(parse_number(p, "crf")?, 0.0, 40.0, "Quality")?;
            let requested_pixel_format =
                p.get("pixel_format").map(String::as_str).unwrap_or("auto");
            let source_pixel_format = info.pixel_format.as_deref().unwrap_or("yuv420p");
            let ten_bit_source = source_pixel_format.contains("10")
                || info.bits_per_raw_sample.is_some_and(|bits| bits > 8);
            let pixel_format = match requested_pixel_format {
                "yuv420p" | "yuv420p10le" => requested_pixel_format,
                "source" => source_pixel_format,
                "auto"
                    if ten_bit_source
                        && (encoder.contains("265")
                            || encoder.contains("hevc")
                            || encoder.contains("vp9")
                            || encoder.contains("av1")) =>
                {
                    "yuv420p10le"
                }
                "auto" => "yuv420p",
                _ => return Err("Invalid pixel format.".into()),
            };
            let hardware_h264 = ["h264_amf", "h264_nvenc", "h264_qsv"].contains(&encoder);
            if hardware_h264 && pixel_format != "yuv420p" && pixel_format != "nv12" {
                return Err("The selected H.264 hardware encoder cannot safely produce this pixel format. Choose Compatible 8-bit 4:2:0 or use CPU/HEVC/AV1.".into());
            }
            args.extend([
                "-map".into(),
                "0:v:0".into(),
                "-c:v".into(),
                encoder.into(),
                "-pix_fmt".into(),
                pixel_format.into(),
            ]);
            match encoder {
                "libx264" | "libx265" => args.extend([
                    "-preset".into(),
                    "veryfast".into(),
                    "-crf".into(),
                    quality.to_string(),
                ]),
                "libvpx-vp9" => args.extend([
                    "-crf".into(),
                    quality.to_string(),
                    "-b:v".into(),
                    "0".into(),
                    "-row-mt".into(),
                    "1".into(),
                    "-deadline".into(),
                    "good".into(),
                    "-cpu-used".into(),
                    "4".into(),
                ]),
                "libsvtav1" => args.extend([
                    "-crf".into(),
                    quality.to_string(),
                    "-preset".into(),
                    "8".into(),
                ]),
                "h264_nvenc" | "hevc_nvenc" | "av1_nvenc" => args.extend([
                    "-rc".into(),
                    "vbr".into(),
                    "-cq".into(),
                    quality.to_string(),
                    "-b:v".into(),
                    "0".into(),
                ]),
                "h264_amf" | "hevc_amf" | "av1_amf" => args.extend([
                    "-rc".into(),
                    "cqp".into(),
                    "-qp_i".into(),
                    quality.to_string(),
                    "-qp_p".into(),
                    quality.to_string(),
                ]),
                "h264_qsv" | "av1_qsv" => {
                    args.extend(["-global_quality".into(), quality.to_string()])
                }
                _ => return Err("Unsupported encoder quality mode.".into()),
            }
            append_audio_routing(&mut args, info, p, true, "main")?;
            extension = if encoder.contains("vp9") || encoder.contains("av1") {
                "mkv"
            } else {
                "mp4"
            }
            .into();
        }
        "proxy" => {
            let requested_height = match param(p, "resolution")? {
                "auto" => match info.height.unwrap_or(1080) {
                    0..=719 => 540,
                    720..=1439 => 720,
                    _ => 1080,
                },
                "540" => 540,
                "720" => 720,
                "1080" => 1080,
                _ => return Err("Invalid proxy resolution.".into()),
            };
            let crf = match param(p, "quality")? {
                "edit" => "16",
                "compact" => "22",
                _ => return Err("Invalid proxy quality.".into()),
            };
            let (width, height) = scaled_height(info, requested_height);
            let scale_flags = if discord_is_low_complexity(info) {
                "neighbor"
            } else {
                "lanczos"
            };
            let fps = info.fps.unwrap_or(30.0).clamp(1.0, 120.0);
            let gop = (fps * 2.0).round().max(12.0) as u64;
            args.extend([
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                "0:a?".into(),
                "-vf".into(),
                format!("scale={width}:{height}:flags={scale_flags}"),
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-tune".into(),
                "fastdecode".into(),
                "-crf".into(),
                crf.into(),
                "-g".into(),
                gop.to_string(),
                "-keyint_min".into(),
                gop.to_string(),
                "-sc_threshold".into(),
                "0".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-c:a".into(),
                "copy".into(),
            ]);
            // Matroska accepts copied AAC, Opus, AC-3, FLAC and other common source
            // tracks, so keeping the original audio does not depend on MP4 support.
            extension = "mkv".into();
        }
        "fix_timestamps" => {
            let method = param(p, "method")?;
            args = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-y".into(),
                "-fflags".into(),
                "+genpts+discardcorrupt".into(),
                "-i".into(),
                request.input.clone(),
                "-avoid_negative_ts".into(),
                "make_zero".into(),
            ];
            match method {
                "fast" => {
                    args.extend(["-map".into(), "0".into(), "-c".into(), "copy".into()]);
                    extension = input
                        .extension()
                        .and_then(|value| value.to_str())
                        .filter(|value| !value.is_empty())
                        .unwrap_or(if info.kind == "audio" { "mka" } else { "mkv" })
                        .to_ascii_lowercase();
                }
                "deep" if info.kind == "video" => {
                    args.extend([
                        "-map".into(),
                        "0:v:0".into(),
                        "-map".into(),
                        "0:a?".into(),
                        "-c:v".into(),
                        "libx264".into(),
                        "-preset".into(),
                        "veryfast".into(),
                        "-crf".into(),
                        "18".into(),
                        "-c:a".into(),
                        "aac".into(),
                        "-b:a".into(),
                        "192k".into(),
                    ]);
                    extension = "mp4".into();
                }
                "deep" => {
                    args.extend(["-map".into(), "0:a?".into(), "-c:a".into(), "flac".into()]);
                    extension = "mka".into();
                }
                _ => return Err("Invalid timestamp repair method.".into()),
            }
        }
        "cut" => {
            let start = check_range(
                parse_number(p, "start")?,
                0.0,
                info.duration.unwrap_or(86400.0),
                "Start",
            )?;
            let end = check_range(
                parse_number(p, "end")?,
                0.0,
                info.duration.unwrap_or(86400.0),
                "End",
            )?;
            if end <= start {
                return Err("End must be greater than start.".into());
            }
            let cut_mode = p.get("cut_mode").map(String::as_str).unwrap_or("lossless");
            args = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-y".into(),
                "-ss".into(),
                format!("{start:.6}"),
                "-i".into(),
                request.input.clone(),
                "-t".into(),
                format!("{:.6}", end - start),
                "-map".into(),
                "0:v:0".into(),
            ];
            match cut_mode {
                "exact" => {
                    let crf = p
                        .get("crf")
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(18.0);
                    check_range(crf, 0.0, 30.0, "CRF")?;
                    args.extend([
                        "-c:v".into(),
                        "libx264".into(),
                        "-preset".into(),
                        "veryfast".into(),
                        "-crf".into(),
                        crf.to_string(),
                    ]);
                    append_audio_routing(&mut args, info, p, true, "main")?;
                    extension = "mp4".into();
                }
                "lossless" => {
                    args.extend(["-c:v".into(), "copy".into()]);
                    append_audio_routing(&mut args, info, p, false, "main")?;
                    args.extend(["-avoid_negative_ts".into(), "make_zero".into()]);
                    extension = input
                        .extension()
                        .and_then(|value| value.to_str())
                        .filter(|value| !value.is_empty())
                        .unwrap_or("mkv")
                        .to_ascii_lowercase();
                }
                _ => return Err("Invalid cut mode.".into()),
            }
        }
        "remux" => {
            let format = param(p, "format")?;
            if !["mp4", "mkv", "mov"].contains(&format) {
                return Err("Invalid container.".into());
            }
            args.extend(["-map".into(), "0:v?".into(), "-c:v".into(), "copy".into()]);
            if format == "mkv" {
                args.extend([
                    "-map".into(),
                    "0:s?".into(),
                    "-map".into(),
                    "0:t?".into(),
                    "-map".into(),
                    "0:d?".into(),
                    "-c:s".into(),
                    "copy".into(),
                    "-c:d".into(),
                    "copy".into(),
                ]);
            } else {
                // MP4/MOV cannot carry arbitrary attachments/data. Text subtitles
                // are converted to the container-native mov_text format.
                args.extend([
                    "-map".into(),
                    "0:s?".into(),
                    "-c:s".into(),
                    "mov_text".into(),
                ]);
            }
            args.extend([
                "-map_metadata".into(),
                "0".into(),
                "-map_chapters".into(),
                "0".into(),
            ]);
            append_audio_routing(&mut args, info, p, false, "all")?;
            extension = format.into();
        }
        "screenshot" => {
            let ts = check_range(
                parse_number(p, "timestamp")?,
                0.0,
                info.duration.unwrap_or(86400.0),
                "Timestamp",
            )?;
            let format = param(p, "format")?;
            if !["png", "jpg", "webp"].contains(&format) {
                return Err("Invalid image format.".into());
            }
            args = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-y".into(),
                "-ss".into(),
                format!("{ts:.6}"),
                "-i".into(),
                request.input.clone(),
                "-frames:v".into(),
                "1".into(),
            ];
            if format == "jpg" {
                args.extend(["-q:v".into(), "2".into()]);
            }
            if format == "webp" {
                args.extend([
                    "-c:v".into(),
                    "libwebp".into(),
                    "-quality".into(),
                    "95".into(),
                ]);
            }
            extension = format.into();
        }
        "gif" => {
            let start = check_range(
                parse_number(p, "start")?,
                0.0,
                info.duration.unwrap_or(86400.0),
                "Start",
            )?;
            let duration = check_range(
                parse_number(p, "duration")?,
                0.01,
                info.duration.unwrap_or(86400.0),
                "Duration",
            )?;
            if info
                .duration
                .is_some_and(|total| start + duration > total + 0.01)
            {
                return Err("GIF range exceeds video duration.".into());
            }
            let height = check_range(parse_number(p, "height")?, 2.0, 2160.0, "Height")? as u64;
            let fps = check_range(parse_number(p, "fps")?, 1.0, 60.0, "FPS")?;
            let width = ((height as f64 * info.width.unwrap_or(1920) as f64
                / info.height.unwrap_or(1080) as f64)
                .round() as u64
                + 1)
                / 2
                * 2;
            let max_colors = p
                .get("max_colors")
                .map(String::as_str)
                .unwrap_or("256")
                .parse::<u64>()
                .map_err(|_| "Invalid GIF color count.")?;
            if ![32, 64, 128, 256].contains(&max_colors) {
                return Err("GIF colors must be 32, 64, 128, or 256.".into());
            }
            let palette_mode = p.get("palette_mode").map(String::as_str).unwrap_or("auto");
            let stats_mode = match palette_mode {
                "auto" => "diff",
                "single" => "full",
                "multi" => "single",
                _ => return Err("Invalid GIF palette mode.".into()),
            };
            let dither = match p.get("dither").map(String::as_str).unwrap_or("balanced") {
                "balanced" => "sierra2_4a",
                "sharp" => "floyd_steinberg",
                "small" => "bayer:bayer_scale=5",
                "off" => "none",
                _ => return Err("Invalid GIF dithering mode.".into()),
            };
            let reserve_transparent =
                match p.get("transparency").map(String::as_str).unwrap_or("off") {
                    "off" => 0,
                    "preserve" => 1,
                    _ => return Err("Invalid GIF transparency mode.".into()),
                };
            let palette_new = if palette_mode == "multi" {
                ":new=1"
            } else {
                ""
            };
            let filter = format!(
                "fps={fps},scale={width}:{height}:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode={stats_mode}:max_colors={max_colors}:reserve_transparent={reserve_transparent}[p];[s1][p]paletteuse=dither={dither}{palette_new}"
            );
            let loop_count = p.get("loop").map(String::as_str).unwrap_or("0");
            if !["-1", "0", "2", "3"].contains(&loop_count) {
                return Err("Invalid GIF loop count.".into());
            }
            args = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-y".into(),
                "-ss".into(),
                format!("{start:.6}"),
                "-i".into(),
                request.input.clone(),
                "-t".into(),
                format!("{duration:.6}"),
                "-filter_complex".into(),
                filter,
                "-loop".into(),
                loop_count.into(),
            ];
            extension = "gif".into();
        }
        "remove_audio" => {
            args.extend([
                "-map".into(),
                "0:v:0".into(),
                "-c:v".into(),
                "copy".into(),
                "-an".into(),
            ]);
            extension = "mp4".into();
        }
        "extract_audio" | "audio_convert" => {
            if op == "extract_audio" && info.audio_codec.is_none() {
                return Err("This video has no audio stream.".into());
            }
            if op == "extract_audio" {
                args.push("-vn".into());
                let requested_format = param(p, "format")?;
                append_audio_routing(&mut args, info, p, false, "main")?;
                if requested_format == "copy" {
                    extension = "mka".into();
                } else {
                    let (format, args_audio) = audio_format(requested_format)?;
                    args.extend(args_audio);
                    extension = if p.get("audio_mode").is_some_and(|mode| mode == "all") {
                        "mka".into()
                    } else {
                        format.into()
                    };
                }
            } else {
                let (format, args_audio) = audio_format(param(p, "format")?)?;
                args.extend(args_audio);
                extension = format.into();
            }
        }
        "replace_audio" => {
            let audio = PathBuf::from(param(p, "audio_path")?);
            if !audio.is_file() {
                return Err("Replacement audio was not found.".into());
            }
            args.extend([
                "-i".into(),
                audio.to_string_lossy().into(),
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                "1:a:0".into(),
                "-c:v".into(),
                "copy".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "320k".into(),
                "-shortest".into(),
            ]);
            extension = "mp4".into();
        }
        "distortion" => {
            if info.kind == "video" && info.audio_codec.is_none() {
                return Err("This video has no audio stream.".into());
            }
            let level = check_range(parse_number(p, "level")?, 1.0, 10.0, "Severity")?;
            let gain = level * 10.0;
            let points = [0, 600, 1500, 3000, 6000, 12000, 16000]
                .iter()
                .map(|f| format!("entry({f},{gain})"))
                .collect::<Vec<_>>()
                .join(";");
            if info.kind == "video" {
                args.extend([
                    "-map".into(),
                    "0:v:0".into(),
                    "-map".into(),
                    "0:a:0".into(),
                    "-c:v".into(),
                    "copy".into(),
                ]);
            }
            args.extend([
                "-af".into(),
                format!("firequalizer=gain_entry='{points}'"),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "256k".into(),
            ]);
            extension = if info.kind == "video" { "mp4" } else { "m4a" }.into();
        }
        "image_potatoify" => {
            return Err("image_potatoify_internal".into());
        }
        _ => return Err("Unknown operation.".into()),
    }
    if extension == "mp4" {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    let output = unique_output(&input, op, &extension)?;
    args.push(output.to_string_lossy().to_string());
    Ok((args, output))
}

async fn run_image_potatoify(
    app: Option<&AppHandle>,
    state: &JobState,
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<JobResult, String> {
    let profile = request
        .params
        .get("profile")
        .map(String::as_str)
        .unwrap_or("custom");
    let (quality, times, scale) = match profile {
        "decent" => (3.0, 2_usize, 1.0),
        "bad" => (5.0, 5, 2.0),
        "terrible" => (8.0, 12, 4.0),
        "unbearable" => (10.0, 30, 8.0),
        "random" => {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as u64;
            (
                (2 + seed % 9) as f64,
                (2 + (seed / 29) % 29) as usize,
                (1 + (seed / 53) % 9) as f64,
            )
        }
        "custom" => (
            check_range(
                parse_number(&request.params, "quality")?,
                1.0,
                10.0,
                "Badness",
            )?,
            check_range(parse_number(&request.params, "times")?, 1.0, 100.0, "Times")? as usize,
            check_range(parse_number(&request.params, "scale")?, 1.0, 10.0, "Scale")?,
        ),
        _ => return Err("Invalid Image Potatoify profile.".into()),
    };
    let width = ((info.width.unwrap_or(1920) as f64 / scale) as u64 / 2 * 2).max(2);
    let height = ((info.height.unwrap_or(1080) as f64 / scale) as u64 / 2 * 2).max(2);
    let qscale = (2.0 + (quality - 1.0) * 29.0 / 9.0).round() as u64;
    let input = PathBuf::from(&request.input);
    let output = unique_output(&input, "image_potatoify", "jpg")?;
    let temp = std::env::temp_dir().join(format!("container_image_{}", std::process::id()));
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut current = input.clone();
    for index in 1..=times {
        if state.cancelled.load(Ordering::Relaxed) {
            let _ = std::fs::remove_dir_all(&temp);
            return Err("Job cancelled.".into());
        }
        let next = temp.join(format!("pass_{index:03}.jpg"));
        let status = hidden_command("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&current)
            .args([
                "-frames:v",
                "1",
                "-vf",
                &format!("scale={width}:{height}:flags=neighbor"),
                "-q:v",
                &qscale.to_string(),
                "-pix_fmt",
                "yuvj420p",
            ])
            .arg(&next)
            .status()
            .await
            .map_err(|e| e.to_string())?;
        if !status.success() {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(format!("Image compression failed on pass {index}."));
        }
        current = next;
        if let Some(app) = app {
            let _ = app.emit(
                "container-progress",
                ProgressEvent {
                    percent: index as f64 / times as f64 * 100.0,
                    time: started.elapsed().as_secs_f64(),
                    speed: "—".into(),
                    frame: format!("{index}/{times}"),
                    status: "compressing image".into(),
                },
            );
        }
    }
    std::fs::copy(&current, &output).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&temp);
    if let Some(app) = app {
        allow_asset_file(app, &output)?;
    }
    Ok(JobResult {
        output: output.to_string_lossy().to_string(),
        elapsed: started.elapsed().as_secs_f64(),
    })
}

fn discord_bitrate_plan(
    target_mb: f64,
    duration: f64,
    requested_audio_kbps: u64,
    has_audio: bool,
) -> Result<(u64, u64, u64), String> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err("Video duration is unavailable.".into());
    }
    let target_bytes = (target_mb * 1024.0 * 1024.0).round() as u64;
    // Two-pass output is predictable; reserve four percent for MP4 overhead
    // and let the existing size-check retry handle unusual containers.
    let usable_total_bps = target_bytes as f64 * 0.96 * 8.0 / duration;
    // The user's audio choice is a quality promise. Never silently reduce it to
    // make room for video; reject an impossible target instead.
    let audio_bps = if has_audio {
        requested_audio_kbps * 1000
    } else {
        0
    };
    if usable_total_bps < audio_bps as f64 + 50_000.0 {
        return Err(format!(
            "{target_mb:.0} MB cannot preserve the selected {requested_audio_kbps} kbps audio for a {duration:.1}s video. Choose a larger limit, lower audio bitrate, or a shorter video."
        ));
    }
    let video_bps = (usable_total_bps - audio_bps as f64).floor() as u64;
    Ok((target_bytes, video_bps, audio_bps))
}

fn discord_audio_kbps(choice: &str, usable_total_bps: f64, has_audio: bool) -> Result<u64, String> {
    if !has_audio {
        return Ok(0);
    }
    if choice == "auto" {
        return Ok(if usable_total_bps < 250_000.0 {
            64
        } else if usable_total_bps < 500_000.0 {
            96
        } else {
            128
        });
    }
    let value = choice
        .parse::<u64>()
        .map_err(|_| "Invalid Discord audio bitrate.")?;
    if (32..=320).contains(&value) {
        Ok(value)
    } else {
        Err("Audio bitrate must be between 32 and 320 kbps.".into())
    }
}

fn discord_auto_profile(video_bps: u64) -> (u64, u64) {
    if video_bps < 180_000 {
        (240, 15)
    } else if video_bps < 350_000 {
        (360, 20)
    } else if video_bps < 750_000 {
        (480, 24)
    } else if video_bps < 1_800_000 {
        (720, 30)
    } else if video_bps < 3_500_000 {
        (1080, 30)
    } else {
        (0, 0)
    }
}

fn discord_is_low_complexity(info: &MediaInfo) -> bool {
    match (info.width, info.height, info.fps, info.bitrate) {
        (Some(width), Some(height), Some(fps), Some(bitrate))
            if width >= 1280 && height >= 720 && fps > 0.0 && bitrate > 0.0 =>
        {
            bitrate / (width as f64 * height as f64 * fps) <= 0.015
        }
        _ => false,
    }
}

fn discord_video_filter(
    info: &MediaInfo,
    resolution: &str,
    fps_limit: &str,
    video_bps: u64,
) -> Result<Option<String>, String> {
    let mut filters = Vec::new();
    let (auto_height, auto_fps) = discord_auto_profile(video_bps);
    let box_size = match resolution {
        "auto" if discord_is_low_complexity(info) => None,
        "auto" if auto_height == 240 => Some((426_u64, 240_u64)),
        "auto" if auto_height == 360 => Some((640_u64, 360_u64)),
        "auto" if auto_height == 480 => Some((854_u64, 480_u64)),
        "auto" if auto_height == 720 => Some((1280_u64, 720_u64)),
        "auto" if auto_height == 1080 => Some((1920_u64, 1080_u64)),
        "auto" if auto_height == 0 => None,
        "source" => None,
        "1080" => Some((1920_u64, 1080_u64)),
        "720" => Some((1280_u64, 720_u64)),
        "480" => Some((854_u64, 480_u64)),
        "360" => Some((640_u64, 360_u64)),
        "240" => Some((426_u64, 240_u64)),
        _ => return Err("Invalid Discord resolution limit.".into()),
    };
    if let (Some((mut max_w, mut max_h)), Some(width), Some(height)) =
        (box_size, info.width, info.height)
    {
        if height > width {
            std::mem::swap(&mut max_w, &mut max_h);
        }
        let scale = (max_w as f64 / width as f64)
            .min(max_h as f64 / height as f64)
            .min(1.0);
        if scale < 0.9999 {
            let out_w = (((width as f64 * scale).floor() as u64) / 2 * 2).max(2);
            let out_h = (((height as f64 * scale).floor() as u64) / 2 * 2).max(2);
            filters.push(format!("scale={out_w}:{out_h}:flags=lanczos"));
        }
    }
    match fps_limit {
        "auto" if discord_is_low_complexity(info) => {}
        "auto" if auto_fps > 0 => {
            if info.fps.is_some_and(|fps| fps > auto_fps as f64 + 0.01) {
                filters.push(format!("fps={auto_fps}"));
            }
        }
        "auto" => {}
        "source" => {}
        "60" | "30" | "24" => {
            let limit = fps_limit.parse::<f64>().unwrap_or(30.0);
            if info.fps.is_some_and(|fps| fps > limit + 0.01) {
                filters.push(format!("fps={fps_limit}"));
            }
        }
        _ => return Err("Invalid Discord FPS limit.".into()),
    }
    Ok((!filters.is_empty()).then(|| filters.join(",")))
}

async fn run_ffmpeg_stage(
    app: Option<&AppHandle>,
    state: &JobState,
    mut args: Vec<String>,
    duration: f64,
    started: &Instant,
    base_percent: f64,
    percent_span: f64,
    label: &str,
) -> Result<(), String> {
    let output_index = args.len().saturating_sub(1);
    args.splice(
        output_index..output_index,
        ["-progress".into(), "pipe:1".into(), "-nostats".into()],
    );
    let mut child = hidden_command("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("FFmpeg could not start: {e}"))?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or("FFmpeg progress stream unavailable.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("FFmpeg error stream unavailable.")?;
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut result = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                result.push(line);
            }
        }
        result
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut out_time = 0.0;
    let mut speed = "—".to_string();
    let mut frame = "—".to_string();
    while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        if state.cancelled.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            break;
        }
        if let Some(value) = line.strip_prefix("out_time_us=") {
            out_time = value.parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
        } else if let Some(value) = line.strip_prefix("speed=") {
            speed = value.into();
        } else if let Some(value) = line.strip_prefix("frame=") {
            frame = value.into();
        }
        if line.starts_with("progress=") {
            let local = if duration > 0.0 {
                (out_time / duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if let Some(app) = app {
                let _ = app.emit(
                    "container-progress",
                    ProgressEvent {
                        percent: (base_percent + local * percent_span).clamp(0.0, 99.0),
                        time: started.elapsed().as_secs_f64(),
                        speed: speed.clone(),
                        frame: frame.clone(),
                        status: label.into(),
                    },
                );
            }
        }
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = None;
    let errors = stderr_task.await.unwrap_or_default();
    if state.cancelled.load(Ordering::Relaxed) {
        return Err("Job cancelled.".into());
    }
    if !status.success() {
        let detail = errors
            .into_iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(if detail.is_empty() {
            "FFmpeg operation failed.".into()
        } else {
            detail
        });
    }
    Ok(())
}

fn cleanup_passlog(prefix: &Path) {
    for suffix in [
        "-0.log",
        "-0.log.mbtree",
        "-0.log.cutree",
        ".log",
        ".log.mbtree",
        ".log.cutree",
    ] {
        let _ = std::fs::remove_file(format!("{}{suffix}", prefix.to_string_lossy()));
    }
}

async fn run_discord_compressor(
    app: Option<&AppHandle>,
    state: &JobState,
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<JobResult, String> {
    if info.kind != "video" {
        return Err("Discord Compressor requires a video file.".into());
    }
    let target_mb = check_range(
        parse_number(&request.params, "target_mb")?,
        2.0,
        2000.0,
        "Discord size",
    )?;
    let duration = info.duration.ok_or("Video duration is unavailable.")?;
    let target_bytes = (target_mb * 1024.0 * 1024.0).round() as u64;
    let usable_total_bps = target_bytes as f64 * 0.96 * 8.0 / duration;
    let requested_audio = discord_audio_kbps(
        param(&request.params, "audio_kbps")?,
        usable_total_bps,
        info.audio_codec.is_some(),
    )?;
    let preset = param(&request.params, "preset")?;
    if !["veryfast", "fast", "medium", "slow"].contains(&preset) {
        return Err("Invalid Discord compression speed.".into());
    }
    let (target_bytes, mut video_bps, audio_bps) = discord_bitrate_plan(
        target_mb,
        duration,
        requested_audio,
        info.audio_codec.is_some(),
    )?;
    if let Some(source_bps) = info
        .bitrate
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        video_bps = video_bps.min(source_bps.floor() as u64);
    }
    let codec = param(&request.params, "codec")?;
    let encoder = match codec {
        "h264" => "libx264",
        "hevc" => "libx265",
        _ => return Err("Invalid Discord video codec.".into()),
    };
    let resolution = param(&request.params, "resolution")?.to_string();
    let fps_limit = param(&request.params, "fps_limit")?.to_string();
    let input = PathBuf::from(&request.input);
    let output = unique_output(&input, "discord_compressor", "mp4")?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let passlog = std::env::temp_dir().join(format!(
        "container_discord_{}_{}",
        std::process::id(),
        stamp
    ));
    let started = Instant::now();
    let null_output = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };

    for attempt in 0..3_u64 {
        cleanup_passlog(&passlog);
        let filter = discord_video_filter(info, &resolution, &fps_limit, video_bps)?;
        let retry = if attempt == 0 {
            String::new()
        } else {
            format!(" · retry {attempt}/2")
        };
        let mut common = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-y".into(),
            "-i".into(),
            request.input.clone(),
            "-map".into(),
            "0:v:0".into(),
        ];
        if let Some(value) = &filter {
            common.extend(["-vf".into(), value.clone()]);
        }
        common.extend([
            "-c:v".into(),
            encoder.into(),
            "-preset".into(),
            preset.into(),
            "-b:v".into(),
            video_bps.to_string(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ]);

        let mut pass1 = common.clone();
        pass1.extend([
            "-pass".into(),
            "1".into(),
            "-passlogfile".into(),
            passlog.to_string_lossy().into(),
            "-an".into(),
            "-f".into(),
            "null".into(),
            null_output.into(),
        ]);
        run_ffmpeg_stage(
            app,
            state,
            pass1,
            duration,
            &started,
            0.0,
            48.0,
            &format!("discord pass 1/2{retry}"),
        )
        .await
        .inspect_err(|_| {
            cleanup_passlog(&passlog);
            let _ = std::fs::remove_file(&output);
        })?;

        let mut pass2 = common;
        pass2.extend([
            "-pass".into(),
            "2".into(),
            "-passlogfile".into(),
            passlog.to_string_lossy().into(),
        ]);
        if audio_bps > 0 {
            pass2.extend([
                "-map".into(),
                "0:a:0?".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                audio_bps.to_string(),
            ]);
        } else {
            pass2.push("-an".into());
        }
        if codec == "hevc" {
            pass2.extend(["-tag:v".into(), "hvc1".into()]);
        }
        pass2.extend([
            "-movflags".into(),
            "+faststart".into(),
            output.to_string_lossy().into(),
        ]);
        run_ffmpeg_stage(
            app,
            state,
            pass2,
            duration,
            &started,
            48.0,
            51.0,
            &format!("discord pass 2/2{retry}"),
        )
        .await
        .inspect_err(|_| {
            cleanup_passlog(&passlog);
            let _ = std::fs::remove_file(&output);
        })?;
        cleanup_passlog(&passlog);

        let actual = std::fs::metadata(&output).map_err(|e| e.to_string())?.len();
        if actual <= target_bytes {
            if let Some(app) = app {
                allow_asset_file(app, &output)?;
            }
            return Ok(JobResult {
                output: output.to_string_lossy().into(),
                elapsed: started.elapsed().as_secs_f64(),
            });
        }
        if attempt == 2 {
            let _ = std::fs::remove_file(&output);
            return Err(format!(
                "Output remained above {target_mb:.0} MB after automatic retries."
            ));
        }
        let adjusted = (video_bps as f64 * target_bytes as f64 / actual as f64 * 0.96) as u64;
        let retry_ceiling = video_bps.saturating_sub(1_000).max(50_000);
        video_bps = adjusted.max(50_000).min(retry_ceiling);
        let _ = std::fs::remove_file(&output);
        if let Some(app) = app {
            let _ = app.emit(
                "container-progress",
                ProgressEvent {
                    percent: 0.0,
                    time: started.elapsed().as_secs_f64(),
                    speed: "—".into(),
                    frame: "—".into(),
                    status: "size retry".into(),
                },
            );
        }
    }
    unreachable!()
}

async fn run_quality_capture(
    state: &JobState,
    args: &[String],
) -> Result<std::process::Output, String> {
    let child = hidden_command("ffmpeg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("FFmpeg could not start: {e}"))?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = child.id();
    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = None;
    if state.cancelled.load(Ordering::Relaxed) {
        return Err("Job cancelled.".into());
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(if detail.is_empty() {
            "Quality analysis failed.".into()
        } else {
            detail
        });
    }
    Ok(output)
}

async fn analyze_quality_inner(
    app: Option<&AppHandle>,
    state: &JobState,
    request: QualityAnalysisRequest,
) -> Result<QualityAnalysis, String> {
    state.cancelled.store(false, Ordering::Relaxed);
    let info = probe_media(request.input.clone()).await?;
    if info.kind != "video" {
        return Err("Smart Quality Analysis requires a video file.".into());
    }
    let duration = info.duration.ok_or("Video duration is unavailable.")?;
    let sample_duration =
        check_range(request.sample_duration, 1.0, 3.0, "Sample duration")?.min(duration.max(0.1));
    let target = quality_target(&request.goal)?;
    let positions = quality_sample_positions(duration, sample_duration);
    let (width, height) = quality_dimensions(&info)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp = std::env::temp_dir().join(format!(
        "container_quality_{}_{}",
        std::process::id(),
        stamp
    ));
    std::fs::create_dir_all(&temp)
        .map_err(|e| format!("Analysis folder could not be created: {e}"))?;
    let started = Instant::now();
    let crfs = [16_u64, 20, 24, 28];
    let total_steps = (positions.len() * (1 + crfs.len() * 2)).max(1) as f64;
    let mut completed = 0.0;

    let work = async {
        let mut references = Vec::new();
        for (index, position) in positions.iter().enumerate() {
            let reference = temp.join(format!("reference_{index}.mkv"));
            let args = vec![
                "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
                "-ss".into(), format!("{position:.3}"), "-t".into(), format!("{sample_duration:.3}"),
                "-i".into(), request.input.clone(), "-map".into(), "0:v:0".into(), "-an".into(),
                "-vf".into(), format!("scale={width}:{height}:flags=lanczos,setsar=1,format=yuv420p"),
                "-c:v".into(), "ffv1".into(), reference.to_string_lossy().into(),
            ];
            run_quality_capture(state, &args).await?;
            references.push(reference);
            completed += 1.0;
            if let Some(app) = app {
                let _ = app.emit("container-progress", ProgressEvent {
                    percent: completed / total_steps * 99.0,
                    time: started.elapsed().as_secs_f64(), speed: "—".into(),
                    frame: format!("{}/{}", index + 1, positions.len()), status: "preparing samples".into(),
                });
            }
        }

        let mut candidates = Vec::new();
        for crf in crfs {
            let mut scores = Vec::new();
            let mut sample_bytes = 0_u64;
            for (index, reference) in references.iter().enumerate() {
                let encoded = temp.join(format!("crf_{crf}_{index}.mp4"));
                let encode_args = vec![
                    "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
                    "-i".into(), reference.to_string_lossy().into(), "-an".into(),
                    "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(),
                    "-crf".into(), crf.to_string(), "-pix_fmt".into(), "yuv420p".into(),
                    encoded.to_string_lossy().into(),
                ];
                run_quality_capture(state, &encode_args).await?;
                sample_bytes += std::fs::metadata(&encoded).map_err(|e| e.to_string())?.len();
                completed += 1.0;
                if let Some(app) = app {
                    let _ = app.emit("container-progress", ProgressEvent {
                        percent: completed / total_steps * 99.0,
                        time: started.elapsed().as_secs_f64(), speed: "—".into(),
                        frame: format!("CRF {crf}"), status: "encoding test samples".into(),
                    });
                }

                let null_output = if cfg!(target_os = "windows") { "NUL" } else { "/dev/null" };
                let compare_args = vec![
                    "-hide_banner".into(), "-loglevel".into(), "info".into(),
                    "-i".into(), encoded.to_string_lossy().into(), "-i".into(), reference.to_string_lossy().into(),
                    "-lavfi".into(), "[0:v]setpts=PTS-STARTPTS[dist];[1:v]setpts=PTS-STARTPTS[ref];[dist][ref]libvmaf=n_threads=4".into(),
                    "-f".into(), "null".into(), null_output.into(),
                ];
                let output = run_quality_capture(state, &compare_args).await?;
                let text = String::from_utf8_lossy(&output.stderr);
                let score = text.lines().find_map(|line| line.split("VMAF score:").nth(1))
                    .and_then(|value| value.trim().parse::<f64>().ok())
                    .ok_or("FFmpeg did not return a VMAF score.")?;
                scores.push(score);
                completed += 1.0;
                if let Some(app) = app {
                    let _ = app.emit("container-progress", ProgressEvent {
                        percent: completed / total_steps * 99.0,
                        time: started.elapsed().as_secs_f64(), speed: "—".into(),
                        frame: format!("CRF {crf}"), status: "measuring visual quality".into(),
                    });
                }
            }
            let vmaf = scores.iter().sum::<f64>() / scores.len() as f64;
            let sampled_seconds = sample_duration * positions.len() as f64;
            let estimated_size_mb = sample_bytes as f64 / sampled_seconds * duration / 1_048_576.0;
            candidates.push(QualityCandidate {
                crf, vmaf, estimated_size_mb, rating: quality_rating(vmaf).into(),
            });
        }
        let recommended_crf = recommend_quality_crf(&candidates, target)
            .ok_or("No quality result was produced.")?;
        Ok(QualityAnalysis {
            recommended_crf, target_vmaf: target, candidates,
            sample_count: positions.len(), sampled_seconds: sample_duration * positions.len() as f64,
            elapsed: started.elapsed().as_secs_f64(),
        })
    }.await;
    let _ = std::fs::remove_dir_all(&temp);
    work
}

#[tauri::command]
async fn analyze_quality(
    app: AppHandle,
    state: State<'_, JobState>,
    request: QualityAnalysisRequest,
) -> Result<QualityAnalysis, String> {
    analyze_quality_inner(Some(&app), &state, request).await
}

#[tauri::command]
async fn run_operation(
    app: AppHandle,
    state: State<'_, JobState>,
    request: OperationRequest,
) -> Result<JobResult, String> {
    state.cancelled.store(false, Ordering::Relaxed);
    let info = probe_media(request.input.clone()).await?;
    if request.operation == "image_potatoify" {
        return run_image_potatoify(Some(&app), &state, &request, &info).await;
    }
    if request.operation == "discord_compressor" {
        return run_discord_compressor(Some(&app), &state, &request, &info).await;
    }
    let (mut args, output) = build_command(&request, &info).await?;
    let output_index = args.len() - 1;
    args.splice(
        output_index..output_index,
        ["-progress".into(), "pipe:1".into(), "-nostats".into()],
    );
    let started = Instant::now();
    let mut child = hidden_command("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("FFmpeg could not start: {e}"))?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or("FFmpeg progress stream unavailable.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("FFmpeg error stream unavailable.")?;
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut result = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                result.push(line)
            }
        }
        result
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut out_time = 0.0;
    let mut speed = "—".to_string();
    let mut frame = "—".to_string();
    let duration = info.duration.unwrap_or(0.0);
    while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        if state.cancelled.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            break;
        }
        if let Some(value) = line.strip_prefix("out_time_us=") {
            out_time = value.parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
        } else if let Some(value) = line.strip_prefix("speed=") {
            speed = value.into();
        } else if let Some(value) = line.strip_prefix("frame=") {
            frame = value.into();
        }
        if line.starts_with("progress=") {
            let percent = if duration > 0.0 {
                (out_time / duration * 100.0).clamp(0.0, 99.9)
            } else {
                0.0
            };
            let _ = app.emit(
                "container-progress",
                ProgressEvent {
                    percent,
                    time: started.elapsed().as_secs_f64(),
                    speed: speed.clone(),
                    frame: frame.clone(),
                    status: "rendering".into(),
                },
            );
        }
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    *state.pid.lock().map_err(|_| "Job state lock failed")? = None;
    let errors = stderr_task.await.unwrap_or_default();
    if state.cancelled.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&output);
        return Err("Job cancelled.".into());
    }
    if !status.success() {
        let _ = std::fs::remove_file(&output);
        let detail = errors
            .into_iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(if detail.is_empty() {
            "FFmpeg operation failed.".into()
        } else {
            detail
        });
    }
    allow_asset_file(&app, &output)?;
    Ok(JobResult {
        output: output.to_string_lossy().to_string(),
        elapsed: started.elapsed().as_secs_f64(),
    })
}

#[tauri::command]
async fn cancel_job(state: State<'_, JobState>) -> Result<(), String> {
    state.cancelled.store(true, Ordering::Relaxed);
    let pid = *state.pid.lock().map_err(|_| "Job state lock failed")?;
    if let Some(pid) = pid {
        #[cfg(target_os = "windows")]
        let _ = hidden_command("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()
            .await;
        #[cfg(not(target_os = "windows"))]
        let _ = hidden_command("kill").arg(pid.to_string()).output().await;
    }
    Ok(())
}

#[tauri::command]
fn startup_media_path() -> Option<String> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(JobState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            probe_media,
            ffmpeg_status,
            list_system_fonts,
            available_encoders,
            hash_file,
            list_media_files,
            compute_autocut_waveform,
            compute_video_filmstrip,
            autocut_presets,
            recommend_autocut_settings,
            analyze_autocut,
            export_autocut,
            analyze_quality,
            run_operation,
            cancel_job,
            startup_media_path
        ])
        .setup(|app| {
            if let Some(path) = std::env::args_os()
                .skip(1)
                .map(PathBuf::from)
                .find(|path| path.is_file())
            {
                app.asset_protocol_scope().allow_file(path)?;
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("CONTAINER");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running CONTAINER");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_uses_only_enabled_valid_cuts() {
        let cuts = vec![
            KeepInterval {
                start: 3.0,
                end: 4.0,
                enabled: true,
            },
            KeepInterval {
                start: 1.0,
                end: 2.0,
                enabled: false,
            },
            KeepInterval {
                start: -2.0,
                end: 0.5,
                enabled: true,
            },
            KeepInterval {
                start: 9.0,
                end: 9.001,
                enabled: true,
            },
        ];
        let enabled = enabled_cuts(&cuts, 5.0).unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].start, 0.0);
        assert_eq!(enabled[1].start, 3.0);
    }

    #[test]
    fn export_normalization_merges_duplicate_overlapping_and_touching_ranges() {
        let cuts = vec![
            KeepInterval {
                start: 4.0,
                end: 5.0,
                enabled: true,
            },
            KeepInterval {
                start: 0.0,
                end: 1.0,
                enabled: true,
            },
            KeepInterval {
                start: 0.0,
                end: 1.0,
                enabled: true,
            },
            KeepInterval {
                start: 0.8,
                end: 2.0,
                enabled: true,
            },
            KeepInterval {
                start: 2.0005,
                end: 3.0,
                enabled: true,
            },
            KeepInterval {
                start: 4.2,
                end: 4.8,
                enabled: true,
            },
            KeepInterval {
                start: -5.0,
                end: -1.0,
                enabled: true,
            },
            KeepInterval {
                start: 9.0,
                end: 8.0,
                enabled: true,
            },
        ];
        let normalized = enabled_cuts(&cuts, 8.0).unwrap();
        assert_eq!(normalized.len(), 2);
        assert_eq!((normalized[0].start, normalized[0].end), (0.0, 3.0));
        assert_eq!((normalized[1].start, normalized[1].end), (4.0, 5.0));
        assert!(normalized
            .windows(2)
            .all(|pair| pair[0].end < pair[1].start));
    }

    #[test]
    fn automatic_audio_profile_separates_noise_from_voice() {
        let mut levels = vec![-58.0; 80];
        levels.extend(vec![-14.0; 120]);
        levels.extend(vec![-55.0; 25]);
        levels.extend(vec![-16.0; 100]);
        let recommendation = recommend_from_levels(&levels, 0.02);
        assert!((0.35..=0.65).contains(&recommendation.threshold));
        assert!((0.18..=0.80).contains(&recommendation.min_silence));
        assert!((0.10..=0.30).contains(&recommendation.min_speech));
        assert!((0.35..=0.65).contains(&recommendation.minimum_pause));
        assert_eq!(recommendation.keep_before_speech, 0.10);
        assert_eq!(recommendation.keep_after_speech, 0.18);
        assert!(recommendation.speech_level_db > recommendation.noise_floor_db);
    }

    #[tokio::test]
    async fn real_ffmpeg_audio_is_scored_and_cached_by_vad() {
        let root = std::env::temp_dir().join("container_autocut_detection_test");
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("speech-shape.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=160x90:r=25:d=3",
                "-f",
                "lavfi",
                "-i",
                r"aevalsrc=if(between(t\,1\,2)\,0.5*sin(2*PI*440*t)\,0):s=48000:d=3",
                "-shortest",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let state = JobState::default();
        let scores = cached_vad_scores(&state, &source).await.unwrap();
        assert!(!scores.is_empty());
        let cached = cached_vad_scores(&state, &source).await.unwrap();
        assert_eq!(scores, cached);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn cached_vad_probabilities_are_resegmented_without_model_rerun() {
        let mut scores = vec![0.02; 10];
        scores.extend(vec![0.9; 30]);
        scores.extend(vec![0.01; 10]);
        let keeps = keeps_from_vad_scores(&scores, 0.5, 0.1, 0.1, 0.05, 2.0);
        assert_eq!(keeps.len(), 1);
        assert!(keeps[0].start > 0.2 && keeps[0].start < 0.4);
        assert!(keeps[0].end > 1.2 && keeps[0].end < 1.4);
    }

    #[test]
    fn vad_ignores_isolated_noise_spikes() {
        let mut scores = vec![0.02; 80];
        scores[25] = 0.99;
        scores[55] = 0.90;
        let keeps = keeps_from_vad_scores(&scores, 0.5, 0.25, 0.12, 0.1, 3.0);
        assert!(keeps.is_empty());
    }

    #[test]
    fn vad_keeps_words_together_across_brief_dips() {
        let mut scores = vec![0.02; 10];
        scores.extend(vec![0.88; 20]);
        scores.extend(vec![0.01; 5]);
        scores.extend(vec![0.82; 20]);
        scores.extend(vec![0.01; 12]);
        let keeps = keeps_from_vad_scores(&scores, 0.5, 0.25, 0.12, 0.08, 3.0);
        assert_eq!(keeps.len(), 1);
        assert!(keeps[0].start < 0.35);
        assert!(keeps[0].end > 1.70);
    }

    #[test]
    fn vad_matches_autocut_hysteresis_without_smoothing_or_attack() {
        // AutoCut enters immediately at 0.50, remains active at 0.40, and
        // exits only below 0.35. A one-chunk utterance is kept when the
        // requested minimum speech duration is one chunk.
        let scores = [0.0, 0.50, 0.40, 0.34, 0.0];
        let keeps = keeps_from_vad_scores(&scores, 0.50, 0.0, 0.032, 0.0, 1.0);
        assert_eq!(keeps.len(), 1);
        assert_eq!(keeps[0].start, 0.032);
        assert_eq!(keeps[0].end, 0.096);
    }

    #[test]
    fn vad_uses_autocuts_strict_min_silence_boundary() {
        // With a 64 ms requirement, a two-chunk (64 ms) gap is a real split;
        // AutoCut merges only gaps strictly shorter than the requested value.
        let scores = [0.9, 0.0, 0.0, 0.9];
        let keeps = keeps_from_vad_scores(&scores, 0.5, 0.064, 0.032, 0.0, 1.0);
        assert_eq!(keeps.len(), 2);
        assert_eq!((keeps[0].start, keeps[0].end), (0.0, 0.032));
        assert_eq!((keeps[1].start, keeps[1].end), (0.096, 0.128));
    }

    #[test]
    fn vad_applies_padding_after_segmentation_and_merges_overlap() {
        let scores = [0.9, 0.0, 0.0, 0.9];
        let keeps = keeps_from_vad_scores(&scores, 0.5, 0.032, 0.032, 0.04, 1.0);
        assert_eq!(keeps.len(), 1);
        assert_eq!(keeps[0].start, 0.0);
        assert_eq!(keeps[0].end, 0.168);
    }

    #[test]
    fn strong_two_frame_interjection_survives_min_speech_filter() {
        let scores = [0.0, 0.96, 0.95, 0.0];
        let keeps = keeps_from_vad_scores(&scores, 0.5, 0.1, 0.15, 0.0, 1.0);
        assert_eq!(keeps.len(), 1);
        assert!((keeps[0].end - keeps[0].start - 0.064).abs() < 1e-9);
    }

    #[test]
    fn editing_minimum_pause_preserves_natural_micro_pauses() {
        let mut scores = vec![0.9; 10];
        scores.extend(vec![0.0; 8]);
        scores.extend(vec![0.9; 10]);
        let samples = vec![0_i16; scores.len() * 512];
        let (balanced, _) = natural_keeps_from_vad_scores(
            &scores,
            &samples,
            0.5,
            0.1,
            0.15,
            AutoCutEditSettings {
                minimum_pause: 0.35,
                keep_before_speech: 0.1,
                keep_after_speech: 0.18,
                boundary_refinement: false,
            },
            2.0,
        );
        assert_eq!(balanced.len(), 1, "256 ms pause should remain intact");
    }

    #[test]
    fn asymmetric_padding_uses_after_on_left_and_before_on_right() {
        let mut scores = vec![0.9; 10];
        scores.extend(vec![0.0; 20]);
        scores.extend(vec![0.9; 10]);
        let samples = vec![0_i16; scores.len() * 512];
        let (keeps, _) = natural_keeps_from_vad_scores(
            &scores,
            &samples,
            0.5,
            0.1,
            0.15,
            AutoCutEditSettings {
                minimum_pause: 0.35,
                keep_before_speech: 0.1,
                keep_after_speech: 0.18,
                boundary_refinement: false,
            },
            2.0,
        );
        assert_eq!(keeps.len(), 2);
        assert!((keeps[0].end - (0.320 + 0.180)).abs() < 1e-9);
        assert!((keeps[1].start - (0.960 - 0.100)).abs() < 1e-9);
    }

    #[test]
    fn boundary_refinement_only_expands_toward_safe_quiet_audio() {
        let sample = 8_000;
        let mut samples = vec![0_i16; 12_000];
        for value in &mut samples[sample - 320..sample + 800] {
            *value = 8_000;
        }
        let refined_start = refine_boundary(&samples, sample, -1);
        let refined_end = refine_boundary(&samples, sample, 1);
        assert!(refined_start <= sample);
        assert!(refined_end >= sample + 800);
    }

    #[test]
    fn editing_presets_are_centralized_and_directionally_safe() {
        assert_eq!(AUTOCUT_PRESETS[0].id, "natural");
        assert_eq!(AUTOCUT_PRESETS[1].id, "balanced");
        assert_eq!(AUTOCUT_PRESETS[2].id, "tight");
        for preset in AUTOCUT_PRESETS {
            assert!(preset.keep_after_speech > preset.keep_before_speech);
        }
    }

    #[tokio::test]
    #[ignore = "set CONTAINER_VAD_PARITY_MEDIA to run the real-media parity report"]
    async fn vad_real_media_parity_report() {
        let source = PathBuf::from(
            std::env::var("CONTAINER_VAD_PARITY_MEDIA")
                .expect("CONTAINER_VAD_PARITY_MEDIA must point to a media file"),
        );
        let duration_output = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(&source)
            .output()
            .unwrap();
        assert!(duration_output.status.success());
        let duration = String::from_utf8_lossy(&duration_output.stdout)
            .trim()
            .parse::<f64>()
            .unwrap();
        let state = JobState::default();
        let (scores, samples) = cached_vad_data(&state, &source).await.unwrap();
        let v6 = keeps_from_vad_scores(&scores, 0.5, 0.1, 0.15, 0.3, duration);
        let v6_kept: f64 = v6.iter().map(|keep| keep.end - keep.start).sum();
        eprintln!(
            "V6_PARITY source={} chunks={} regions={} kept={v6_kept:.3}s removed={:.3}s duration={duration:.3}s",
            source.display(),
            scores.len(),
            v6.len(),
            duration - v6_kept
        );
        let balanced_settings = AutoCutEditSettings {
            minimum_pause: 0.35,
            keep_before_speech: 0.10,
            keep_after_speech: 0.18,
            boundary_refinement: true,
        };
        let (balanced, overlaps) = natural_keeps_from_vad_scores(
            &scores,
            &samples,
            0.5,
            0.1,
            0.15,
            balanced_settings,
            duration,
        );
        let balanced_kept: f64 = balanced.iter().map(|keep| keep.end - keep.start).sum();
        eprintln!(
            "BALANCED regions={} cuts={} kept={balanced_kept:.3}s removed={:.3}s threshold=.50 minimum_pause=.350s min_speech=.150s before=.100s after=.180s refinement=true overlaps_before_normalization={overlaps}",
            balanced.len(),
            balanced.len().saturating_sub(1),
            duration - balanced_kept
        );
        assert!(!balanced.is_empty());
        if std::env::var("CONTAINER_PARITY_EXPORT").as_deref() == Ok("1") {
            let expected_duration: f64 = balanced.iter().map(|cut| cut.end - cut.start).sum();
            let fps = 30.0_f64;
            let expected_video_duration: f64 = balanced
                .iter()
                .map(|cut| {
                    let frame_duration = ((cut.end * fps).ceil() - (cut.start * fps).ceil()) / fps;
                    frame_duration.max(cut.end - cut.start)
                })
                .sum();
            let rendered = export_autocut_inner(
                None,
                &state,
                AutoCutExportRequest {
                    input: source.to_string_lossy().into_owned(),
                    cuts: balanced.clone(),
                    format: "mp4".into(),
                    quality: "medium".into(),
                    resolution: "source".into(),
                    linked_tracks: Vec::new(),
                },
            )
            .await
            .unwrap();
            let rendered = PathBuf::from(rendered.output);
            let info = probe_media(rendered.to_string_lossy().into_owned())
                .await
                .unwrap();
            let actual_duration = info.duration.unwrap();
            eprintln!("RENDER_DURATIONS logical={expected_duration:.6} frame_aligned={expected_video_duration:.6} actual={actual_duration:.6}");
            assert!((actual_duration - expected_video_duration).abs() < 0.05);
            let mut output_cursor = 0.0;
            for cut in &balanced {
                let first_frame = (cut.start * fps).ceil();
                let frame_count = ((cut.end * fps).ceil() - first_frame).max(1.0);
                let segment_duration = (frame_count / fps).max(cut.end - cut.start);
                let middle_frame = (frame_count / 2.0).floor();
                let source_signature =
                    sample_video_signature(&source, (first_frame + middle_frame) / fps);
                let output_signature =
                    sample_video_signature(&rendered, output_cursor + middle_frame / fps);
                let mean_error = source_signature
                    .iter()
                    .zip(output_signature.iter())
                    .map(|(left, right)| (*left as f64 - *right as f64).abs())
                    .sum::<f64>()
                    / source_signature.len() as f64;
                assert!(
                    mean_error < 14.0,
                    "source-range order mismatch: MAE={mean_error}"
                );
                output_cursor += segment_duration;
            }
            assert!(packet_timestamps_are_monotonic(&rendered, "v:0"));
            assert!(packet_timestamps_are_monotonic(&rendered, "a:0"));
            eprintln!(
                "RENDER_VALIDATED path={} logical_duration={expected_duration:.3}s frame_aligned_duration={expected_video_duration:.3}s actual_duration={actual_duration:.3}s regions={} video_timestamps=monotonic audio_timestamps=monotonic signatures=matched",
                rendered.display(),
                balanced.len()
            );
        }
    }

    #[test]
    fn atempo_chain_stays_inside_ffmpeg_limits() {
        for speed in [0.05, 0.25, 0.5, 1.0, 2.0, 4.0, 16.0, 100.0] {
            for filter in atempo(speed).split(',') {
                let factor = filter.split('=').nth(1).unwrap().parse::<f64>().unwrap();
                assert!((0.5..=2.0).contains(&factor));
            }
        }
    }

    #[test]
    fn supported_audio_formats_have_expected_extensions() {
        assert_eq!(audio_format("aac").unwrap().0, "m4a");
        assert_eq!(audio_format("mp3").unwrap().0, "mp3");
        assert_eq!(audio_format("wav").unwrap().0, "wav");
        assert_eq!(audio_format("flac").unwrap().0, "flac");
        assert_eq!(audio_format("opus").unwrap().0, "opus");
        assert!(audio_format("invalid").is_err());
    }

    #[test]
    fn drawtext_escapes_windows_paths_and_user_text() {
        assert_eq!(
            drawtext_escape(r"C:\Windows\Fonts\impact.ttf"),
            r"C\:\\Windows\\Fonts\\impact.ttf"
        );
        assert_eq!(
            drawtext_escape("dean's [text], ok"),
            r"dean\'s \[text\]\, ok"
        );
    }

    #[test]
    fn discord_size_plan_reserves_overhead_and_audio_budget() {
        let (target, video, audio) = discord_bitrate_plan(20.0, 60.0, 128, true).unwrap();
        assert_eq!(target, 20 * 1024 * 1024);
        assert_eq!(audio, 128_000);
        assert!(video > 2_000_000);
        let estimated = ((video + audio) as f64 / 8.0 * 60.0) as u64;
        assert!(estimated <= (target as f64 * 0.961) as u64);
        let (_, long_video, long_audio) = discord_bitrate_plan(20.0, 801.0, 128, true).unwrap();
        assert_eq!(long_audio, 128_000);
        assert!((70_000..80_000).contains(&long_video));
        assert!(discord_bitrate_plan(2.0, 3600.0, 192, true).is_err());
    }

    #[test]
    fn discord_filter_only_downscales_and_limits_high_fps() {
        let info = MediaInfo {
            path: "portrait.mp4".into(),
            name: "portrait.mp4".into(),
            kind: "video".into(),
            duration: Some(10.0),
            width: Some(2160),
            height: Some(3840),
            fps: Some(120.0),
            codec: "h264".into(),
            audio_codec: Some("aac".into()),
            audio_tracks: Vec::new(),
            pixel_format: Some("yuv420p".into()),
            bits_per_raw_sample: None,
            color_transfer: None,
            color_primaries: None,
            color_space: None,
            bitrate: None,
            size: 1,
            start_timecode: None,
        };
        let filter = discord_video_filter(&info, "1080", "60", 2_000_000)
            .unwrap()
            .unwrap();
        assert!(filter.contains("scale=1080:1920"));
        assert!(filter.contains("fps=60"));
        let automatic = discord_video_filter(&info, "auto", "auto", 65_000)
            .unwrap()
            .unwrap();
        assert!(automatic.contains("scale=238:426"));
        assert!(automatic.contains("fps=15"));
    }

    #[test]
    fn discord_smart_budget_protects_both_audio_and_picture() {
        assert_eq!(discord_audio_kbps("auto", 190_000.0, true).unwrap(), 64);
        assert_eq!(discord_audio_kbps("auto", 350_000.0, true).unwrap(), 96);
        assert_eq!(discord_audio_kbps("auto", 900_000.0, true).unwrap(), 128);
        assert_eq!(discord_audio_kbps("auto", 190_000.0, false).unwrap(), 0);
        assert_eq!(discord_auto_profile(100_000), (240, 15));
        assert_eq!(discord_auto_profile(250_000), (360, 20));
        assert_eq!(discord_auto_profile(600_000), (480, 24));
        assert_eq!(discord_auto_profile(1_000_000), (720, 30));
    }

    #[test]
    fn discord_keeps_resolution_for_low_complexity_screen_video() {
        let screen = MediaInfo {
            path: "screen.mp4".into(),
            name: "screen.mp4".into(),
            kind: "video".into(),
            duration: Some(800.0),
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            codec: "h264".into(),
            audio_codec: Some("aac".into()),
            audio_tracks: Vec::new(),
            pixel_format: Some("yuv420p".into()),
            bits_per_raw_sample: None,
            color_transfer: None,
            color_primaries: None,
            color_space: None,
            bitrate: Some(340_000.0),
            size: 1,
            start_timecode: None,
        };
        assert!(discord_is_low_complexity(&screen));
        assert_eq!(
            discord_video_filter(&screen, "auto", "auto", 137_000).unwrap(),
            None
        );
    }

    #[test]
    fn menu_operations_route_to_separate_categories() {
        assert_eq!(category("interpolation"), "motion");
        assert_eq!(category("noise"), "effects");
        assert_eq!(category("extract_audio"), "audio");
        assert_eq!(category("image_ratio"), "image");
        assert_eq!(category("image_potatoify"), "image");
        assert_eq!(category("discord_compressor"), "quality");
        assert_eq!(category("smart_quality"), "quality");
        assert_eq!(category("dedupe"), "motion");
        assert_eq!(category("proxy"), "proxy");
        assert_eq!(category("upscale"), "upscale");
    }

    #[test]
    fn every_user_tool_has_an_automated_test_path() {
        let source = include_str!("../../src/lib/tools.ts");
        let mut tool_ids = source
            .lines()
            .filter_map(|line| {
                let rest = line.get(line.find("id:")? + 3..)?.trim_start();
                let quoted = rest.strip_prefix('"')?;
                Some(quoted.split('"').next()?.to_string())
            })
            .collect::<Vec<_>>();
        tool_ids.sort();
        tool_ids.dedup();
        let tested = [
            "audio_convert",
            "bitrate",
            "color",
            "compression",
            "corruption",
            "cut",
            "dedupe",
            "deep_fry",
            "discord_compressor",
            "distortion",
            "encode",
            "extract_audio",
            "file_hash",
            "fix_timestamps",
            "fps",
            "frame_blend",
            "gif",
            "image_potatoify",
            "interpolation",
            "noise",
            "potatoify",
            "proxy",
            "remove_audio",
            "remux",
            "replace_audio",
            "screenshot",
            "speed",
            "text",
            "transform",
            "upscale",
        ];
        assert_eq!(
            tool_ids, tested,
            "Update the real-media smoke coverage when adding a user tool."
        );
    }

    #[test]
    fn quality_analysis_samples_video_across_its_duration() {
        assert_eq!(quality_sample_positions(1.0, 1.0), vec![0.0]);
        assert_eq!(quality_sample_positions(5.0, 2.0), vec![0.0, 3.0]);
        assert_eq!(quality_sample_positions(20.0, 2.0), vec![0.0, 9.0, 18.0]);
    }

    #[test]
    fn quality_analysis_chooses_smallest_crf_that_meets_the_goal() {
        let candidates = vec![
            QualityCandidate {
                crf: 16,
                vmaf: 97.0,
                estimated_size_mb: 50.0,
                rating: "excellent".into(),
            },
            QualityCandidate {
                crf: 20,
                vmaf: 94.0,
                estimated_size_mb: 32.0,
                rating: "very_good".into(),
            },
            QualityCandidate {
                crf: 24,
                vmaf: 91.0,
                estimated_size_mb: 20.0,
                rating: "very_good".into(),
            },
            QualityCandidate {
                crf: 28,
                vmaf: 84.0,
                estimated_size_mb: 12.0,
                rating: "heavy_loss".into(),
            },
        ];
        assert_eq!(recommend_quality_crf(&candidates, 92.0), Some(20));
        assert_eq!(recommend_quality_crf(&candidates, 88.0), Some(24));
        assert_eq!(recommend_quality_crf(&candidates, 99.0), Some(16));
    }

    fn values(items: &[(&str, &str)]) -> HashMap<String, String> {
        items
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect()
    }

    #[tokio::test]
    async fn special_tools_render_real_media() {
        let root = std::env::temp_dir().join("container_special_tools_smoke_test");
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let video = root.join("source.mp4");
        let image = root.join("source.png");
        assert!(std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=320x180:r=25:d=1",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&video)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=120x80",
                "-frames:v",
                "1",
            ])
            .arg(&image)
            .status()
            .unwrap()
            .success());

        let digest = hash_file(image.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest
            .chars()
            .all(|character| character.is_ascii_hexdigit()));

        let state = JobState::default();
        let image_info = probe_media(image.to_string_lossy().into_owned())
            .await
            .unwrap();
        let potato = run_image_potatoify(
            None,
            &state,
            &OperationRequest {
                input: image.to_string_lossy().into_owned(),
                operation: "image_potatoify".into(),
                params: values(&[
                    ("profile", "custom"),
                    ("quality", "5"),
                    ("times", "2"),
                    ("scale", "2"),
                ]),
            },
            &image_info,
        )
        .await
        .unwrap();
        let potato_info = probe_media(potato.output.clone()).await.unwrap();
        assert_eq!(
            (potato_info.width, potato_info.height),
            (Some(60), Some(40))
        );

        let video_info = probe_media(video.to_string_lossy().into_owned())
            .await
            .unwrap();
        let discord = run_discord_compressor(
            None,
            &state,
            &OperationRequest {
                input: video.to_string_lossy().into_owned(),
                operation: "discord_compressor".into(),
                params: values(&[
                    ("target_mb", "2"),
                    ("codec", "h264"),
                    ("resolution", "source"),
                    ("fps_limit", "source"),
                    ("audio_kbps", "64"),
                    ("preset", "veryfast"),
                ]),
            },
            &video_info,
        )
        .await
        .unwrap();
        let discord_path = PathBuf::from(&discord.output);
        assert!(discord_path.is_file());
        assert!(std::fs::metadata(&discord_path).unwrap().len() <= 2 * 1024 * 1024);

        let quality = analyze_quality_inner(
            None,
            &state,
            QualityAnalysisRequest {
                input: video.to_string_lossy().into_owned(),
                goal: "balanced".into(),
                sample_duration: 1.0,
            },
        )
        .await
        .unwrap();
        assert_eq!(quality.candidates.len(), 4);
        assert!(quality
            .candidates
            .iter()
            .all(|candidate| candidate.vmaf.is_finite()));

        let smartcut = export_autocut_inner(
            None,
            &state,
            AutoCutExportRequest {
                input: video.to_string_lossy().into_owned(),
                cuts: vec![
                    KeepInterval {
                        start: 0.0,
                        end: 0.35,
                        enabled: true,
                    },
                    KeepInterval {
                        start: 0.55,
                        end: 0.9,
                        enabled: true,
                    },
                ],
                format: "mp4".into(),
                quality: "medium".into(),
                resolution: "source".into(),
                linked_tracks: Vec::new(),
            },
        )
        .await
        .unwrap();
        let smartcut_info = probe_media(smartcut.output.clone()).await.unwrap();
        assert_eq!(smartcut_info.kind, "video");
        assert!(smartcut_info
            .duration
            .is_some_and(|duration| duration > 0.5 && duration < 0.9));

        let fcpxml = export_autocut_inner(
            None,
            &state,
            AutoCutExportRequest {
                input: video.to_string_lossy().into_owned(),
                cuts: vec![KeepInterval {
                    start: 0.1,
                    end: 0.8,
                    enabled: true,
                }],
                format: "fcpxml".into(),
                quality: "medium".into(),
                resolution: "source".into(),
                linked_tracks: Vec::new(),
            },
        )
        .await
        .unwrap();
        let xml = std::fs::read_to_string(fcpxml.output).unwrap();
        assert!(xml.contains("CONTAINER SmartCut"));
        assert!(xml.contains("duration=\"0.700000s\""));

        std::fs::remove_dir_all(&root).unwrap();
    }

    fn sample_video_rgb(path: &Path, at: f64) -> [u8; 3] {
        let output = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-ss"])
            .arg(format!("{at:.3}"))
            .arg("-i")
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=1:1,format=rgb24",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        [output.stdout[0], output.stdout[1], output.stdout[2]]
    }

    fn sample_video_signature(path: &Path, at: f64) -> Vec<u8> {
        let output = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-ss"])
            .arg(format!("{at:.6}"))
            .arg("-i")
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=16:9,format=rgb24",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        output.stdout
    }

    fn packet_timestamps_are_monotonic(path: &Path, stream: &str) -> bool {
        let output = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                stream,
                "-show_frames",
                "-show_entries",
                "frame=best_effort_timestamp_time",
                "-of",
                "csv=p=0",
            ])
            .arg(path)
            .output()
            .unwrap();
        if !output.status.success() {
            return false;
        }
        let mut previous = f64::NEG_INFINITY;
        for current in String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.trim().parse::<f64>().ok())
        {
            if current + 1e-9 < previous {
                return false;
            }
            previous = current;
        }
        previous.is_finite()
    }

    fn sample_audio_frequency(path: &Path, at: f64) -> f64 {
        let output = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-ss"])
            .arg(format!("{at:.3}"))
            .arg("-i")
            .arg(path)
            .args([
                "-t", "0.35", "-vn", "-ac", "1", "-ar", "16000", "-f", "s16le", "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let samples = output
            .stdout
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let crossings = samples
            .windows(2)
            .filter(|pair| (pair[0] < 0 && pair[1] >= 0) || (pair[0] >= 0 && pair[1] < 0))
            .count();
        crossings as f64 * 16_000.0 / (2.0 * samples.len() as f64)
    }

    #[tokio::test]
    async fn smartcut_export_never_replays_a_source_range() {
        let root = std::env::temp_dir().join("container_smartcut_replay_regression");
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("abc-source.mp4");
        let video_source = "color=c=black:s=160x90:r=30:d=6,drawbox=x=0:y=0:w=iw:h=ih:color=red:t=fill:enable='between(t,0,1)',drawbox=x=0:y=0:w=iw:h=ih:color=green:t=fill:enable='between(t,2,3)',drawbox=x=0:y=0:w=iw:h=ih:color=blue:t=fill:enable='between(t,4,5)'";
        let audio_source = "aevalsrc=exprs='if(between(t,0,1),0.5*sin(2*PI*440*t),if(between(t,2,3),0.5*sin(2*PI*660*t),if(between(t,4,5),0.5*sin(2*PI*880*t),0)))':s=48000:d=6";
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
            ])
            .arg(video_source)
            .args(["-f", "lavfi", "-i"])
            .arg(audio_source)
            .args([
                "-c:v",
                "libx264",
                "-g",
                "180",
                "-keyint_min",
                "180",
                "-sc_threshold",
                "0",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let cuts = vec![
            KeepInterval {
                start: 0.0,
                end: 1.0,
                enabled: true,
            },
            KeepInterval {
                start: 2.0,
                end: 3.0,
                enabled: true,
            },
            KeepInterval {
                start: 4.0,
                end: 5.0,
                enabled: true,
            },
        ];
        // Reproduce the old exporter with two ranges that overlap by 200 ms.
        // The old `enabled_cuts` passed both ranges through and concat appended
        // the shared source time twice, producing a 2.2 s file from a 2.0 s
        // source-time union.
        let legacy_list = root.join("legacy.ffconcat");
        let escaped = source.to_string_lossy().replace('\'', "'\\''");
        std::fs::write(
            &legacy_list,
            format!(
                "ffconcat version 1.0\nfile '{escaped}'\ninpoint 0\noutpoint 1.2\nfile '{escaped}'\ninpoint 1\noutpoint 2\n"
            ),
        )
        .unwrap();
        let legacy = root.join("legacy-export.mp4");
        let legacy_status = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
            ])
            .arg(&legacy_list)
            .args(["-c:v", "libx264", "-c:a", "aac"])
            .arg(&legacy)
            .status()
            .unwrap();
        assert!(legacy_status.success());
        let legacy_info = probe_media(legacy.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert!(legacy_info.duration.is_some_and(|duration| duration > 2.15));

        let rendered = export_autocut_inner(
            None,
            &JobState::default(),
            AutoCutExportRequest {
                input: source.to_string_lossy().into_owned(),
                cuts,
                format: "mp4".into(),
                quality: "medium".into(),
                resolution: "source".into(),
                linked_tracks: Vec::new(),
            },
        )
        .await
        .unwrap();
        let rendered = PathBuf::from(rendered.output);
        let colors = [
            sample_video_rgb(&rendered, 0.5),
            sample_video_rgb(&rendered, 1.5),
            sample_video_rgb(&rendered, 2.5),
        ];
        assert!(colors[0][0] > colors[0][1] * 2 && colors[0][0] > colors[0][2] * 2);
        assert!(colors[1][1] > colors[1][0] * 2 && colors[1][1] > colors[1][2] * 2);
        assert!(colors[2][2] > colors[2][0] * 2 && colors[2][2] > colors[2][1] * 2);
        let frequencies = [
            sample_audio_frequency(&rendered, 0.3),
            sample_audio_frequency(&rendered, 1.3),
            sample_audio_frequency(&rendered, 2.3),
        ];
        for (actual, expected) in frequencies.into_iter().zip([440.0, 660.0, 880.0]) {
            assert!(
                (actual - expected).abs() < 35.0,
                "actual={actual} expected={expected}"
            );
        }
        let rendered_info = probe_media(rendered.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert!(rendered_info
            .duration
            .is_some_and(|duration| (duration - 3.0).abs() < 0.08));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn transform_rotation_and_flip_filters_follow_visible_axes() {
        let root = std::env::temp_dir().join("container_transform_matrix_test");
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let input = root.join("matrix.png");
        let info = MediaInfo {
            path: input.to_string_lossy().into_owned(),
            name: "matrix.png".into(),
            kind: "image".into(),
            duration: None,
            width: Some(320),
            height: Some(180),
            fps: None,
            codec: "png".into(),
            audio_codec: None,
            audio_tracks: Vec::new(),
            pixel_format: Some("rgba".into()),
            bits_per_raw_sample: Some(8),
            color_transfer: None,
            color_primaries: None,
            color_space: None,
            bitrate: None,
            size: 1,
            start_timecode: None,
        };
        for (rotation, flip_h, flip_v, expected) in [
            ("0", "false", "false", None),
            ("0", "true", "true", Some("hflip,vflip")),
            ("90", "true", "false", Some("transpose=clock,hflip")),
            ("180", "false", "false", Some("hflip,vflip")),
            ("270", "false", "true", Some("transpose=cclock,vflip")),
        ] {
            let request = OperationRequest {
                input: input.to_string_lossy().into_owned(),
                operation: "transform".into(),
                params: values(&[
                    ("crop_mode", "off"),
                    ("crop_x", "0"),
                    ("crop_y", "0"),
                    ("crop_w", "100"),
                    ("crop_h", "100"),
                    ("rotate", rotation),
                    ("flip_h", flip_h),
                    ("flip_v", flip_v),
                    ("size_mode", "source"),
                    ("size", "180"),
                    ("output_width", "320"),
                    ("output_height", "180"),
                    ("format", "png"),
                ]),
            };
            let (args, _) = build_command(&request, &info).await.unwrap();
            let filter = args
                .iter()
                .position(|arg| arg == "-vf")
                .map(|index| args[index + 1].as_str());
            assert_eq!(
                filter, expected,
                "rotation={rotation}, h={flip_h}, v={flip_v}"
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn generated_ffmpeg_commands_render_real_test_media() {
        let root = std::env::temp_dir().join("container_studio_smoke_test");
        assert!(root.starts_with(std::env::temp_dir()));
        assert_eq!(
            root.file_name().and_then(|name| name.to_str()),
            Some("container_studio_smoke_test")
        );
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let video = root.join("source.mp4");
        let audio = root.join("source.wav");
        let fixture = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=160x90:r=30:d=0.6",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=0.6",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=660:sample_rate=48000:duration=0.6",
                "-map",
                "0:v:0",
                "-map",
                "1:a:0",
                "-map",
                "2:a:0",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&video)
            .status()
            .unwrap();
        assert!(fixture.success());
        let audio_fixture = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:sample_rate=48000:duration=0.6",
            ])
            .arg(&audio)
            .status()
            .unwrap();
        assert!(audio_fixture.success());
        let video_info = probe_media(video.to_string_lossy().to_string())
            .await
            .unwrap();
        let audio_text = audio.to_string_lossy().to_string();

        let operations: Vec<(&str, HashMap<String, String>)> = vec![
            (
                "transform",
                values(&[
                    ("crop_mode", "free"),
                    ("crop_x", "10"),
                    ("crop_y", "10"),
                    ("crop_w", "80"),
                    ("crop_h", "80"),
                    ("rotate", "90"),
                    ("flip_h", "true"),
                    ("flip_v", "false"),
                    ("size_mode", "height"),
                    ("size", "180"),
                    ("output_width", "320"),
                    ("output_height", "180"),
                ]),
            ),
            ("upscale", values(&[("target_edge", "720")])),
            ("ratio", values(&[("ratio", "1:1")])),
            (
                "resize",
                values(&[("axis", "height"), ("size", "180"), ("crf", "20")]),
            ),
            ("fps", values(&[("fps", "24"), ("crf", "20")])),
            ("interpolation", values(&[("fps", "60")])),
            ("frame_blend", values(&[("fps", "24")])),
            ("dedupe", values(&[("profile", "safe")])),
            ("speed", values(&[("speed", "2"), ("crf", "20")])),
            (
                "speed",
                values(&[
                    ("speed", "1.5"),
                    ("speed_mode", "lossless_video"),
                    ("crf", "20"),
                ]),
            ),
            (
                "compression",
                values(&[("crf", "22"), ("preset", "ultrafast")]),
            ),
            ("bitrate", values(&[("mbps", "0.3")])),
            (
                "potatoify",
                values(&[
                    ("profile", "custom"),
                    ("fps", "12"),
                    ("video_badness", "5"),
                    ("audio_badness", "5"),
                    ("shrink", "2"),
                ]),
            ),
            ("potatoify", values(&[("profile", "decent")])),
            (
                "text",
                values(&[(
                    "layers",
                    r##"[{"text":"CONTAINER","x":50,"y":50,"size":24,"color":"#ffffff","opacity":80,"outline":2,"outline_color":"#000000","shadow":3,"shadow_color":"#000000","background":true,"background_color":"#102030","background_opacity":55,"background_padding":8}]"##,
                )]),
            ),
            (
                "color",
                values(&[
                    ("brightness", "2"),
                    ("brightness_enabled", "true"),
                    ("contrast", "108"),
                    ("contrast_enabled", "true"),
                    ("saturation", "110"),
                    ("saturation_enabled", "true"),
                    ("gamma", "100"),
                    ("gamma_enabled", "false"),
                    ("hue", "0"),
                    ("hue_enabled", "false"),
                    ("temperature", "6500"),
                    ("temperature_enabled", "false"),
                    ("sharpen", "25"),
                    ("sharpen_enabled", "false"),
                    ("blur", "20"),
                    ("blur_enabled", "false"),
                    ("denoise", "off"),
                    ("deband", "25"),
                    ("deband_enabled", "false"),
                    ("vignette", "35"),
                    ("vignette_enabled", "false"),
                    ("grayscale", "off"),
                    ("deinterlace", "off"),
                ]),
            ),
            ("noise", values(&[("amount", "3")])),
            ("negate", values(&[])),
            ("deep_fry", values(&[("level", "2")])),
            ("corruption", values(&[("level", "1")])),
            ("encode", values(&[("encoder", "libx264"), ("crf", "20")])),
            (
                "encode",
                values(&[
                    ("encoder", "libx264"),
                    ("crf", "22"),
                    ("pixel_format", "yuv420p"),
                    ("audio_mode", "merge"),
                ]),
            ),
            (
                "proxy",
                values(&[("resolution", "auto"), ("quality", "compact")]),
            ),
            ("fix_timestamps", values(&[("method", "fast")])),
            ("fix_timestamps", values(&[("method", "deep")])),
            ("cut", values(&[("start", "0.1"), ("end", "0.4")])),
            (
                "cut",
                values(&[("start", "0.1"), ("end", "0.4"), ("cut_mode", "exact")]),
            ),
            (
                "cut",
                values(&[
                    ("start", "0.1"),
                    ("end", "0.4"),
                    ("cut_mode", "lossless"),
                    ("audio_mode", "all"),
                ]),
            ),
            ("remux", values(&[("format", "mkv")])),
            (
                "screenshot",
                values(&[("timestamp", "0.1"), ("format", "png")]),
            ),
            (
                "gif",
                values(&[
                    ("start", "0"),
                    ("duration", "0.3"),
                    ("height", "90"),
                    ("fps", "10"),
                    ("max_colors", "64"),
                    ("palette_mode", "multi"),
                    ("dither", "small"),
                    ("transparency", "off"),
                    ("loop", "-1"),
                ]),
            ),
            ("cfr", values(&[("fps", "30")])),
            ("remove_audio", values(&[])),
            ("extract_audio", values(&[("format", "mp3")])),
            (
                "extract_audio",
                values(&[("format", "copy"), ("audio_mode", "all")]),
            ),
            (
                "replace_audio",
                values(&[("audio_path", audio_text.as_str())]),
            ),
            ("distortion", values(&[("level", "1")])),
        ];

        for (operation, params) in operations {
            let lossless_cut = operation == "cut"
                && params
                    .get("cut_mode")
                    .map(String::as_str)
                    .unwrap_or("lossless")
                    == "lossless";
            let request = OperationRequest {
                input: video.to_string_lossy().to_string(),
                operation: operation.into(),
                params,
            };
            let (args, output) = build_command(&request, &video_info)
                .await
                .unwrap_or_else(|error| panic!("{operation}: {error}"));
            if lossless_cut {
                assert!(args.windows(2).any(|pair| pair == ["-c:v", "copy"]));
                assert!(!args.iter().any(|arg| arg == "libx264"));
                let seek = args.iter().position(|arg| arg == "-ss").unwrap();
                let input = args.iter().position(|arg| arg == "-i").unwrap();
                assert!(
                    seek < input,
                    "fast input seeking must happen before opening the media"
                );
            }
            let status = std::process::Command::new("ffmpeg")
                .args(&args)
                .status()
                .unwrap();
            assert!(status.success(), "operation failed: {operation}");
            assert!(output.is_file(), "output missing: {operation}");
            assert!(
                std::fs::metadata(&output).unwrap().len() > 0,
                "empty output: {operation}"
            );
            probe_media(output.to_string_lossy().into_owned())
                .await
                .unwrap_or_else(|error| panic!("unreadable output for {operation}: {error}"));
        }

        let audio_info = probe_media(audio.to_string_lossy().to_string())
            .await
            .unwrap();
        let request = OperationRequest {
            input: audio.to_string_lossy().to_string(),
            operation: "audio_convert".into(),
            params: values(&[("format", "flac")]),
        };
        let (args, output) = build_command(&request, &audio_info).await.unwrap();
        assert!(std::process::Command::new("ffmpeg")
            .args(&args)
            .status()
            .unwrap()
            .success());
        assert!(output.is_file());
        assert_eq!(
            probe_media(output.to_string_lossy().into_owned())
                .await
                .unwrap()
                .kind,
            "audio"
        );

        let image = root.join("source.png");
        assert!(std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=120x80",
                "-frames:v",
                "1",
            ])
            .arg(&image)
            .status()
            .unwrap()
            .success());
        let image_info = probe_media(image.to_string_lossy().to_string())
            .await
            .unwrap();
        for (ratio, format) in [("4:5", "png"), ("191:100", "jpg")] {
            let request = OperationRequest {
                input: image.to_string_lossy().to_string(),
                operation: "image_ratio".into(),
                params: values(&[("ratio", ratio), ("format", format)]),
            };
            let (args, output) = build_command(&request, &image_info).await.unwrap();
            assert!(std::process::Command::new("ffmpeg")
                .args(&args)
                .status()
                .unwrap()
                .success());
            assert!(output.is_file());
            assert_eq!(
                probe_media(output.to_string_lossy().into_owned())
                    .await
                    .unwrap()
                    .kind,
                "image"
            );
        }
        for format in ["png", "webp"] {
            let request = OperationRequest {
                input: image.to_string_lossy().to_string(),
                operation: "transform".into(),
                params: values(&[
                    ("crop_mode", "free"),
                    ("crop_x", "10"),
                    ("crop_y", "10"),
                    ("crop_w", "80"),
                    ("crop_h", "80"),
                    ("rotate", "90"),
                    ("flip_h", "true"),
                    ("flip_v", "false"),
                    ("size_mode", "exact"),
                    ("size", "100"),
                    ("output_width", "80"),
                    ("output_height", "100"),
                    ("format", format),
                ]),
            };
            let (args, output) = build_command(&request, &image_info).await.unwrap();
            assert!(std::process::Command::new("ffmpeg")
                .args(&args)
                .status()
                .unwrap()
                .success());
            assert!(output.is_file());
            assert_eq!(
                probe_media(output.to_string_lossy().into_owned())
                    .await
                    .unwrap()
                    .kind,
                "image"
            );
        }

        std::fs::remove_dir_all(&root).unwrap();
    }
}
