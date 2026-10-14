//! Processing Stack: chaining several steps into one render.

use crate::*;

pub(crate) fn prepare_stack_raster(
    step: &mut processing_stack::Step,
    directory: &Path,
    index: usize,
) -> Result<(), String> {
    step.params.remove("text_raster_path");
    let Some(data) = step.params.remove("text_raster_png") else {
        return Ok(());
    };
    if step.operation != "text" && step.operation != "clipper" {
        return Err("Raster is not allowed for this step.".into());
    }
    let encoded = data
        .strip_prefix("data:image/png;base64,")
        .ok_or("Invalid text raster image.")?;
    if encoded.len() > 24_000_000 {
        return Err("Text raster is too large.".into());
    }
    let png = BASE64
        .decode(encoded)
        .map_err(|_| "Invalid text raster encoding.")?;
    if png.len() < 24 || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid text raster PNG.".into());
    }
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("Invalid text raster dimensions.".into());
    }
    if step.operation == "clipper"
        && (f64::from(width) != parse_number(&step.params, "output_width")?
            || f64::from(height) != parse_number(&step.params, "output_height")?)
    {
        return Err("Watermark raster must match the output dimensions.".into());
    }
    let path = directory.join(format!("raster-{index}.png"));
    std::fs::write(&path, png).map_err(|e| e.to_string())?;
    step.params.insert(
        "text_raster_path".into(),
        path.to_string_lossy().into_owned(),
    );
    Ok(())
}

pub(crate) fn stack_smartcut_args(
    params: &HashMap<String, String>,
    info: &MediaInfo,
    input: &str,
) -> Result<(Vec<String>, f64), String> {
    if params.get("source_path").map(String::as_str) != Some(input) {
        return Err("SmartCut cuts belong to a different source. Add the cuts again.".into());
    }
    let raw = params.get("cuts").ok_or("SmartCut cuts are missing.")?;
    let cuts: Vec<KeepInterval> =
        serde_json::from_str(raw).map_err(|_| "Invalid SmartCut cuts.")?;
    if cuts.len() > 1000 {
        return Err("SmartCut Stack is limited to 1000 regions.".into());
    }
    let cuts = enabled_cuts(
        &cuts,
        info.duration
            .ok_or("Known duration required for SmartCut.")?,
    )?;
    let resolution = params
        .get("resolution")
        .map(String::as_str)
        .unwrap_or("source");
    validate_smartcut_resolution(resolution)?;
    let duration = cuts.iter().map(|cut| cut.end - cut.start).sum();
    let mut args = vec![
        "-hide_banner".into(),
        "-y".into(),
        "-i".into(),
        input.into(),
        "-filter_complex".into(),
        autocut_filter_graph(&cuts, info.audio_codec.is_some(), resolution),
        "-map".into(),
        "[vout]".into(),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        "14".into(),
        "-preset".into(),
        "veryfast".into(),
    ];
    if let Some(fps) = info.fps.filter(|fps| fps.is_finite() && *fps > 0.0) {
        args.extend([
            "-r".into(),
            format!("{fps:.10}"),
            "-fps_mode".into(),
            "cfr".into(),
        ]);
    }
    if info.audio_codec.is_some() {
        args.extend([
            "-map".into(),
            "[aout]".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
        ]);
    } else {
        args.push("-an".into());
    }
    args.push("stack-output.mp4".into());
    Ok((args, duration))
}

