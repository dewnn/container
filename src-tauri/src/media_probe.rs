//! ffprobe-based media inspection and media file discovery.

use crate::*;

#[derive(Debug, Serialize)]
pub(crate) struct AudioTrackInfo {
    pub(crate) index: u64,
    pub(crate) codec: String,
    pub(crate) channels: Option<u64>,
    pub(crate) channel_layout: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) bitrate: Option<f64>,
    pub(crate) is_default: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct MediaInfo {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) duration: Option<f64>,
    pub(crate) width: Option<u64>,
    pub(crate) height: Option<u64>,
    pub(crate) fps: Option<f64>,
    pub(crate) codec: String,
    pub(crate) audio_codec: Option<String>,
    pub(crate) audio_tracks: Vec<AudioTrackInfo>,
    pub(crate) pixel_format: Option<String>,
    pub(crate) bits_per_raw_sample: Option<u64>,
    pub(crate) color_transfer: Option<String>,
    pub(crate) color_primaries: Option<String>,
    pub(crate) color_space: Option<String>,
    pub(crate) bitrate: Option<f64>,
    pub(crate) size: u64,
    pub(crate) start_timecode: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SubtitleTrackInfo {
    pub(crate) index: u64,
    pub(crate) codec: String,
    pub(crate) language: Option<String>,
    pub(crate) title: Option<String>,
}

pub(crate) fn parse_rate(text: Option<&str>) -> Option<f64> {
    let text = text?;
    let mut pieces = text.split('/');
    let numerator = pieces.next()?.parse::<f64>().ok()?;
    let denominator = pieces.next().unwrap_or("1").parse::<f64>().ok()?;
    (denominator != 0.0).then_some(numerator / denominator)
}

pub(crate) fn parse_number(params: &HashMap<String, String>, key: &str) -> Result<f64, String> {
    params
        .get(key)
        .ok_or_else(|| format!("Missing parameter: {key}"))?
        .parse::<f64>()
        .map_err(|_| format!("Invalid number for {key}"))
}

