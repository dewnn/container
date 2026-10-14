//! SmartCut: Silero VAD analysis, keep regions, FCPXML and MP4 export.

use crate::*;

// Embed the exact model used by the dependency, so released builds never
// depend on the build machine's Cargo registry directory.
pub(crate) fn load_silero_vad() -> Result<OnnxModel, String> {
    let directory = app_cache_directory()?.join("models");
    load_embedded_vad(&directory)
}

pub(crate) fn load_embedded_vad(directory: &Path) -> Result<OnnxModel, String> {
    let bytes = include_bytes!("../resources/silero_vad.onnx");
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let path = directory.join(format!("silero-{:x}.onnx", Sha256::digest(bytes)));
    if std::fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    }
    OnnxModel::from_path(path, true).map_err(|e| e.to_string())
}

pub(crate) struct VadCache {
    pub(crate) path: PathBuf,
    pub(crate) modified: Option<std::time::SystemTime>,
    pub(crate) len: u64,
    pub(crate) scores: Arc<Vec<f32>>,
    pub(crate) samples: Arc<Vec<i16>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct KeepInterval {
    pub(crate) start: f64,
    pub(crate) end: f64,
    pub(crate) enabled: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AutoCutRequest {
    pub(crate) input: String,
    pub(crate) analysis_input: Option<String>,
    pub(crate) threshold: f64,
    pub(crate) min_silence: f64,
    pub(crate) min_speech: f64,
    #[serde(default)]
    pub(crate) padding: Option<f64>,
    #[serde(default)]
    pub(crate) minimum_pause: Option<f64>,
    #[serde(default)]
    pub(crate) keep_before_speech: Option<f64>,
    #[serde(default)]
    pub(crate) keep_after_speech: Option<f64>,
    #[serde(default)]
    pub(crate) boundary_refinement: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AutoCutAnalysis {
    pub(crate) cuts: Vec<KeepInterval>,
    pub(crate) waveform: Vec<f32>,
    pub(crate) duration: f64,
    pub(crate) boundary_refinement: bool,
    pub(crate) overlaps_before_normalization: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct AutoCutRecommendation {
    pub(crate) threshold: f64,
    pub(crate) min_silence: f64,
    pub(crate) min_speech: f64,
    pub(crate) minimum_pause: f64,
    pub(crate) keep_before_speech: f64,
    pub(crate) keep_after_speech: f64,
    pub(crate) noise_floor_db: f64,
    pub(crate) speech_level_db: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct AutoCutPreset {
    pub(crate) id: &'static str,
    pub(crate) minimum_pause: f64,
    pub(crate) keep_before_speech: f64,
    pub(crate) keep_after_speech: f64,
}

pub(crate) const AUTOCUT_PRESETS: [AutoCutPreset; 3] = [
    AutoCutPreset {
        id: "natural",
        minimum_pause: 0.500,
        keep_before_speech: 0.150,
        keep_after_speech: 0.250,
    },
    AutoCutPreset {
        id: "balanced",
        minimum_pause: 0.350,
        keep_before_speech: 0.100,
        keep_after_speech: 0.180,
    },
    AutoCutPreset {
        id: "tight",
        minimum_pause: 0.225,
        keep_before_speech: 0.075,
        keep_after_speech: 0.130,
    },
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct AutoCutEditSettings {
    pub(crate) minimum_pause: f64,
    pub(crate) keep_before_speech: f64,
    pub(crate) keep_after_speech: f64,
    pub(crate) boundary_refinement: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AutoCutExportRequest {
    pub(crate) input: String,
    pub(crate) cuts: Vec<KeepInterval>,
    pub(crate) format: String,
    pub(crate) quality: String,
    pub(crate) resolution: String,
    pub(crate) linked_tracks: Vec<LinkedTrackRequest>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LinkedTrackRequest {
    pub(crate) path: String,
    pub(crate) offset: f64,
}

pub(crate) async fn waveform_for(input: &Path) -> Result<Vec<f32>, String> {
    let output = process_timeout::output_within(
        hidden_command("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(input)
            .args(["-vn", "-ac", "1", "-ar", "200", "-f", "f32le", "pipe:1"]),
        process_timeout::FULL_DECODE,
    )
    .await
    .map_err(|e| format!("FFmpeg waveform analysis could not start: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let samples: Vec<f32> = output
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]).abs().min(1.0))
        .collect();
    if samples.len() <= 1200 {
        return Ok(samples);
    }
    let chunk = (samples.len() as f64 / 1200.0).ceil() as usize;
    Ok(samples
        .chunks(chunk)
        .map(|values| values.iter().copied().fold(0.0_f32, f32::max))
        .collect())
}

fn ensure_not_cancelled(state: &JobState) -> Result<(), String> {
    if state.cancelled.load(Ordering::Relaxed) {
        Err(smartcut_audio::CANCELLED.into())
    } else {
        Ok(())
    }
}

pub(crate) fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return -60.0;
    }
    sorted[((sorted.len() - 1) as f64 * fraction.clamp(0.0, 1.0)).round() as usize]
}

pub(crate) fn median(values: &mut [f64], fallback: f64) -> f64 {
    if values.is_empty() {
        return fallback;
    }
    values.sort_by(f64::total_cmp);
    percentile(values, 0.5)
}

pub(crate) fn recommend_from_levels(levels: &[f64], block_seconds: f64) -> AutoCutRecommendation {
    let mut sorted = levels.to_vec();
    sorted.sort_by(f64::total_cmp);
    let noise = percentile(&sorted, 0.20);
    let speech = percentile(&sorted, 0.82);
    let spread = (speech - noise).max(6.0);
    let level_threshold = (noise + spread * 0.42).clamp(-55.0, -18.0).round();
    let active: Vec<bool> = levels
        .iter()
        .map(|level| *level >= level_threshold)
        .collect();
    let mut speech_runs = Vec::new();
    let mut silence_runs = Vec::new();
    let mut index = 0;
    while index < active.len() {
        let state = active[index];
        let start = index;
        while index < active.len() && active[index] == state {
            index += 1;
        }
        let length = (index - start) as f64 * block_seconds;
        if state {
            speech_runs.push(length);
        } else if start > 0 && index < active.len() {
            silence_runs.push(length);
        }
    }
    let typical_gap = median(&mut silence_runs, 0.35);
    let typical_speech = median(&mut speech_runs, 0.4);
    let min_silence = (typical_gap * 0.70).clamp(0.18, 0.80);
    let min_speech = (typical_speech * 0.25).clamp(0.10, 0.30);
    // Auto remains conservative: level analysis may choose detector values,
    // but it never turns a clean noise floor into hyper-aggressive editing.
    let minimum_pause = (typical_gap * 0.80).clamp(0.35, 0.65);
    AutoCutRecommendation {
        threshold: (0.65 - spread * 0.005).clamp(0.35, 0.65),
        min_silence: (min_silence * 100.0).round() / 100.0,
        min_speech: (min_speech * 100.0).round() / 100.0,
        minimum_pause: (minimum_pause * 100.0).round() / 100.0,
        keep_before_speech: 0.10,
        keep_after_speech: 0.18,
        noise_floor_db: (noise * 10.0).round() / 10.0,
        speech_level_db: (speech * 10.0).round() / 10.0,
    }
}

pub(crate) async fn cached_vad_data(
    state: &JobState,
    input: &Path,
) -> Result<(Arc<Vec<f32>>, Arc<Vec<i16>>), String> {
    let metadata = std::fs::metadata(input).map_err(|e| e.to_string())?;
    let path = std::fs::canonicalize(input).unwrap_or_else(|_| input.to_path_buf());
    let modified = metadata.modified().ok();
    if let Some(cache) = state
        .vad_cache
        .lock()
        .map_err(|_| "VAD cache lock failed")?
        .as_ref()
    {
        if cache.path == path && cache.len == metadata.len() && cache.modified == modified {
            return Ok((cache.scores.clone(), cache.samples.clone()));
        }
    }
    // Decoding and scoring a long recording takes a while: callers hold the
    // job slot so it cannot overlap a render, and so Cancel can stop it.
    let samples = Arc::new(
        smartcut_audio::decode_mono::<i16, 2>(state, input, 16_000, "s16le", i16::from_le_bytes)
            .await?,
    );
    let samples_for_scoring = samples.clone();
    let cancelled = state.cancelled.clone();
    let scores = Arc::new(
        tokio::task::spawn_blocking(move || -> Result<Vec<f32>, String> {
            let mut detector =
                load_silero_vad().map_err(|e| format!("Silero V6 could not initialize: {e}"))?;
            let mut scores = Vec::with_capacity(samples_for_scoring.len().div_ceil(512));
            for (index, chunk) in samples_for_scoring.chunks(512).enumerate() {
                if index % 256 == 0 && cancelled.load(Ordering::Relaxed) {
                    return Err(smartcut_audio::CANCELLED.into());
                }
                let mut frame = vec![0.0_f32; 512];
                for (output, sample) in frame.iter_mut().zip(chunk) {
                    *output = *sample as f32 / 32768.0;
                }
                let probability = detector
                    .forward_chunk(&frame, 16_000)
                    .map_err(|e| format!("Silero V6 inference failed: {e}"))?;
                scores.push(probability[[0, 0]]);
            }
            Ok(scores)
        })
        .await
        .map_err(|e| e.to_string())??,
    );
    *state
        .vad_cache
        .lock()
        .map_err(|_| "VAD cache lock failed")? = Some(VadCache {
        path,
        modified,
        len: metadata.len(),
        scores: scores.clone(),
        samples: samples.clone(),
    });
    Ok((scores, samples))
}

#[cfg(test)]
pub(crate) async fn cached_vad_scores(
    state: &JobState,
    input: &Path,
) -> Result<Arc<Vec<f32>>, String> {
    cached_vad_data(state, input)
        .await
        .map(|(scores, _)| scores)
}

pub(crate) fn keeps_from_vad_scores(
    scores: &[f32],
    threshold: f64,
    min_silence: f64,
    min_speech: f64,
    padding: f64,
    duration: f64,
) -> Vec<KeepInterval> {
    const CHUNK_SECONDS: f64 = 512.0 / 16000.0;
    if scores.is_empty() || duration <= 0.0 {
        return Vec::new();
    }

    // Keep this state machine in parity with cobanov/autocut's vad.rs. The
    // model scores are deliberately not smoothed and there is no attack
    // debounce: speech enters on the first score at threshold and exits below
    // threshold - 0.15. min_silence then merges short non-speech runs, and
    // min_speech is applied only after that merge.
    let release = (threshold - 0.15).max(0.05) as f32;
    let debug_enabled = std::env::var("CONTAINER_VAD_DEBUG")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mut debug_log = String::new();
    if debug_enabled {
        let _ = writeln!(
            debug_log,
            "CONTAINER Silero VAD parity trace\nthreshold={threshold:.3} release={release:.3} min_silence={min_silence:.3}s min_speech={min_speech:.3}s padding={padding:.3}s duration={duration:.3}s chunks={} chunk_seconds={CHUNK_SECONDS:.3}",
            scores.len()
        );
    }

    let mut speaking = false;
    let mut flags = Vec::with_capacity(scores.len());
    for (index, &score) in scores.iter().enumerate() {
        let event = if !speaking && score >= threshold as f32 {
            speaking = true;
            " ENTER_SPEECH"
        } else if speaking && score < release {
            speaking = false;
            " EXIT_SPEECH"
        } else {
            ""
        };
        flags.push(speaking);
        if debug_enabled {
            let _ = writeln!(
                debug_log,
                "FRAME index={index} sample={} time={:.3}s probability={score:.6} speech={}{}",
                index * 512,
                index as f64 * CHUNK_SECONDS,
                speaking,
                event
            );
        }
    }

    let mut regions = Vec::new();
    let mut candidate_start = None;
    for (index, is_speech) in flags.iter().copied().enumerate() {
        match (candidate_start, is_speech) {
            (None, true) => {
                candidate_start = Some(index);
                if debug_enabled {
                    let _ = writeln!(debug_log, "CANDIDATE_START chunk={index}");
                }
            }
            (Some(start), false) => {
                regions.push((start, index));
                candidate_start = None;
                if debug_enabled {
                    let _ = writeln!(debug_log, "CANDIDATE_END chunks={start}..{index}");
                }
            }
            _ => {}
        }
    }
    if let Some(start) = candidate_start {
        regions.push((start, flags.len()));
        if debug_enabled {
            let _ = writeln!(
                debug_log,
                "CANDIDATE_END chunks={start}..{} EOF",
                flags.len()
            );
        }
    }

    let min_gap = vad_chunks_for_seconds(min_silence, CHUNK_SECONDS);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in regions {
        if let Some(last) = merged.last_mut() {
            let gap = start.saturating_sub(last.1);
            // AutoCut uses a strict comparison: a silence must be at least
            // min_silence long to split two speech regions.
            if gap < min_gap {
                if debug_enabled {
                    let _ = writeln!(
                        debug_log,
                        "MERGE previous={}..{} next={start}..{end} gap_chunks={gap}",
                        last.0, last.1
                    );
                }
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let min_len = vad_chunks_for_seconds(min_speech, CHUNK_SECONDS);
    let mut keeps: Vec<KeepInterval> = merged
        .into_iter()
        .filter_map(|(start, end)| {
            let length = end.saturating_sub(start);
            let peak = scores[start..end]
                .iter()
                .copied()
                .fold(0.0_f32, f32::max);
            // Preserve compact, high-confidence interjections (at least 64 ms)
            // while still rejecting isolated one-frame clicks/spikes.
            let strong_short_speech = length >= 2
                && peak >= ((threshold as f32 + 0.30).min(0.90));
            if length < min_len && !strong_short_speech {
                if debug_enabled {
                    let _ = writeln!(
                        debug_log,
                        "REJECT_SHORT chunks={start}..{end} length_chunks={length} peak={peak:.4} required_chunks={min_len}",
                    );
                }
                return None;
            }
            let padded_start = (start as f64 * CHUNK_SECONDS - padding).max(0.0);
            let padded_end = (end as f64 * CHUNK_SECONDS + padding).min(duration);
            if debug_enabled {
                let _ = writeln!(
                    debug_log,
                    "PADDING chunks={start}..{end} raw={:.3}..{:.3} padded={padded_start:.3}..{padded_end:.3}",
                    start as f64 * CHUNK_SECONDS,
                    end as f64 * CHUNK_SECONDS
                );
            }
            Some(KeepInterval {
                start: padded_start,
                end: padded_end,
                enabled: true,
            })
        })
        .collect();

    let mut normalized: Vec<KeepInterval> = Vec::new();
    for keep in keeps.drain(..) {
        if let Some(last) = normalized.last_mut() {
            if keep.start <= last.end {
                if debug_enabled {
                    let _ = writeln!(
                        debug_log,
                        "MERGE_PADDED previous={:.3}..{:.3} next={:.3}..{:.3}",
                        last.start, last.end, keep.start, keep.end
                    );
                }
                last.end = last.end.max(keep.end);
                continue;
            }
        }
        normalized.push(keep)
    }

    if debug_enabled {
        for (index, keep) in normalized.iter().enumerate() {
            let _ = writeln!(
                debug_log,
                "FINAL_KEEP index={} start={:.3} end={:.3} duration={:.3}",
                index + 1,
                keep.start,
                keep.end,
                keep.end - keep.start
            );
        }
        let path = std::env::var_os("CONTAINER_VAD_DEBUG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("container-vad-debug.log"));
        if let Err(error) = std::fs::write(&path, debug_log) {
            eprintln!(
                "Could not write VAD debug trace to {}: {error}",
                path.display()
            );
        } else {
            eprintln!("VAD debug trace written to {}", path.display());
        }
    }
    normalized
}

pub(crate) fn vad_chunks_for_seconds(seconds: f64, chunk_seconds: f64) -> usize {
    (seconds.max(0.0) / chunk_seconds).ceil() as usize
}

pub(crate) fn refine_boundary(samples: &[i16], sample: usize, direction: i8) -> usize {
    const SAMPLE_RATE: usize = 16_000;
    const SEARCH: usize = SAMPLE_RATE * 120 / 1000;
    const WINDOW: usize = SAMPLE_RATE * 10 / 1000;
    if samples.is_empty() || sample == 0 || sample >= samples.len() {
        return sample.min(samples.len());
    }
    let rms = |start: usize| {
        let end = (start + WINDOW).min(samples.len());
        if end <= start {
            return 0.0;
        }
        let energy = samples[start..end]
            .iter()
            .map(|value| {
                let normalized = *value as f64 / 32768.0;
                normalized * normalized
            })
            .sum::<f64>();
        (energy / (end - start) as f64).sqrt()
    };
    let reference = if direction < 0 {
        rms(sample.min(samples.len().saturating_sub(WINDOW)))
    } else {
        rms(sample.saturating_sub(WINDOW))
    };
    let (from, to) = if direction < 0 {
        (sample.saturating_sub(SEARCH), sample)
    } else {
        (sample, (sample + SEARCH).min(samples.len()))
    };
    let mut quietest = (sample, f64::MAX);
    for position in (from..to).step_by(WINDOW.max(1)) {
        let energy = rms(position);
        if energy < quietest.1 {
            quietest = (position, energy);
        }
    }
    // Background music/gameplay may never become quiet. In that case VAD is
    // the safer boundary and refinement deliberately does nothing.
    if quietest.1 <= 0.006 || quietest.1 <= reference * 0.65 {
        if direction < 0 {
            quietest.0.min(sample)
        } else {
            (quietest.0 + WINDOW).max(sample).min(samples.len())
        }
    } else {
        sample
    }
}

pub(crate) fn natural_keeps_from_vad_scores(
    scores: &[f32],
    samples: &[i16],
    threshold: f64,
    min_silence: f64,
    min_speech: f64,
    settings: AutoCutEditSettings,
    duration: f64,
) -> (Vec<KeepInterval>, usize) {
    const SAMPLE_RATE: f64 = 16_000.0;
    let mut speech =
        keeps_from_vad_scores(scores, threshold, min_silence, min_speech, 0.0, duration);
    let minimum_pause_samples = (settings.minimum_pause * SAMPLE_RATE).round() as usize;
    let mut stable: Vec<(usize, usize)> = Vec::with_capacity(speech.len());
    for region in speech.drain(..) {
        let start = (region.start * SAMPLE_RATE).round().max(0.0) as usize;
        let end = (region.end * SAMPLE_RATE).round().max(0.0) as usize;
        if let Some(previous) = stable.last_mut() {
            if start.saturating_sub(previous.1) < minimum_pause_samples {
                previous.1 = previous.1.max(end);
                continue;
            }
        }
        stable.push((start, end));
    }
    let mut padded = Vec::with_capacity(stable.len());
    for (mut start, mut end) in stable {
        if settings.boundary_refinement {
            start = refine_boundary(samples, start, -1);
            end = refine_boundary(samples, end, 1);
        }
        padded.push(KeepInterval {
            start: (start as f64 / SAMPLE_RATE - settings.keep_before_speech).max(0.0),
            end: (end as f64 / SAMPLE_RATE + settings.keep_after_speech).min(duration),
            enabled: true,
        });
    }
    let overlaps = padded
        .windows(2)
        .filter(|pair| pair[1].start <= pair[0].end)
        .count();
    (normalize_keep_intervals(&padded, duration), overlaps)
}

#[tauri::command]
pub(crate) async fn recommend_autocut_settings(
    state: State<'_, JobState>,
    path: String,
    job_id: Option<String>,
) -> Result<AutoCutRecommendation, String> {
    let input = PathBuf::from(path);
    if !input.is_file() {
        return Err("Analysis media was not found.".into());
    }
    let _job = JobLease::acquire(&state, job_id)?;
    let samples =
        smartcut_audio::decode_mono::<f32, 4>(&state, &input, 1_000, "f32le", f32::from_le_bytes)
            .await?;
    if samples.len() < 100 {
        return Err("Not enough audio to calculate automatic settings.".into());
    }
    let levels: Vec<f64> = samples
        .chunks(20)
        .map(|block| {
            let mean = block
                .iter()
                .map(|sample| (*sample as f64).powi(2))
                .sum::<f64>()
                / block.len() as f64;
            20.0 * mean.sqrt().max(0.000001).log10()
        })
        .collect();
    Ok(recommend_from_levels(&levels, 0.02))
}

#[tauri::command]
pub(crate) async fn compute_autocut_waveform(path: String) -> Result<Vec<f32>, String> {
    let input = PathBuf::from(path);
    if !input.is_file() {
        return Err("Media file was not found.".into());
    }
    waveform_for(&input).await
}

#[tauri::command]
pub(crate) async fn compute_video_filmstrip(path: String) -> Result<String, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err("Media file was not found.".into());
    }
    let info = probe_media(path).await?;
    if info.kind != "video" {
        return Err("Filmstrip preview requires a video file.".into());
    }
    let duration = info.duration.unwrap_or(1.0).max(0.1);
    // Seek before each input so a long/high-FPS recording is not decoded from
    // start to finish merely to obtain a few representative thumbnails.
    const TILES: usize = 7;
    let mut command = hidden_command("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error"]);
    for index in 0..TILES {
        let at = duration * (index as f64 + 0.5) / TILES as f64;
        command.args(["-ss", &format!("{at:.6}"), "-i"]).arg(&input);
    }
    let mut filters = Vec::with_capacity(TILES + 1);
    for index in 0..TILES {
        filters.push(format!("[{index}:v]scale=160:90:force_original_aspect_ratio=decrease,pad=160:90:(ow-iw)/2:(oh-ih)/2:black[v{index}]"));
    }
    let inputs = (0..TILES)
        .map(|index| format!("[v{index}]"))
        .collect::<String>();
    filters.push(format!("{inputs}hstack=inputs={TILES}[strip]"));
    let filter = filters.join(";");
    command.args([
        "-filter_complex",
        &filter,
        "-map",
        "[strip]",
        "-frames:v",
        "1",
        "-f",
        "image2pipe",
        "-vcodec",
        "png",
        "pipe:1",
    ]);
    let output = process_timeout::output_within(&mut command, process_timeout::PREVIEW)
        .await
        .map_err(|error| format!("FFmpeg filmstrip could not start: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    if output.stdout.is_empty() {
        return Err("FFmpeg did not produce a filmstrip preview.".into());
    }
    Ok(format!(
        "data:image/png;base64,{}",
        BASE64.encode(output.stdout)
    ))
}

#[tauri::command]
pub(crate) fn autocut_presets() -> Vec<AutoCutPreset> {
    AUTOCUT_PRESETS.to_vec()
}

pub(crate) fn autocut_edit_settings(request: &AutoCutRequest) -> AutoCutEditSettings {
    // Compatibility path for pre-asymmetric clients/saved state: the old
    // padding value becomes both sides and minimum-pause does not alter the
    // already validated min-silence segmentation.
    let legacy_padding = request.padding.unwrap_or(0.12);
    AutoCutEditSettings {
        minimum_pause: request.minimum_pause.unwrap_or(request.min_silence),
        keep_before_speech: request.keep_before_speech.unwrap_or(legacy_padding),
        keep_after_speech: request.keep_after_speech.unwrap_or(legacy_padding),
        boundary_refinement: request.boundary_refinement.unwrap_or(false),
    }
}

#[tauri::command]
pub(crate) async fn analyze_autocut(
    state: State<'_, JobState>,
    request: AutoCutRequest,
    job_id: Option<String>,
) -> Result<AutoCutAnalysis, String> {
    // The whole analysis holds the job slot: probing, decoding, scoring and
    // the waveform. Cancel is checked between phases so a cancelled run never
    // returns cuts.
    let _job = JobLease::acquire(&state, job_id)?;
    let info = probe_media(request.input.clone()).await?;
    if info.kind != "video" {
        return Err("SmartCut requires a video file.".into());
    }
    let duration = info.duration.ok_or("Video duration could not be read.")?;
    check_range(request.threshold, 0.05, 0.95, "Threshold")?;
    check_range(request.min_silence, 0.03, 30.0, "Minimum silence")?;
    check_range(request.min_speech, 0.03, 30.0, "Minimum speech")?;
    let edit = autocut_edit_settings(&request);
    check_range(edit.minimum_pause, 0.03, 30.0, "Minimum pause")?;
    check_range(edit.keep_before_speech, 0.0, 5.0, "Keep before speech")?;
    check_range(edit.keep_after_speech, 0.0, 5.0, "Keep after speech")?;
    let input = PathBuf::from(request.analysis_input.as_deref().unwrap_or(&request.input));
    if !input.is_file() {
        return Err("Analysis audio file was not found.".into());
    }
    let analysis_info = probe_media(input.to_string_lossy().to_string()).await?;
    if analysis_info.audio_codec.is_none() && analysis_info.kind != "audio" {
        return Err("Silence detection requires an audio stream.".into());
    }
    ensure_not_cancelled(&state)?;
    let (scores, samples) = cached_vad_data(&state, &input).await?;
    ensure_not_cancelled(&state)?;
    let (cuts, overlaps_before_normalization) = natural_keeps_from_vad_scores(
        &scores,
        &samples,
        request.threshold,
        request.min_silence,
        request.min_speech,
        edit,
        duration,
    );
    let waveform = waveform_for(&input).await?;
    ensure_not_cancelled(&state)?;
    Ok(AutoCutAnalysis {
        cuts,
        waveform,
        duration,
        boundary_refinement: edit.boundary_refinement,
        overlaps_before_normalization,
    })
}

pub(crate) fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn file_url(path: &Path) -> Result<String, String> {
    Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|_| {
            format!(
                "File path could not be converted to a URL: {}",
                path.display()
            )
        })
}

pub(crate) fn normalize_keep_intervals(cuts: &[KeepInterval], duration: f64) -> Vec<KeepInterval> {
    const TIMESTAMP_EPSILON: f64 = 0.001;
    // `f64::clamp` panics when the upper bound is NaN or below zero; malformed
    // probes can report either, so treat them as "no usable range".
    if !duration.is_finite() || duration < 0.0 {
        return Vec::new();
    }
    let mut values: Vec<_> = cuts
        .iter()
        .filter(|c| c.enabled)
        .map(|c| KeepInterval {
            start: c.start.clamp(0.0, duration),
            end: c.end.clamp(0.0, duration),
            enabled: true,
        })
        .filter(|c| c.end - c.start >= 0.01)
        .collect();
    values.sort_by(|a, b| a.start.total_cmp(&b.start));
    let debug = std::env::var("CONTAINER_AUTOCUT_DEBUG")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mut normalized: Vec<KeepInterval> = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        if let Some(previous) = normalized.last_mut() {
            let gap = value.start - previous.end;
            if debug {
                eprintln!(
                    "AUTOCUT REGION input={} start={:.6} end={:.6} duration={:.6} previous_end={:.6} gap={:.6} overlap={:.6}",
                    index + 1,
                    value.start,
                    value.end,
                    value.end - value.start,
                    previous.end,
                    gap,
                    (-gap).max(0.0)
                );
            }
            if value.start <= previous.end + TIMESTAMP_EPSILON {
                previous.end = previous.end.max(value.end);
                continue;
            }
        } else if debug {
            eprintln!(
                "AUTOCUT REGION input=1 start={:.6} end={:.6} duration={:.6}",
                value.start,
                value.end,
                value.end - value.start
            );
        }
        normalized.push(value);
    }
    debug_assert!(normalized
        .windows(2)
        .all(|pair| pair[0].end < pair[1].start));
    normalized
}

pub(crate) fn enabled_cuts(
    cuts: &[KeepInterval],
    duration: f64,
) -> Result<Vec<KeepInterval>, String> {
    let normalized = normalize_keep_intervals(cuts, duration);
    if normalized.is_empty() {
        Err("At least one enabled cut is required.".into())
    } else {
        Ok(normalized)
    }
}

pub(crate) const SMARTCUT_RESOLUTIONS: [&str; 4] = ["source", "1080", "720", "480"];

pub(crate) fn validate_smartcut_resolution(resolution: &str) -> Result<(), String> {
    if SMARTCUT_RESOLUTIONS.contains(&resolution) {
        Ok(())
    } else {
        Err("Invalid SmartCut resolution.".into())
    }
}

pub(crate) fn autocut_filter_graph(
    cuts: &[KeepInterval],
    has_audio: bool,
    resolution: &str,
) -> String {
    let mut graph = String::new();
    if cuts.len() > 1 {
        let video_outputs = (0..cuts.len())
            .map(|index| format!("[vsrc{index}]"))
            .collect::<String>();
        let _ = writeln!(graph, "[0:v]split={}{video_outputs};", cuts.len());
        if has_audio {
            let audio_outputs = (0..cuts.len())
                .map(|index| format!("[asrc{index}]"))
                .collect::<String>();
            let _ = writeln!(graph, "[0:a]asplit={}{audio_outputs};", cuts.len());
        }
    }
    for (index, cut) in cuts.iter().enumerate() {
        let video_source = if cuts.len() == 1 {
            "[0:v]".to_string()
        } else {
            format!("[vsrc{index}]")
        };
        let _ = writeln!(
            graph,
            "{video_source}trim=start={:.6}:end={:.6},setpts=PTS-STARTPTS[v{index}];",
            cut.start, cut.end
        );
        if has_audio {
            let audio_source = if cuts.len() == 1 {
                "[0:a]".to_string()
            } else {
                format!("[asrc{index}]")
            };
            let _ = writeln!(
                graph,
                "{audio_source}atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS[a{index}];",
                cut.start, cut.end
            );
        }
    }
    let inputs = (0..cuts.len())
        .map(|index| {
            if has_audio {
                format!("[v{index}][a{index}]")
            } else {
                format!("[v{index}]")
            }
        })
        .collect::<String>();
    let video_label = if resolution == "source" {
        "vout"
    } else {
        "vconcat"
    };
    let _ = writeln!(
        graph,
        "{inputs}concat=n={}:v=1:a={}[{video_label}]{};",
        cuts.len(),
        usize::from(has_audio),
        if has_audio { "[aout]" } else { "" }
    );
    if resolution != "source" {
        let _ = writeln!(graph, "[vconcat]scale=-2:{resolution}[vout];");
    }
    graph
}

pub(crate) async fn export_autocut_inner(
    app: Option<&AppHandle>,
    state: &JobState,
    request: AutoCutExportRequest,
) -> Result<JobResult, String> {
    reset_legacy_job_cancellation(state);
    validate_smartcut_resolution(&request.resolution)?;
    let info = probe_media(request.input.clone()).await?;
    let duration = info.duration.ok_or("Video duration could not be read.")?;
    let cuts = enabled_cuts(&request.cuts, duration)?;
    let input = PathBuf::from(&request.input);
    if request.format.eq_ignore_ascii_case("fcpxml") {
        let output = unique_output(&input, "smartcut", "fcpxml")?;
        let fps_value = info.fps.unwrap_or(30.0);
        let fps = fps_value.round().max(1.0) as u64;
        let reference_tc = parse_smpte(info.start_timecode.as_deref(), fps_value);
        let frame_duration = if (fps_value - 29.97).abs() < 0.1 {
            "1001/30000s".to_string()
        } else if (fps_value - 59.94).abs() < 0.1 {
            "1001/60000s".to_string()
        } else if (fps_value - 23.976).abs() < 0.1 {
            "1001/24000s".to_string()
        } else {
            format!("1/{fps}s")
        };
        let tc_format = if info
            .start_timecode
            .as_deref()
            .is_some_and(|tc| tc.contains(';'))
        {
            "DF"
        } else {
            "NDF"
        };
        let uri = file_url(&input)?;
        let mut resources = format!("<format id=\"r1\" name=\"FFVideoFormat\" frameDuration=\"{frame_duration}\"/><asset id=\"r2\" name=\"{}\" src=\"{}\" start=\"{reference_tc:.6}s\" duration=\"{duration:.6}s\" hasVideo=\"1\" hasAudio=\"{}\"/>", xml_escape(&info.name), xml_escape(&uri), if info.audio_codec.is_some() { 1 } else { 0 });
        let mut prepared = Vec::new();
        let mut next_video_lane = 1_i32;
        let mut next_audio_lane = -1_i32;
        for (track_index, track) in request.linked_tracks.iter().enumerate() {
            let linked_info = probe_media(track.path.clone()).await?;
            let linked_duration = linked_info.duration.unwrap_or(duration);
            let linked_tc = parse_smpte(linked_info.start_timecode.as_deref(), fps_value);
            let id = format!("r{}", track_index + 3);
            let linked_uri = file_url(Path::new(&track.path))?;
            resources.push_str(&format!("<asset id=\"{id}\" name=\"{}\" src=\"{}\" start=\"{linked_tc:.6}s\" duration=\"{linked_duration:.6}s\" hasVideo=\"{}\" hasAudio=\"{}\"/>", xml_escape(&linked_info.name), xml_escape(&linked_uri), if linked_info.kind == "video" { 1 } else { 0 }, if linked_info.kind == "audio" || linked_info.audio_codec.is_some() { 1 } else { 0 }));
            let lane = if linked_info.kind == "video" {
                let value = next_video_lane;
                next_video_lane += 1;
                value
            } else {
                let value = next_audio_lane;
                next_audio_lane -= 1;
                value
            };
            prepared.push((
                id,
                linked_info,
                linked_duration,
                linked_tc,
                track.offset,
                lane,
            ));
        }
        let mut cursor = 0.0;
        let mut clips = String::new();
        for (index, cut) in cuts.iter().enumerate() {
            let len = cut.end - cut.start;
            let mut connected = String::new();
            for (id, linked_info, linked_duration, linked_tc, offset, lane) in &prepared {
                let visible_from = cut.start.max(*offset);
                let visible_to = cut.end.min(*offset + *linked_duration);
                if visible_to > visible_from {
                    let clip_offset = cursor + (visible_from - cut.start);
                    let source_start = linked_tc + (visible_from - offset);
                    connected.push_str(&format!("<asset-clip name=\"{}\" ref=\"{id}\" lane=\"{lane}\" offset=\"{clip_offset:.6}s\" start=\"{source_start:.6}s\" duration=\"{:.6}s\" tcFormat=\"{tc_format}\"/>", xml_escape(&linked_info.name), visible_to-visible_from));
                }
            }
            if connected.is_empty() {
                clips.push_str(&format!("<asset-clip name=\"Cut {}\" ref=\"r2\" offset=\"{cursor:.6}s\" start=\"{:.6}s\" duration=\"{len:.6}s\" tcFormat=\"{tc_format}\"/>", index+1,reference_tc+cut.start));
            } else {
                clips.push_str(&format!("<asset-clip name=\"Cut {}\" ref=\"r2\" offset=\"{cursor:.6}s\" start=\"{:.6}s\" duration=\"{len:.6}s\" tcFormat=\"{tc_format}\">{connected}</asset-clip>", index+1,reference_tc+cut.start));
            }
            cursor += len;
        }
        let xml=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.11\"><resources>{resources}</resources><library><event name=\"CONTAINER SmartCut\"><project name=\"{}\"><sequence format=\"r1\" duration=\"{cursor:.6}s\" tcFormat=\"{tc_format}\"><spine>{clips}</spine></sequence></project></event></library></fcpxml>",xml_escape(&safe_stem(&input)));
        std::fs::write(&output, xml).map_err(|e| e.to_string())?;
        if let Some(app) = app {
            allow_asset_file(app, &output)?;
        }
        return Ok(JobResult {
            output: output.to_string_lossy().into(),
            elapsed: 0.0,
        });
    }
    let output = unique_output(&input, "smartcut", "mp4")?;
    let graph = autocut_filter_graph(&cuts, info.audio_codec.is_some(), &request.resolution);
    if std::env::var_os("CONTAINER_AUTOCUT_DEBUG").is_some() {
        eprintln!("AUTOCUT FILTER GRAPH\n{graph}");
    }
    let crf = match request.quality.as_str() {
        "preserve" => "14",
        "lossless" => "0",
        "high" => "18",
        "small" => "26",
        _ => "22",
    };
    let audio_bitrate = match request.quality.as_str() {
        "lossless" => "320k",
        "high" => "192k",
        "small" => "96k",
        _ => "128k",
    };
    let kept_total: f64 = cuts.iter().map(|c| c.end - c.start).sum();
    let started = Instant::now();
    let mut command = hidden_command("ffmpeg");
    command
        .args(["-hide_banner", "-y", "-i"])
        .arg(&input)
        .arg("-filter_complex")
        .arg(&graph)
        .args(["-map", "[vout]"]);
    if let Some(fps) = info.fps.filter(|fps| fps.is_finite() && *fps > 0.0) {
        command.args(["-r", &format!("{fps:.10}"), "-fps_mode", "cfr"]);
    }
    if request.quality == "source" {
        let bitrate = info
            .bitrate
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or("Source video bitrate is unavailable. Choose Lossless or High quality.")?;
        let target = (bitrate.round() as u64).max(1000);
        command.args([
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-b:v",
            &target.to_string(),
        ]);
    } else {
        let preset = if request.quality == "preserve" {
            "fast"
        } else {
            "veryfast"
        };
        command.args(["-c:v", "libx264", "-preset", preset, "-crf", crf]);
    }
    if info.audio_codec.is_some() {
        let source_audio = info
            .audio_tracks
            .first()
            .and_then(|stream| stream.bitrate)
            .map(|rate| format!("{}", rate.clamp(32000.0, 320000.0).round() as u64));
        let audio_rate = if matches!(request.quality.as_str(), "source" | "preserve") {
            source_audio.as_deref().unwrap_or("192k")
        } else {
            audio_bitrate
        };
        command.args(["-map", "[aout]", "-c:a", "aac", "-b:a", audio_rate]);
    } else {
        command.arg("-an");
    }
    command
        .args(["-movflags", "+faststart", "-progress", "pipe:1", "-nostats"])
        .arg(&output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    state.inspector.start(
        &command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    );
    let mut child = command
        .spawn()
        .map_err(|e| format!("FFmpeg could not start: {e}"))?;
    let active_process = ActiveProcess::register(state, child.id())?;
    let stdout = child.stdout.take().ok_or("FFmpeg progress unavailable.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("FFmpeg diagnostics unavailable.")?;
    let stderr_task = line_tail::spawn_tail(stderr);
    let mut lines = BufReader::new(stdout).lines();
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
            let seconds = value.parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
            if let Some(app) = app {
                let _ = app.emit(
                    "container-progress",
                    ProgressEvent {
                        job_id: progress_job_id(state),
                        percent: (seconds / kept_total * 100.0).clamp(0.0, 99.9),
                        time: started.elapsed().as_secs_f64(),
                        speed: "—".into(),
                        frame: "—".into(),
                        status: "exporting smartcut".into(),
                    },
                );
            }
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
        return Err(errors
            .into_iter()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n"));
    }
    if let Some(app) = app {
        allow_asset_file(app, &output)?;
    }
    Ok(JobResult {
        output: output.to_string_lossy().into(),
        elapsed: started.elapsed().as_secs_f64(),
    })
}

#[tauri::command]
pub(crate) async fn export_autocut(
    app: AppHandle,
    state: State<'_, JobState>,
    request: AutoCutExportRequest,
    job_id: Option<String>,
) -> Result<JobResult, String> {
    let _job = JobLease::acquire(&state, job_id)?;
    export_autocut_inner(Some(&app), &state, request).await
}
