//! Shared request types, parameter checks, audio/image encoder arguments and FFmpeg stage runner.

use crate::*;

#[derive(Debug, Deserialize)]
pub(crate) struct OperationRequest {
    pub(crate) input: String,
    pub(crate) operation: String,
    pub(crate) params: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TextLayer {
    pub(crate) text: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) size: f64,
    pub(crate) color: String,
    pub(crate) opacity: f64,
    #[serde(default = "default_text_alignment")]
    pub(crate) align: String,
    #[serde(default)]
    pub(crate) font_path: String,
    #[serde(default)]
    pub(crate) outline: f64,
    #[serde(default)]
    pub(crate) outline_color: String,
    #[serde(default)]
    pub(crate) shadow: f64,
    #[serde(default)]
    pub(crate) shadow_color: String,
    #[serde(default)]
    pub(crate) background: bool,
    #[serde(default)]
    pub(crate) background_color: String,
    #[serde(default)]
    pub(crate) background_opacity: f64,
    #[serde(default)]
    pub(crate) background_padding: f64,
}

pub(crate) fn default_text_alignment() -> String {
    "center".into()
}

#[derive(Debug, Serialize)]
pub(crate) struct JobResult {
    pub(crate) output: String,
    pub(crate) elapsed: f64,
}

#[derive(Clone, Serialize)]
pub(crate) struct ProgressEvent {
    pub(crate) job_id: Option<String>,
    pub(crate) percent: f64,
    pub(crate) time: f64,
    pub(crate) speed: String,
    pub(crate) frame: String,
    pub(crate) status: String,
}

pub(crate) fn operation_progress_duration(request: &OperationRequest, source: f64) -> f64 {
    let number = |key: &str| {
        request
            .params
            .get(key)
            .and_then(|value| value.parse::<f64>().ok())
    };
    let output = match request.operation.as_str() {
        "cut" | "gif" => number("end")
            .zip(number("start"))
            .map(|(end, start)| end - start),
        "speed" => number("speed").map(|speed| source / speed),
        _ => None,
    };
    output
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(source)
}

#[test]
pub(crate) fn output_duration_drives_progress() {
    let mut request = OperationRequest {
        input: String::new(),
        operation: "cut".into(),
        params: HashMap::from([("start".into(), "60".into()), ("end".into(), "90".into())]),
    };
    assert_eq!(operation_progress_duration(&request, 600.0), 30.0);
    request.operation = "speed".into();
    request.params.insert("speed".into(), "2".into());
    assert_eq!(operation_progress_duration(&request, 600.0), 300.0);
    request.params.insert("speed".into(), "0".into());
    assert_eq!(operation_progress_duration(&request, 600.0), 600.0);
}

pub(crate) fn param<'a>(params: &'a HashMap<String, String>, key: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("Missing parameter: {key}"))
}

pub(crate) fn enabled(params: &HashMap<String, String>, key: &str) -> bool {
    params.get(key).is_some_and(|value| value == "true")
}

pub(crate) fn check_range(value: f64, min: f64, max: f64, name: &str) -> Result<f64, String> {
    if value.is_finite() && value >= min && value <= max {
        Ok(value)
    } else {
        Err(format!("{name} must be between {min} and {max}."))
    }
}

pub(crate) fn check_cut_end(value: f64, duration: f64) -> Result<f64, String> {
    // The timeline displays milliseconds, while ffprobe may report a finer
    // duration (for example 100.05988 s). Accept only that rounding gap.
    if value.is_finite() && value > duration && value - duration <= 0.001 {
        return Ok(duration);
    }
    check_range(value, 0.0, duration, "End")
}

pub(crate) fn atempo(speed: f64) -> String {
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

pub(crate) fn audio_format(format: &str) -> Result<(&'static str, Vec<String>), String> {
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

pub(crate) fn append_audio_routing(
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

pub(crate) fn mp4_audio_stream_copy_supported(info: &MediaInfo) -> bool {
    let supported = |codec: &str| matches!(codec, "aac" | "mp3" | "ac3" | "eac3" | "alac");
    if !info.audio_tracks.is_empty() {
        info.audio_tracks
            .iter()
            .all(|track| supported(track.codec.as_str()))
    } else {
        info.audio_codec.as_deref().is_none_or(supported)
    }
}

pub(crate) fn append_mp4_audio_codec(args: &mut Vec<String>, info: &MediaInfo) {
    if info.audio_codec.is_none() && info.audio_tracks.is_empty() {
        return;
    }
    if mp4_audio_stream_copy_supported(info) {
        args.extend(["-c:a".into(), "copy".into()]);
    } else {
        args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
    }
}

pub(crate) fn image_extension(input: &Path) -> String {
    match input
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpeg" | "jpg" => "jpg".into(),
        "png" => "png".into(),
        "webp" => "webp".into(),
        "bmp" => "bmp".into(),
        "tif" | "tiff" => "tiff".into(),
        "avif" => "avif".into(),
        _ => "png".into(),
    }
}

pub(crate) fn image_has_alpha(info: &MediaInfo) -> bool {
    info.pixel_format.as_deref().is_some_and(|format| {
        format.contains("rgba")
            || format.contains("bgra")
            || format.contains("argb")
            || format.contains("abgr")
            || format.starts_with("yuva")
            || format.starts_with("gbrap")
            || format == "pal8"
    })
}

pub(crate) fn parse_hex_color(value: &str, name: &str) -> Result<(u8, u8, u8), String> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{name} must be a six-digit hex color such as #ffffff."
        ));
    }
    let component = |offset| u8::from_str_radix(&value[offset..offset + 2], 16).unwrap_or(0);
    Ok((component(0), component(2), component(4)))
}

