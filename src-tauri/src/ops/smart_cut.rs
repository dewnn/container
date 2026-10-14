//! Smart Cut: keyframe-aware cutting with re-encoded edges.

use crate::*;

#[cfg(test)]
pub(crate) fn smart_cut_encoded_args(
    request: &OperationRequest,
    info: &MediaInfo,
    start: f64,
    end: f64,
    output: &Path,
    encoder: &str,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-ss".into(),
        format!("{start:.6}"),
        "-i".into(),
        request.input.clone(),
        "-ss".into(),
        "0".into(),
        "-t".into(),
        format!("{:.6}", end - start),
        "-map".into(),
        "0:v:0".into(),
        "-c:v".into(),
        encoder.into(),
        "-preset".into(),
        "veryfast".into(),
    ];
    if encoder == "libx265" {
        args.extend(["-x265-params".into(), "qp=1".into()]);
    } else {
        args.extend(["-profile:v".into(), "high".into(), "-qp".into(), "1".into()]);
    }
    if let Some(pixel_format) = &info.pixel_format {
        args.extend(["-pix_fmt".into(), pixel_format.clone()]);
    }
    let mp4_output = output
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"));
    if info.codec == "hevc" && mp4_output {
        args.extend(["-tag:v".into(), "hvc1".into()]);
    }
    append_audio_routing(&mut args, info, &request.params, false, "main")?;
    if mp4_output {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    args.push(output.to_string_lossy().to_string());
    Ok(args)
}

#[cfg(test)]
pub(crate) async fn run_smart_cut(
    app: Option<&AppHandle>,
    state: &JobState,
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<JobResult, String> {
    if info.kind != "video" {
        return Err("Smart Cut requires a video file.".into());
    }
    let encoder = match info.codec.as_str() {
        "h264" => "libx264",
        "hevc" => "libx265",
        codec => {
            return Err(format!(
                "Smart Cut currently supports H.264 and HEVC video; this file uses {codec}."
            ));
        }
    };
    if request.params.get("audio_mode").map(String::as_str) == Some("merge") {
        return Err("Smart Cut cannot merge audio tracks without re-encoding them. Choose Main, All, Selected, or None.".into());
    }
    let duration = info.duration.ok_or("Video duration is unavailable.")?;
    let start = check_range(
        parse_number(&request.params, "start")?,
        0.0,
        duration,
        "Start",
    )?;
    let end = check_range(parse_number(&request.params, "end")?, 0.0, duration, "End")?;
    if end <= start {
        return Err("End must be greater than start.".into());
    }
    let input = PathBuf::from(&request.input);
    let source_extension = input
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mp4_safe = matches!(source_extension.as_str(), "mp4" | "mov" | "m4v")
        && mp4_audio_stream_copy_supported(info);
    let output_extension = if mp4_safe { "mp4" } else { "mkv" };
    let output = unique_output(&input, "cut", output_extension)?;
    let selected_duration = end - start;
    let started = Instant::now();
    let result = async {
        // A boundary-only encode cannot be safely byte-concatenated with an
        // arbitrary H.264/HEVC source GOP: SPS/PPS and reference-frame settings
        // can differ even when profile and pixel format match. Such files may
        // software-decode partially yet turn green in hardware players. Encode
        // the selected interval as one consistent near-lossless stream instead.
        let args = smart_cut_encoded_args(request, info, start, end, &output, encoder)?;
        run_ffmpeg_stage(
            app,
            state,
            args,
            selected_duration,
            &started,
            0.0,
            99.0,
            "smart cut · precise source-quality encode",
        )
        .await?;
        if let Some(app) = app {
            allow_asset_file(app, &output)?;
        }
        Ok(JobResult {
            output: output.to_string_lossy().to_string(),
            elapsed: started.elapsed().as_secs_f64(),
        })
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_file(&output);
    }
    result
}
