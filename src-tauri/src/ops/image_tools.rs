//! Image Compressor and Image Potatoify.

use crate::*;

pub(crate) fn compressor_args(
    input: &Path,
    output: &Path,
    extension: &str,
    quality: u8,
    background: &str,
    flatten_alpha: bool,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        input.to_string_lossy().to_string(),
    ];
    if extension == "jpg" && flatten_alpha {
        args.extend(["-vf".into(), alpha_flatten_filter(background)?]);
    }
    args.extend([
        "-frames:v".into(),
        "1".into(),
        "-map_metadata".into(),
        "0".into(),
    ]);
    append_image_encoder(&mut args, extension, quality)?;
    args.push(output.to_string_lossy().to_string());
    Ok(args)
}

pub(crate) async fn optimized_png(
    input: PathBuf,
    palette: bool,
    target: Option<u64>,
    cancelled: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    static PNG_WORKER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let permit = PNG_WORKER.acquire().await.map_err(|e| e.to_string())?;
    let result = tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(input).map_err(|e| e.to_string())?;
        png_compression::compress(&bytes, palette, target, &cancelled)
    })
    .await
    .map_err(|e| format!("PNG worker failed: {e}"))?;
    drop(permit);
    result
}

#[tauri::command]
pub(crate) async fn estimate_image_compression(request: OperationRequest) -> Result<u64, String> {
    let input = PathBuf::from(&request.input);
    let info = probe_media(request.input.clone()).await?;
    if info.kind != "image" {
        return Err("Image Compressor requires an image file.".into());
    }
    let extension = match param(&request.params, "format")? {
        "source" => image_extension(&operation_output_source(&request)?),
        "jpeg" | "jpg" => "jpg".into(),
        "png" => "png".into(),
        "webp" => "webp".into(),
        _ => return Err("Unsupported compressor output format.".into()),
    };
    let quality = check_range(
        parse_number(&request.params, "quality")?,
        1.0,
        100.0,
        "Quality",
    )? as u8;
    let background = request
        .params
        .get("jpeg_background")
        .map(String::as_str)
        .unwrap_or("#ffffff");
    if extension == "jpg" {
        parse_hex_color(background, "JPEG background")?;
    }
    let temp = operation_temp_dir("image-estimate")?;
    let output = temp.join(format!("estimate.{extension}"));
    let result = async {
        let palette = request
            .params
            .get("png_mode")
            .is_some_and(|mode| mode == "palette");
        if extension == "png" && image_extension(&input) == "png" {
            return optimized_png(
                input.clone(),
                palette,
                None,
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .map(|bytes| bytes.len() as u64);
        }
        let child = hidden_command("ffmpeg")
            .args(compressor_args(
                &input,
                &output,
                &extension,
                quality,
                background,
                image_has_alpha(&info),
            )?)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("Image size estimate could not start: {error}"))?;
        let completed = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
            .await
            .map_err(|_| "Image size estimate timed out.".to_string())?
            .map_err(|error| format!("Image size estimate failed: {error}"))?;
        if !completed.status.success() {
            let detail = String::from_utf8_lossy(&completed.stderr)
                .trim()
                .to_string();
            return Err(if detail.is_empty() {
                "Image size estimate failed.".into()
            } else {
                detail
            });
        }
        if extension == "png" {
            return optimized_png(
                output.clone(),
                palette,
                None,
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .map(|bytes| bytes.len() as u64);
        }
        std::fs::metadata(&output)
            .map(|metadata| metadata.len())
            .map_err(|error| format!("Estimated image size could not be read: {error}"))
    }
    .await;
    let _ = std::fs::remove_dir_all(&temp);
    result
}

pub(crate) async fn run_image_compressor(
    app: Option<&AppHandle>,
    state: &JobState,
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<JobResult, String> {
    if info.kind != "image" {
        return Err("Image Compressor requires an image file.".into());
    }
    let input = PathBuf::from(&request.input);
    let output_source = operation_output_source(request)?;
    let requested_format = param(&request.params, "format")?;
    let extension = match requested_format {
        "source" => image_extension(&output_source),
        "jpeg" | "jpg" => "jpg".into(),
        "png" => "png".into(),
        "webp" => "webp".into(),
        _ => return Err("Unsupported compressor output format.".into()),
    };
    let mode = param(&request.params, "mode")?;
    if !["quality", "target"].contains(&mode) {
        return Err("Invalid image compression mode.".into());
    }
    let quality = check_range(
        parse_number(&request.params, "quality")?,
        1.0,
        100.0,
        "Quality",
    )? as u8;
    let background = request
        .params
        .get("jpeg_background")
        .map(String::as_str)
        .unwrap_or("#ffffff");
    if extension == "jpg" {
        parse_hex_color(background, "JPEG background")?;
    }
    let output = unique_output(&output_source, "image_compressor", &extension)?;
    let temp = operation_temp_dir("image-compressor")?;
    let started = Instant::now();
    let result = async {
        let chosen = if extension == "png" {
            let candidate = temp.join("optimized.png");
            let source = if image_extension(&input) == "png" { input.clone() } else {
                let converted = temp.join("converted.png");
                run_ffmpeg_stage(app, state, compressor_args(&input, &converted, "png", 100, background, false)?, 1.0, &started, 0.0, 30.0, "preparing PNG").await?;
                converted
            };
            let target = if mode == "target" { Some((check_range(parse_number(&request.params, "target_kb")?, 1.0, 1_048_576.0, "Target size")? * 1024.0) as u64) } else { None };
            let palette = match request.params.get("png_mode").map(String::as_str).unwrap_or("lossless") {
                "lossless" => false,
                "palette" => true,
                _ => return Err("Invalid PNG compression mode.".into()),
            };
            let bytes = optimized_png(source, palette, target, state.cancelled.clone()).await?;
            if state.cancelled.load(Ordering::Relaxed) { return Err("Job cancelled.".into()); }
            std::fs::write(&candidate, bytes).map_err(|e| e.to_string())?;
            candidate
        } else if mode == "quality" {
            let candidate = temp.join(format!("quality.{extension}"));
            run_ffmpeg_stage(
                app,
                state,
                compressor_args(&input, &candidate, &extension, quality, background, image_has_alpha(info))?,
                1.0,
                &started,
                0.0,
                99.0,
                "compressing image",
            )
            .await?;
            candidate
        } else {
            if !["jpg", "webp"].contains(&extension.as_str()) {
                return Err("Target Size needs JPEG or WebP because lossless PNG size cannot be controlled reliably. Choose JPEG or WebP.".into());
            }
            let target = check_range(
                parse_number(&request.params, "target_kb")?,
                1.0,
                1_048_576.0,
                "Target size",
            )? * 1024.0;
            let mut low = 1u8;
            let mut high = 100u8;
            let mut best_under: Option<(u8, PathBuf, u64)> = None;
            let mut smallest: Option<(PathBuf, u64)> = None;
            for pass in 0..7 {
                let current = low + (high - low) / 2;
                let candidate = temp.join(format!("candidate_{pass}_{current}.{extension}"));
                run_ffmpeg_stage(
                    app,
                    state,
                    compressor_args(&input, &candidate, &extension, current, background, image_has_alpha(info))?,
                    1.0,
                    &started,
                    pass as f64 * 99.0 / 7.0,
                    99.0 / 7.0,
                    "finding target size",
                )
                .await?;
                let size = std::fs::metadata(&candidate).map_err(|error| error.to_string())?.len();
                if smallest.as_ref().is_none_or(|(_, known)| size < *known) {
                    smallest = Some((candidate.clone(), size));
                }
                if size as f64 <= target {
                    if best_under.as_ref().is_none_or(|(known, _, _)| current > *known) {
                        best_under = Some((current, candidate.clone(), size));
                    }
                    if current == 100 { break; }
                    low = current.saturating_add(1);
                } else {
                    if current <= 1 { break; }
                    high = current.saturating_sub(1);
                }
                if low > high { break; }
            }
            if let Some((_, path, _)) = best_under {
                path
            } else if let Some((_, size)) = smallest {
                return Err(format!(
                    "The requested size cannot be reached without changing dimensions. The smallest tested result was {:.1} KB.",
                    size as f64 / 1024.0
                ));
            } else {
                return Err("Image compression did not produce an output.".into());
            }
        };
        std::fs::copy(&chosen, &output).map_err(|error| format!("Compressed image could not be saved: {error}"))?;
        if let Some(app) = app { allow_asset_file(app, &output)?; }
        Ok(JobResult { output: output.to_string_lossy().to_string(), elapsed: started.elapsed().as_secs_f64() })
    }.await;
    let _ = std::fs::remove_dir_all(&temp);
    if result.is_err() {
        let _ = std::fs::remove_file(&output);
    }
    result
}

pub(crate) async fn run_image_potatoify(
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
    let output_source = operation_output_source(request)?;
    let output = unique_output(&output_source, "image_potatoify", "jpg")?;
    let temp = operation_temp_dir("image")?;
    let started = Instant::now();
    let result = async {
        let mut current = input.clone();
        for index in 1..=times {
            if state.cancelled.load(Ordering::Relaxed) {
                return Err("Job cancelled.".into());
            }
            let next = temp.join(format!("pass_{index:03}.jpg"));
            run_ffmpeg_stage(
                app,
                state,
                vec![
                    "-hide_banner".into(),
                    "-loglevel".into(),
                    "error".into(),
                    "-y".into(),
                    "-i".into(),
                    current.to_string_lossy().into(),
                    "-frames:v".into(),
                    "1".into(),
                    "-vf".into(),
                    format!("scale={width}:{height}:flags=neighbor"),
                    "-q:v".into(),
                    qscale.to_string(),
                    "-pix_fmt".into(),
                    "yuvj420p".into(),
                    next.to_string_lossy().into(),
                ],
                0.0,
                &started,
                (index - 1) as f64 / times as f64 * 99.0,
                99.0 / times as f64,
                "compressing image",
            )
            .await
            .map_err(|error| format!("Image compression failed on pass {index}: {error}"))?;
            current = next;
            if let Some(app) = app {
                let _ = app.emit(
                    "container-progress",
                    ProgressEvent {
                        job_id: progress_job_id(state),
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
        if let Some(app) = app {
            allow_asset_file(app, &output)?;
        }
        Ok(JobResult {
            output: output.to_string_lossy().to_string(),
            elapsed: started.elapsed().as_secs_f64(),
        })
    }
    .await;
    let _ = std::fs::remove_dir_all(&temp);
    if result.is_err() {
        let _ = std::fs::remove_file(&output);
    }
    result
}