pub(crate) fn alpha_flatten_filter(color: &str) -> Result<String, String> {
    let (r, g, b) = parse_hex_color(color, "JPEG background")?;
    Ok(format!(
        "format=rgba,geq=r='r(X,Y)*alpha(X,Y)/255+{r}*(1-alpha(X,Y)/255)':g='g(X,Y)*alpha(X,Y)/255+{g}*(1-alpha(X,Y)/255)':b='b(X,Y)*alpha(X,Y)/255+{b}*(1-alpha(X,Y)/255)':a=255,format=rgb24"
    ))
}

pub(crate) fn append_image_encoder(
    args: &mut Vec<String>,
    extension: &str,
    quality: u8,
) -> Result<(), String> {
    match extension {
        "png" => args.extend([
            "-c:v".into(),
            "png".into(),
            "-compression_level".into(),
            "9".into(),
        ]),
        "jpg" => {
            let q = (31.0 - f64::from(quality.clamp(1, 100)) * 29.0 / 100.0)
                .round()
                .clamp(2.0, 31.0) as u8;
            args.extend(["-c:v".into(), "mjpeg".into(), "-q:v".into(), q.to_string()]);
        }
        "webp" => {
            args.extend(["-c:v".into(), "libwebp".into()]);
            if quality >= 100 {
                args.extend(["-lossless".into(), "1".into()]);
            } else {
                args.extend(["-quality".into(), quality.clamp(1, 100).to_string()]);
            }
        }
        "bmp" => args.extend(["-c:v".into(), "bmp".into()]),
        "tiff" => args.extend(["-c:v".into(), "tiff".into()]),
        "avif" => {
            let crf = (63.0 - f64::from(quality.clamp(1, 100)) * 55.0 / 100.0)
                .round()
                .clamp(8.0, 62.0) as u8;
            args.extend([
                "-c:v".into(),
                "libaom-av1".into(),
                "-still-picture".into(),
                "1".into(),
                "-crf".into(),
                crf.to_string(),
                "-cpu-used".into(),
                "6".into(),
            ]);
        }
        _ => return Err("Unsupported image output format.".into()),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_ffmpeg_stage(
    app: Option<&AppHandle>,
    state: &JobState,
    args: Vec<String>,
    duration: f64,
    started: &Instant,
    base_percent: f64,
    percent_span: f64,
    label: &str,
) -> Result<(), String> {
    match run_ffmpeg_stage_attempt(
        app,
        state,
        args.clone(),
        duration,
        started,
        base_percent,
        percent_span,
        label,
    )
    .await
    {
        Err(error) if !state.cancelled.load(Ordering::Relaxed) => {
            if let Some(retry) = auto_encoder::cpu_retry_args(&args, &error) {
                // One retry only; use the same reserved output and preserve
                // the original inputs, graph, frame rate and audio settings.
                run_ffmpeg_stage_attempt(
                    app,
                    state,
                    retry,
                    duration,
                    started,
                    base_percent,
                    percent_span,
                    label,
                )
                .await
            } else {
                Err(error)
            }
        }
        result => result,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_ffmpeg_stage_attempt(
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
    state.inspector.start(&args);
    let mut child = hidden_command("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("FFmpeg could not start: {e}"))?;
    let active_process = ActiveProcess::register(state, child.id())?;
    let stdout = child
        .stdout
        .take()
        .ok_or("FFmpeg progress stream unavailable.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("FFmpeg error stream unavailable.")?;
    let stderr_task = line_tail::spawn_tail(stderr);
    let mut lines = BufReader::new(stdout).lines();
    let mut out_time = 0.0;
    let mut speed = "—".to_string();
    let mut frame = "—".to_string();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                let _ = child.kill().await;
                return Err(error.to_string());
            }
        };
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
                        job_id: progress_job_id(state),
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
    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => return Err(error.to_string()),
    };
    drop(active_process);
    let errors = stderr_task.await.unwrap_or_default();
    state.inspector.errors(&errors);
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

pub(crate) fn operation_temp_dir(name: &str) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("container_{name}_{}_{stamp}", std::process::id()));
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

pub(crate) struct OperationTempGuard(pub(crate) PathBuf);

impl Drop for OperationTempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) async fn require_filters(names: &[&str], feature: &str) -> Result<(), String> {
    let filters = ffmpeg_filter_names().await?;
    let missing = names
        .iter()
        .filter(|name| !filter_list_has(&filters, name))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{feature} is unavailable because this FFmpeg build is missing: {}.",
            missing.join(", ")
        ))
    }
}

pub(crate) async fn require_demuxers(names: &[&str], feature: &str) -> Result<(), String> {
    let demuxers = ffmpeg_demuxer_names().await?;
    let missing = names
        .iter()
        .filter(|name| !demuxer_list_has(&demuxers, name))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{feature} is unavailable because this FFmpeg build is missing: {} demuxer.",
            missing.join(", ")
        ))
    }
}
