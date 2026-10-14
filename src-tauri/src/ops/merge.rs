//! Merging multiple videos.

use crate::*;

pub(crate) async fn stream_signature(path: &Path) -> Result<(String, bool, f64), String> {
    let output = process_timeout::output_within(
        hidden_command("ffprobe")
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
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
    let streams = data["streams"]
        .as_array()
        .ok_or("No media streams found.")?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .ok_or("A selected file has no video stream.")?;
    let audio = streams
        .iter()
        .find(|stream| stream["codec_type"] == "audio");
    let signature = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        video["codec_name"],
        video["width"],
        video["height"],
        video["pix_fmt"],
        video["r_frame_rate"],
        audio.map(|v| &v["codec_name"]).unwrap_or(&Value::Null),
        audio.map(|v| &v["sample_rate"]).unwrap_or(&Value::Null),
        audio.map(|v| &v["channels"]).unwrap_or(&Value::Null),
        audio.map(|v| &v["channel_layout"]).unwrap_or(&Value::Null),
        audio.is_some()
    );
    let duration = data["format"]["duration"]
        .as_str()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0);
    Ok((signature, audio.is_some(), duration))
}

pub(crate) fn ffconcat_line(path: &Path) -> String {
    format!(
        "file '{}'{newline}",
        path.to_string_lossy().replace('\'', "'\\''"),
        newline = "\n"
    )
}

pub(crate) fn select_merge_normalization_target(
    candidates: &[(u64, u64, f64)],
) -> Option<(u64, u64, f64)> {
    let &(width, height, _) = candidates
        .iter()
        .min_by_key(|(width, height, _)| (width.saturating_mul(*height), *width, *height))?;
    let fps = candidates
        .iter()
        .map(|(_, _, fps)| *fps)
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .fold(f64::INFINITY, f64::min);
    Some((width, height, if fps.is_finite() { fps } else { 30.0 }))
}

pub(crate) async fn run_merge_videos(
    app: Option<&AppHandle>,
    state: &JobState,
    request: &OperationRequest,
) -> Result<JobResult, String> {
    require_demuxers(&["concat"], "Merge Videos").await?;
    let paths: Vec<String> = serde_json::from_str(param(&request.params, "inputs")?)
        .map_err(|_| "The merge file list is invalid.".to_string())?;
    if paths.len() < 2 {
        return Err("Choose at least two videos to merge.".into());
    }
    let inputs = paths.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    if inputs.iter().any(|path| !path.is_file()) {
        return Err("One of the selected videos could not be found.".into());
    }
    let mut signatures = Vec::new();
    let mut total_duration = 0.0;
    for path in &inputs {
        let signature = stream_signature(path).await.map_err(|error| {
            format!(
                "{}: {error}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        })?;
        total_duration += signature.2;
        signatures.push(signature);
    }
    let compatible = signatures.windows(2).all(|pair| pair[0].0 == pair[1].0);
    let mode = param(&request.params, "mode")?;
    let container = request
        .params
        .get("container")
        .map(String::as_str)
        .unwrap_or("mkv");
    if !["mkv", "mp4"].contains(&container) {
        return Err("Invalid merge output format.".into());
    }
    let first = probe_media(inputs[0].to_string_lossy().to_string()).await?;
    let container_compatible = container == "mkv"
        || (matches!(first.codec.as_str(), "h264" | "hevc" | "mpeg4" | "av1")
            && first
                .audio_codec
                .as_deref()
                .is_none_or(|codec| matches!(codec, "aac" | "mp3" | "ac3" | "eac3" | "alac")));
    if mode == "lossless" && (!compatible || !container_compatible) {
        return Err(
            "These videos cannot be merged losslessly in the selected format. Choose Auto to normalize them safely."
                .into(),
        );
    }
    if !["auto", "lossless", "reencode"].contains(&mode) {
        return Err("Invalid merge mode.".into());
    }
    let normalize =
        mode == "reencode" || (mode == "auto" && (!compatible || !container_compatible));
    let output = unique_output(&inputs[0], "merge_videos", container)?;
    let temp = operation_temp_dir("merge")?;
    let started = Instant::now();
    let result = async {
        let concat_inputs = if normalize {
            let mut candidates = Vec::with_capacity(inputs.len());
            for (index, path) in inputs.iter().enumerate() {
                let probed = if index == 0 {
                    None
                } else {
                    Some(probe_media(path.to_string_lossy().to_string()).await?)
                };
                let media = probed.as_ref().unwrap_or(&first);
                let (width, height) = display_dimensions(path, media).await?;
                candidates.push((
                    width,
                    height,
                    media.fps.unwrap_or(30.0).clamp(1.0, 240.0),
                ));
            }
            let (target_width, target_height, fps) =
                select_merge_normalization_target(&candidates)
                    .ok_or("Merge output properties could not be determined.")?;
            let width = target_width.max(2) / 2 * 2;
            let height = target_height.max(2) / 2 * 2;
            let (encoder_name, encoder_args) = fastest_h264_encoder().await;
            let normalize_status = format!("normalizing videos · {encoder_name}");
            let mut normalized = Vec::new();
            for (index, path) in inputs.iter().enumerate() {
                let target = temp.join(format!("segment_{index:03}.mkv"));
                let has_audio = signatures[index].1;
                let mut args = vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(), "-i".into(), path.to_string_lossy().to_string()];
                if !has_audio { args.extend(["-f".into(), "lavfi".into(), "-i".into(), "anullsrc=channel_layout=stereo:sample_rate=48000".into()]); }
                args.extend(["-map".into(), "0:v:0".into(), "-map".into(), if has_audio { "0:a:0".into() } else { "1:a:0".into() },
                    "-vf".into(), format!("scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,fps={fps:.6},setsar=1,format=yuv420p"),
                ]);
                args.extend(encoder_args.clone());
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into(), "-ar".into(), "48000".into(), "-ac".into(), "2".into(), "-shortest".into(), target.to_string_lossy().to_string()]);
                run_ffmpeg_stage(app, state, args, signatures[index].2, &started, index as f64 * 80.0 / inputs.len() as f64, 80.0 / inputs.len() as f64, &normalize_status).await?;
                normalized.push(target);
            }
            normalized
        } else { inputs.clone() };
        let list = temp.join("concat.ffconcat");
        let content = format!("ffconcat version 1.0\n{}", concat_inputs.iter().map(|path| ffconcat_line(path)).collect::<String>());
        std::fs::write(&list, content).map_err(|error| error.to_string())?;
        run_ffmpeg_stage(app, state, vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(), "-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), list.to_string_lossy().to_string(), "-map".into(), "0:v:0".into(), "-map".into(), "0:a?".into(), "-c".into(), "copy".into(), output.to_string_lossy().to_string()], total_duration, &started, if normalize { 80.0 } else { 0.0 }, if normalize { 19.0 } else { 99.0 }, "merging videos").await?;
        if let Some(app) = app { allow_asset_file(app, &output)?; }
        Ok(JobResult { output: output.to_string_lossy().to_string(), elapsed: started.elapsed().as_secs_f64() })
    }.await;
    let _ = std::fs::remove_dir_all(&temp);
    if result.is_err() {
        let _ = std::fs::remove_file(&output);
    }
    result
}
