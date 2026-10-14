//! Discord Compressor (size-targeted two-pass encoding).

use crate::*;

pub(crate) fn discord_bitrate_plan(
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

pub(crate) fn discord_audio_kbps(
    choice: &str,
    usable_total_bps: f64,
    has_audio: bool,
) -> Result<u64, String> {
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

pub(crate) fn discord_auto_profile(video_bps: u64) -> (u64, u64) {
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

pub(crate) fn discord_is_low_complexity(info: &MediaInfo) -> bool {
    match (info.width, info.height, info.fps, info.bitrate) {
        (Some(width), Some(height), Some(fps), Some(bitrate))
            if width >= 1280 && height >= 720 && fps > 0.0 && bitrate > 0.0 =>
        {
            bitrate / (width as f64 * height as f64 * fps) <= 0.015
        }
        _ => false,
    }
}

pub(crate) fn discord_video_filter(
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

pub(crate) fn cleanup_passlog(prefix: &Path) {
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

pub(crate) async fn run_discord_compressor(
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
                    job_id: progress_job_id(state),
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
