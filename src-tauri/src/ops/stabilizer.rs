//! Two-pass video stabilization.

use crate::*;

pub(crate) async fn run_stabilizer(
    app: Option<&AppHandle>,
    state: &JobState,
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<JobResult, String> {
    require_filters(&["vidstabdetect", "vidstabtransform"], "Video Stabilizer").await?;
    let (shakiness, accuracy, smoothing) = match param(&request.params, "strength")? {
        "light" => (4, 9, 10),
        "medium" => (6, 12, 20),
        "strong" => (8, 15, 30),
        _ => return Err("Invalid stabilization strength.".into()),
    };
    let input = PathBuf::from(&request.input);
    let output = unique_output(&input, "stabilizer", "mp4")?;
    let temp = operation_temp_dir("stabilizer")?;
    let transforms = temp.join("motion.trf");
    let null_output = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };
    let duration = info.duration.unwrap_or(0.0);
    let started = Instant::now();
    let (encoder_name, encoder_args) = fastest_h264_encoder().await;
    let result = async {
        run_ffmpeg_stage(app, state, vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(), "-i".into(), request.input.clone(),
            "-vf".into(), format!("vidstabdetect=shakiness={shakiness}:accuracy={accuracy}:result='{}'", ffmpeg_filter_path(&transforms)),
            "-an".into(), "-f".into(), "null".into(), null_output.into(),
        ], duration, &started, 0.0, 35.0, "analyzing motion").await?;
        let mut transform_args = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(), "-i".into(), request.input.clone(),
            "-map".into(), "0:v:0".into(), "-map".into(), "0:a?".into(), "-vf".into(),
            format!("vidstabtransform=input='{}':smoothing={smoothing}:optzoom=2:interpol=bilinear,setsar=1", ffmpeg_filter_path(&transforms)),
        ];
        transform_args.extend(encoder_args);
        append_mp4_audio_codec(&mut transform_args, info);
        transform_args.push(output.to_string_lossy().to_string());
        let status = format!("stabilizing video · {encoder_name}");
        run_ffmpeg_stage(app, state, transform_args, duration, &started, 35.0, 64.0, &status).await?;
        if let Some(app) = app { allow_asset_file(app, &output)?; }
        Ok(JobResult { output: output.to_string_lossy().to_string(), elapsed: started.elapsed().as_secs_f64() })
    }.await;
    let _ = std::fs::remove_dir_all(&temp);
    if result.is_err() {
        let _ = std::fs::remove_file(&output);
    }
    result
}