#[tauri::command]
pub(crate) async fn probe_media(path: String) -> Result<MediaInfo, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err("Input file was not found.".into());
    }
    let output = process_timeout::output_within(
        hidden_command("ffprobe")
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                "-show_stream_groups",
            ])
            .arg(&input),
        process_timeout::PROBE,
    )
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
    let kind = classify_media_kind(&extension, video.is_some(), audio.is_some())?;
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
    let primary_dimensions = if kind == "image" {
        primary_tile_grid_display_dimensions(&data)
    } else {
        None
    };
    Ok(MediaInfo {
        path: input.to_string_lossy().to_string(),
        name: input
            .file_name()
            .and_then(|part| part.to_str())
            .unwrap_or("media")
            .to_string(),
        kind: kind.to_string(),
        duration,
        width: primary_dimensions
            .map(|dimensions| dimensions.0)
            .or_else(|| primary["width"].as_u64()),
        height: primary_dimensions
            .map(|dimensions| dimensions.1)
            .or_else(|| primary["height"].as_u64()),
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
pub(crate) fn authorize_media_preview(app: AppHandle, path: String) -> Result<(), String> {
    let media = PathBuf::from(path);
    if !media.is_file() {
        return Err("Preview media file was not found.".into());
    }
    allow_asset_file(&app, &media)
}

pub(crate) fn primary_tile_grid_display_dimensions(data: &Value) -> Option<(u64, u64)> {
    let groups = data["stream_groups"].as_array()?;
    let group = groups
        .iter()
        .filter(|group| group["type"].as_str() == Some("Tile Grid"))
        .max_by_key(|group| {
            let default = group["disposition"]["default"].as_u64().unwrap_or(0);
            let component = group["components"]
                .as_array()
                .and_then(|items| items.first());
            let area = component
                .and_then(|item| item["width"].as_u64().zip(item["height"].as_u64()))
                .map_or(0, |(width, height)| width.saturating_mul(height));
            (default, area)
        })?;
    let component = group["components"].as_array()?.first()?;
    let mut width = component["width"].as_u64()?;
    let mut height = component["height"].as_u64()?;
    let rotation = component["side_data_list"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|item| item["rotation"].as_i64())
        .unwrap_or(0)
        .rem_euclid(360);
    if matches!(rotation, 90 | 270) {
        std::mem::swap(&mut width, &mut height);
    }
    Some((width, height))
}

pub(crate) fn classify_media_kind(
    extension: &str,
    has_video_stream: bool,
    has_audio_stream: bool,
) -> Result<&'static str, String> {
    // FFprobe exposes the primary item in still-image containers such as
    // HEIC/HEIF as a video stream, so the known image container wins here.
    const IMAGE_EXTENSIONS: &[&str] = &[
        "jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "avif", "heic", "heif",
    ];
    if IMAGE_EXTENSIONS.contains(&extension) {
        Ok("image")
    } else if has_video_stream {
        Ok("video")
    } else if has_audio_stream {
        Ok("audio")
    } else {
        Err("Unsupported media file.".into())
    }
}

#[tauri::command]
pub(crate) async fn probe_subtitles(path: String) -> Result<Vec<SubtitleTrackInfo>, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err("Input file was not found.".into());
    }
    let output = process_timeout::output_within(
        hidden_command("ffprobe")
            .args(["-v", "error", "-print_format", "json", "-show_streams"])
            .arg(&input),
        process_timeout::PROBE,
    )
    .await
    .map_err(|error| format!("ffprobe could not start: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let data: Value = serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    Ok(data["streams"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|stream| stream["codec_type"] == "subtitle")
        .map(|stream| SubtitleTrackInfo {
            index: stream["index"].as_u64().unwrap_or(0),
            codec: stream["codec_name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            language: stream["tags"]["language"].as_str().map(str::to_string),
            title: stream["tags"]["title"].as_str().map(str::to_string),
        })
        .collect())
}

pub(crate) async fn display_dimensions(
    path: &Path,
    fallback: &MediaInfo,
) -> Result<(u64, u64), String> {
    let output = process_timeout::output_within(
        hidden_command("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-print_format",
                "json",
                "-show_streams",
            ])
            .arg(path),
        process_timeout::PROBE,
    )
    .await
    .map_err(|error| format!("ffprobe could not start: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let data: Value = serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let stream = data["streams"]
        .as_array()
        .and_then(|items| items.first())
        .ok_or("Video geometry is unavailable.")?;
    let mut width = stream["width"]
        .as_u64()
        .or(fallback.width)
        .ok_or("Video width is unavailable.")?;
    let mut height = stream["height"]
        .as_u64()
        .or(fallback.height)
        .ok_or("Video height is unavailable.")?;
    let rotation = stream["side_data_list"]
        .as_array()
        .and_then(|items| items.iter().find_map(|item| item["rotation"].as_i64()))
        .or_else(|| {
            stream["tags"]["rotate"]
                .as_str()
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0)
        .rem_euclid(360);
    if matches!(rotation, 90 | 270) {
        std::mem::swap(&mut width, &mut height);
    }
    Ok((width, height))
}

#[tauri::command]
pub(crate) async fn hash_file(path: String) -> Result<String, String> {
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

pub(crate) fn collect_media_files(
    folder: &Path,
    recursive: bool,
    files: &mut Vec<String>,
) -> Result<(), String> {
    const EXTENSIONS: &[&str] = &[
        "mp4", "mkv", "mov", "avi", "webm", "m4v", "mp3", "wav", "m4a", "aac", "flac", "opus",
        "ogg", "jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "avif",
    ];
    for entry in std::fs::read_dir(folder).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        let linked_directory = metadata.file_type().is_symlink() || {
            #[cfg(target_os = "windows")]
            {
                metadata.file_attributes() & 0x400 != 0
            }
            #[cfg(not(target_os = "windows"))]
            {
                false
            }
        };
        if path.is_dir() && recursive && !linked_directory {
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
pub(crate) async fn list_media_files(
    folder: String,
    recursive: bool,
) -> Result<Vec<String>, String> {
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

pub(crate) fn parse_smpte(value: Option<&str>, fps: f64) -> f64 {
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
