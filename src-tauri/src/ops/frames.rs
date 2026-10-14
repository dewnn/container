//! Frame Extractor.

use crate::*;

pub(crate) async fn run_frame_extractor(
    app: &AppHandle,
    state: &JobState,
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<JobResult, String> {
    if info.kind != "video" {
        return Err("Frame extraction requires video.".into());
    }
    let p = &request.params;
    let start = check_range(
        parse_number(p, "start")?,
        0.0,
        info.duration.ok_or("Known duration required.")?,
        "Start",
    )?;
    let end = check_range(
        parse_number(p, "end")?,
        start + 0.001,
        info.duration.unwrap(),
        "End",
    )?;
    let duration = end - start;
    let mode = param(p, "mode")?;
    let count = check_range(parse_number(p, "count")?, 1.0, 1000.0, "Count")?.floor() as usize;
    let filter = match mode {
        "burst" => {
            let interval = check_range(parse_number(p, "interval")?, 0.001, 3600.0, "Interval")?;
            format!(
                "select=isnan(prev_selected_t)+gte(t-prev_selected_t\\,{})",
                (interval - 0.000001).max(0.0)
            )
        }
        "seconds" => {
            let interval = check_range(parse_number(p, "interval")?, 0.1, 3600.0, "Interval")?;
            if duration / interval > 1000.0 {
                return Err("Requested interval exceeds the 1000-frame safety limit.".into());
            }
            format!("fps={}", 1.0 / interval)
        }
        "frames" => {
            let n = check_range(
                parse_number(p, "every_frames")?,
                1.0,
                100000.0,
                "Frame interval",
            )?
            .floor();
            if duration * info.fps.unwrap_or(60.0) / n > 1000.0 {
                return Err("Requested interval exceeds the 1000-frame safety limit.".into());
            }
            format!("select=not(mod(n\\,{n}))")
        }
        "scene" => format!(
            "select=gt(scene\\,{})",
            check_range(
                parse_number(p, "scene_threshold")?,
                0.01,
                1.0,
                "Scene threshold"
            )?
        ),
        "even" => format!("fps={}", count as f64 / duration),
        "sheet" => {
            if count > 100 {
                return Err("Contact sheet is limited to 100 frames.".into());
            }
            let columns =
                check_range(parse_number(p, "columns")?, 1.0, 10.0, "Columns")?.floor() as usize;
            format!(
                "fps={},scale=320:-2,tile={}x{}",
                count as f64 / duration,
                columns,
                count.div_ceil(columns)
            )
        }
        _ => return Err("Invalid extraction mode.".into()),
    };
    let output = unique_output(
        Path::new(&request.input),
        "frame_extractor",
        if mode == "sheet" { "png" } else { "frames" },
    )?;
    if mode != "sheet" {
        std::fs::create_dir(&output).map_err(|e| e.to_string())?;
    }
    let destination = if mode == "sheet" {
        output.clone()
    } else {
        output.join("frame-%06d.png")
    };
    let args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-ss".into(),
        start.to_string(),
        "-i".into(),
        request.input.clone(),
        "-t".into(),
        duration.to_string(),
        "-vf".into(),
        filter,
        "-fps_mode".into(),
        "vfr".into(),
        "-frames:v".into(),
        if mode == "sheet" {
            "1".into()
        } else if mode == "even" || mode == "burst" {
            count.to_string()
        } else {
            "1000".into()
        },
        destination.to_string_lossy().into_owned(),
    ];
    let started = Instant::now();
    if let Err(error) = run_ffmpeg_stage(
        Some(app),
        state,
        args,
        duration,
        &started,
        0.0,
        99.0,
        "extracting frames",
    )
    .await
    {
        if mode == "sheet" {
            let _ = std::fs::remove_file(&output);
        } else {
            let _ = std::fs::remove_dir_all(&output);
        }
        return Err(error);
    }
    if mode == "sheet" {
        if !output.is_file() || std::fs::metadata(&output).map_err(|e| e.to_string())?.len() == 0 {
            return Err("Contact sheet was not produced.".into());
        }
        allow_asset_file(app, &output)?;
    } else {
        let mut produced = 0;
        for entry in std::fs::read_dir(&output).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_file() {
                allow_asset_file(app, &path)?;
                produced += 1
            }
        }
        if produced == 0 {
            let _ = std::fs::remove_dir(&output);
            return Err("No matching frames found in this range.".into());
        }
    }
    Ok(JobResult {
        output: output.to_string_lossy().into_owned(),
        elapsed: started.elapsed().as_secs_f64(),
    })
}
