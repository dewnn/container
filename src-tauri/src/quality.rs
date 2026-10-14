//! Smart Quality Analysis (VMAF-based CRF recommendation).

use crate::*;

#[derive(Debug, Deserialize)]
pub(crate) struct QualityAnalysisRequest {
    pub(crate) input: String,
    pub(crate) goal: String,
    pub(crate) sample_duration: f64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct QualityCandidate {
    pub(crate) crf: u64,
    pub(crate) vmaf: f64,
    pub(crate) estimated_size_mb: f64,
    pub(crate) rating: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct QualityAnalysis {
    pub(crate) recommended_crf: u64,
    pub(crate) target_vmaf: f64,
    pub(crate) candidates: Vec<QualityCandidate>,
    pub(crate) sample_count: usize,
    pub(crate) sampled_seconds: f64,
    pub(crate) elapsed: f64,
}

pub(crate) fn quality_sample_positions(duration: f64, sample_duration: f64) -> Vec<f64> {
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

pub(crate) fn quality_dimensions(info: &MediaInfo) -> Result<(u64, u64), String> {
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

pub(crate) fn quality_target(goal: &str) -> Result<f64, String> {
    match goal {
        "high" => Ok(95.0),
        "balanced" => Ok(92.0),
        "small" => Ok(88.0),
        _ => Err("Invalid quality goal.".into()),
    }
}

pub(crate) fn quality_rating(vmaf: f64) -> &'static str {
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

pub(crate) fn recommend_quality_crf(candidates: &[QualityCandidate], target: f64) -> Option<u64> {
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

pub(crate) async fn run_quality_capture(
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
    let active_process = ActiveProcess::register(state, child.id())?;
    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
    drop(active_process);
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

pub(crate) async fn analyze_quality_inner(
    app: Option<&AppHandle>,
    state: &JobState,
    request: QualityAnalysisRequest,
) -> Result<QualityAnalysis, String> {
    reset_legacy_job_cancellation(state);
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
                        job_id: progress_job_id(state),
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
                        job_id: progress_job_id(state),
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
                        job_id: progress_job_id(state),
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
pub(crate) async fn analyze_quality(
    app: AppHandle,
    state: State<'_, JobState>,
    request: QualityAnalysisRequest,
    job_id: Option<String>,
) -> Result<QualityAnalysis, String> {
    let _job = JobLease::acquire(&state, job_id)?;
    analyze_quality_inner(Some(&app), &state, request).await
}