pub(crate) async fn run_processing_stack(
    app: &AppHandle,
    state: &JobState,
    request: &OperationRequest,
    mut info: MediaInfo,
) -> Result<JobResult, String> {
    if info.kind != "video" {
        return Err("The Processing Stack currently requires video.".into());
    }
    let raw = request
        .params
        .get("steps")
        .ok_or("Processing Stack is empty.")?;
    // Text/watermark layers can contain a full-resolution PNG data URL.
    if raw.len() > 64_000_000 {
        return Err("Processing Stack is too large.".into());
    }
    let steps: Vec<processing_stack::Step> =
        serde_json::from_str(raw).map_err(|_| "Invalid Processing Stack.")?;
    if steps.len() > 24 {
        return Err("Processing Stack is limited to 24 steps.".into());
    }
    let steps: Vec<_> = steps.into_iter().filter(|step| step.enabled).collect();
    if steps.is_empty() {
        return Err("Enable at least one Stack step.".into());
    }
    if steps
        .iter()
        .enumerate()
        .any(|(index, step)| step.operation == "smartcut" && index != 0)
    {
        return Err(
            "SmartCut must be the first Stack step because its cuts use source timestamps.".into(),
        );
    }
    let quality = request
        .params
        .get("quality")
        .map(String::as_str)
        .unwrap_or("high");
    if !["lossless", "high", "medium", "small"].contains(&quality) {
        return Err("Invalid Stack quality.".into());
    }
    let temp = OperationTempGuard(operation_temp_dir("processing-stack")?);
    if steps.len() == 2
        && steps[0].operation == "smartcut"
        && steps[1].operation == "clipper"
        && steps[0]
            .params
            .get("resolution")
            .is_none_or(|value| value == "source")
    {
        let (cut_args, duration) = stack_smartcut_args(&steps[0].params, &info, &request.input)?;
        let graph = cut_args
            .windows(2)
            .find(|pair| pair[0] == "-filter_complex")
            .map(|pair| pair[1].as_str())
            .ok_or("SmartCut graph is missing.")?;
        let mut clipper = steps[1].clone();
        prepare_stack_raster(&mut clipper, &temp.0, 1)?;
        let stage = OperationRequest {
            input: request.input.clone(),
            operation: "clipper".into(),
            params: clipper.params,
        };
        let (args, _) = build_command(&stage, &info).await?;
        if let Some(args) =
            processing_stack::fuse_smartcut_clipper(args, graph, info.audio_codec.is_some())
        {
            let destination = unique_output(Path::new(&request.input), "processing_stack", "mp4")?;
            let mut args = processing_stack::output_args(args, &destination, false, Some(quality))?;
            if quality == "high" {
                let (_, profile) = fastest_h264_encoder().await;
                args = processing_stack::verified_high_args(args, &profile);
            }
            let started = Instant::now();
            if let Err(error) = run_ffmpeg_stage(
                Some(app),
                state,
                args,
                duration,
                &started,
                0.0,
                99.0,
                "processing stack",
            )
            .await
            {
                let _ = std::fs::remove_file(&destination);
                return Err(error);
            }
            if let Err(error) = probe_media(destination.to_string_lossy().into_owned()).await {
                let _ = std::fs::remove_file(&destination);
                return Err(error);
            }
            allow_asset_file(app, &destination)?;
            return Ok(JobResult {
                output: destination.to_string_lossy().into_owned(),
                elapsed: started.elapsed().as_secs_f64(),
            });
        }
    }
    let mut input = request.input.clone();
    let started = Instant::now();
    let count = steps.len();
    let source_duration = info.duration;
    let mut source_ranges = source_duration
        .map(|duration| vec![(0.0, duration)])
        .unwrap_or_default();
    for (index, mut step) in steps.into_iter().enumerate() {
        if state.cancelled.load(Ordering::Relaxed) {
            return Err("Job cancelled.".into());
        }
        if ![
            "smartcut",
            "cut",
            "transform",
            "clipper",
            "color",
            "text",
            "image_overlay",
            "audio_lab",
        ]
        .contains(&step.operation.as_str())
        {
            return Err(format!(
                "{} cannot be used in the Processing Stack.",
                step.operation
            ));
        }
        step.params.remove("__source_path");
        prepare_stack_raster(&mut step, &temp.0, index)?;
        let audio_only_step = step.operation == "audio_lab";
        let stage = OperationRequest {
            input: input.clone(),
            operation: step.operation,
            params: step.params,
        };
        let (mut args, stage_duration) = if stage.operation == "smartcut" {
            let cuts: Vec<KeepInterval> = serde_json::from_str(
                stage
                    .params
                    .get("cuts")
                    .ok_or("SmartCut cuts are missing.")?,
            )
            .map_err(|_| "Invalid SmartCut cuts.")?;
            let cuts = enabled_cuts(
                &cuts,
                source_duration.ok_or("Known source duration required.")?,
            )?;
            source_ranges = cuts.iter().map(|cut| (cut.start, cut.end)).collect();
            stack_smartcut_args(&stage.params, &info, &input)?
        } else if stage.operation == "cut" {
            let duration =
                source_duration.ok_or("Known source duration required for Stack cuts.")?;
            let start = check_range(
                parse_number(&stage.params, "start")?,
                0.0,
                duration,
                "Source start",
            )?;
            let end = check_cut_end(parse_number(&stage.params, "end")?, duration)?;
            let (current, kept) = processing_stack::source_window(&source_ranges, start, end)?;
            // Frame-accurate trims keep the source mapping valid; stream-copy
            // keyframe cuts can silently include extra time between steps.
            let cuts: Vec<_> = current
                .iter()
                .map(|&(start, end)| KeepInterval {
                    start,
                    end,
                    enabled: true,
                })
                .collect();
            let params = HashMap::from([
                ("source_path".into(), input.clone()),
                (
                    "cuts".into(),
                    serde_json::to_string(&cuts).map_err(|e| e.to_string())?,
                ),
                ("resolution".into(), "source".into()),
            ]);
            let result = stack_smartcut_args(&params, &info, &input)?;
            source_ranges = kept;
            result
        } else {
            let (args, _) = build_command(&stage, &info).await?;
            (args, info.duration.unwrap_or(0.0))
        };
        let last = index + 1 == count;
        let destination = if last {
            unique_output(
                Path::new(&request.input),
                "processing_stack",
                if audio_only_step { "mkv" } else { "mp4" },
            )?
        } else {
            temp.0.join(format!("stage-{index}.mkv"))
        };
        args = processing_stack::output_args(
            args,
            &destination,
            !last,
            if last && (count > 1 || quality != "high") && !(count == 1 && audio_only_step) {
                Some(quality)
            } else {
                None
            },
        )?;
        if last && count > 1 && quality == "high" && !audio_only_step {
            let (_, profile) = fastest_h264_encoder().await;
            args = processing_stack::verified_high_args(args, &profile);
        }
        if let Err(error) = run_ffmpeg_stage(
            Some(app),
            state,
            args,
            stage_duration,
            &started,
            index as f64 * 99.0 / count as f64,
            99.0 / count as f64,
            "processing stack",
        )
        .await
        {
            let _ = std::fs::remove_file(&destination);
            return Err(error);
        }
        if !destination.is_file()
            || std::fs::metadata(&destination)
                .map_err(|e| e.to_string())?
                .len()
                == 0
        {
            return Err("Stack stage did not produce a video.".into());
        }
        info = match probe_media(destination.to_string_lossy().into_owned()).await {
            Ok(info) => info,
            Err(error) => {
                let _ = std::fs::remove_file(&destination);
                return Err(error);
            }
        };
        // Keep at most the previous and current intermediate, not the whole
        // lossless chain. Only exact files owned by this temp guard are removed.
        if index > 0 {
            let previous = temp.0.join(format!("stage-{}.mkv", index - 1));
            if Path::new(&input) == previous {
                let _ = std::fs::remove_file(previous);
            }
        }
        input = destination.to_string_lossy().into_owned();
    }
    let output = PathBuf::from(input);
    allow_asset_file(app, &output)?;
    Ok(JobResult {
        output: output.to_string_lossy().into_owned(),
        elapsed: started.elapsed().as_secs_f64(),
    })
}
