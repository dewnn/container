//! `run_operation` dispatch and job cancellation.

use crate::*;

#[tauri::command]
pub(crate) async fn run_operation(
    app: AppHandle,
    state: State<'_, JobState>,
    mut request: OperationRequest,
    job_id: Option<String>,
) -> Result<JobResult, String> {
    let _job = JobLease::acquire(&state, job_id)?;
    reset_legacy_job_cancellation(&state);
    let info = probe_media(request.input.clone()).await?;
    if request.operation == "frame_extractor" {
        return run_frame_extractor(&app, &state, &request, &info).await;
    }
    if request.operation == "processing_stack" {
        return run_processing_stack(&app, &state, &request, info).await;
    }
    request.params.remove("text_raster_path");
    let _text_raster_temp = if request.operation == "text"
        || (request.operation == "clipper" && enabled(&request.params, "watermark_enabled"))
    {
        if let Some(data_url) = request.params.remove("text_raster_png") {
            let encoded = data_url
                .strip_prefix("data:image/png;base64,")
                .ok_or("Invalid text raster image.")?;
            if encoded.len() > 24_000_000 {
                return Err("Text raster image is too large.".into());
            }
            let png = BASE64
                .decode(encoded)
                .map_err(|_| "Invalid text raster image encoding.")?;
            if png.len() < 24 || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err("Invalid text raster PNG.".into());
            }
            let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
            if width == 0 || height == 0 || width > 8192 || height > 8192 {
                return Err("Invalid text raster dimensions.".into());
            }
            if request.operation == "clipper"
                && (f64::from(width) != parse_number(&request.params, "output_width")?
                    || f64::from(height) != parse_number(&request.params, "output_height")?)
            {
                return Err("Watermark raster must match the output dimensions.".into());
            }
            let directory = OperationTempGuard(operation_temp_dir("text-raster")?);
            let path = directory.0.join("text.png");
            std::fs::write(&path, png).map_err(|error| error.to_string())?;
            request.params.insert(
                "text_raster_path".into(),
                path.to_string_lossy().into_owned(),
            );
            Some(directory)
        } else {
            None
        }
    } else {
        request.params.remove("text_raster_png");
        None
    };
    if request.operation == "merge_videos" {
        return run_merge_videos(Some(&app), &state, &request).await;
    }
    if request.operation == "stabilizer" {
        return run_stabilizer(Some(&app), &state, &request, &info).await;
    }
    if request.operation == "image_potatoify" {
        return run_image_potatoify(Some(&app), &state, &request, &info).await;
    }
    if request.operation == "image_compressor" {
        return run_image_compressor(Some(&app), &state, &request, &info).await;
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
    let mut cpu_retried = false;
    loop {
        state.inspector.start(&args);
        let mut child = hidden_command("ffmpeg")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("FFmpeg could not start: {e}"))?;
        let active_process = ActiveProcess::register(&state, child.id())?;
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
        let duration = operation_progress_duration(&request, info.duration.unwrap_or(0.0));
        loop {
            let line = match lines.next_line().await {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(error) => {
                    let _ = child.kill().await;
                    let _ = std::fs::remove_file(&output);
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
                let percent = if duration > 0.0 {
                    (out_time / duration * 100.0).clamp(0.0, 99.9)
                } else {
                    0.0
                };
                let _ = app.emit(
                    "container-progress",
                    ProgressEvent {
                        job_id: progress_job_id(&state),
                        percent,
                        time: started.elapsed().as_secs_f64(),
                        speed: speed.clone(),
                        frame: frame.clone(),
                        status: "rendering".into(),
                    },
                );
            }
        }
        let status = match child.wait().await {
            Ok(status) => status,
            Err(error) => {
                let _ = std::fs::remove_file(&output);
                return Err(error.to_string());
            }
        };
        drop(active_process);
        let errors = stderr_task.await.unwrap_or_default();
        state.inspector.errors(&errors);
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
            if !cpu_retried {
                if let Some(retry) = auto_encoder::cpu_retry_args(&args, &detail) {
                    args = retry;
                    cpu_retried = true;
                    continue;
                }
            }
            return Err(if detail.is_empty() {
                "FFmpeg operation failed.".into()
            } else {
                detail
            });
        }
        allow_asset_file(&app, &output)?;
        return Ok(JobResult {
            output: output.to_string_lossy().to_string(),
            elapsed: started.elapsed().as_secs_f64(),
        });
    }
}

#[tauri::command]
pub(crate) async fn cancel_job(
    state: State<'_, JobState>,
    job_id: Option<String>,
) -> Result<(), String> {
    let Some(target) = request_job_cancellation(&state, job_id.as_deref())? else {
        return Ok(());
    };
    if let Some(pid) = target.pid {
        tokio::task::spawn_blocking(move || stop_process_tree(pid))
            .await
            .ok();
    }
    let active = state
        .active_job
        .lock()
        .map_err(|_| "Job state lock failed")?;
    if *active == target.job_id {
        cleanup_current_download(&state);
    }
    Ok(())
}
