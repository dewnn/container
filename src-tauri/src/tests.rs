//! Integration-style tests for the crate (real media, FFmpeg graphs, jobs).
#[test]
fn job_identity_rejects_overlap_and_stale_cancel() {
    let state = super::JobState::default();
    let first = super::JobLease::acquire(&state, Some("first".into())).unwrap();
    assert!(super::JobLease::acquire(&state, Some("second".into())).is_err());
    assert_eq!(super::progress_job_id(&state).as_deref(), Some("first"));
    drop(first);
    let second = super::JobLease::acquire(&state, Some("second".into())).unwrap();
    assert!(super::request_job_cancellation(&state, Some("first"))
        .unwrap()
        .is_none());
    assert!(!state.cancelled.load(std::sync::atomic::Ordering::Relaxed));
    assert!(super::request_job_cancellation(&state, Some("second"))
        .unwrap()
        .is_some());
    assert!(state.cancelled.load(std::sync::atomic::Ordering::Relaxed));
    super::reset_legacy_job_cancellation(&state);
    assert!(state.cancelled.load(std::sync::atomic::Ordering::Relaxed));
    drop(second);
    assert!(super::progress_job_id(&state).is_none());
    let _third = super::JobLease::acquire(&state, Some("third".into())).unwrap();
    assert!(!state.cancelled.load(std::sync::atomic::Ordering::Relaxed));
}

#[tokio::test]
async fn hardware_startup_failure_retries_cpu_and_produces_probeable_output() {
    let root = super::OperationTempGuard(super::operation_temp_dir("cpu-fallback-test").unwrap());
    let output = root.0.join("fallback.mp4");
    let args = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=s=2x2:r=1:d=1",
        "-c:v",
        "h264_nvenc",
        "-preset",
        "p4",
        "-tune",
        "hq",
        "-rc",
        "vbr",
        "-cq",
        "14",
        "-b:v",
        "0",
        "-pix_fmt",
        "yuv420p",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(std::iter::once(output.to_string_lossy().into_owned()))
    .collect::<Vec<_>>();
    let state = super::JobState::default();
    let started = std::time::Instant::now();
    let failure = super::run_ffmpeg_stage_attempt(
        None,
        &state,
        args.clone(),
        1.0,
        &started,
        0.0,
        99.0,
        "test",
    )
    .await
    .unwrap_err();
    assert!(
        super::auto_encoder::cpu_retry_args(&args, &failure).is_some(),
        "{failure}"
    );
    super::run_ffmpeg_stage(None, &state, args, 1.0, &started, 0.0, 99.0, "test")
        .await
        .unwrap();
    let info = super::probe_media(output.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(info.width, Some(2));
    assert_eq!(info.height, Some(2));
    assert!(std::fs::metadata(output).unwrap().len() > 0);
    assert!(state.pid.lock().unwrap().is_none());
}
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn stack_smartcut_real_media_preserves_frames_audio_and_cancellation() {
    let temp = OperationTempGuard(operation_temp_dir("stack-smartcut-test").unwrap());
    for audio in [false, true] {
        let source = temp.0.join(format!("kaynak ğ & ({audio}).mp4"));
        let mut command = std::process::Command::new("ffmpeg");
        command.args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=160x90:r=30:d=1",
        ]);
        if audio {
            command.args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-c:a",
                "aac",
            ]);
        }
        command
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&source);
        assert!(command.status().unwrap().success());
        let input = source.to_string_lossy().into_owned();
        let info = probe_media(input.clone()).await.unwrap();
        let params = values(&[
            ("source_path", &input),
            (
                "cuts",
                r#"[{"start":0,"end":0.2,"enabled":true},{"start":0.6,"end":0.9,"enabled":true},{"start":0.3,"end":0.5,"enabled":false}]"#,
            ),
            ("resolution", "source"),
        ]);
        let (args, duration) = stack_smartcut_args(&params, &info, &input).unwrap();
        assert!((duration - 0.5).abs() < 0.001);
        assert!(stack_smartcut_args(&params, &info, "different.mp4").is_err());
        let stage = temp.0.join(format!("stage-{audio}.mkv"));
        let args = processing_stack::output_args(args, &stage, true, None).unwrap();
        let state = JobState::default();
        run_ffmpeg_stage(
            None,
            &state,
            args.clone(),
            duration,
            &Instant::now(),
            0.0,
            50.0,
            "stack test",
        )
        .await
        .unwrap();
        let cut_info = probe_media(stage.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert!((cut_info.duration.unwrap() - 0.5).abs() < 0.08);
        assert_eq!(cut_info.width, Some(160));
        assert_eq!(cut_info.audio_codec.is_some(), audio);
        // A source cut crossing removed silence must map onto the shortened
        // intermediate; .1-.7 in the source retains .1-.2 and .6-.7.
        let (mapped, _) =
            processing_stack::source_window(&[(0.0, 0.2), (0.6, 0.9)], 0.1, 0.7).unwrap();
        let mapped: Vec<_> = mapped
            .into_iter()
            .map(|(start, end)| KeepInterval {
                start,
                end,
                enabled: true,
            })
            .collect();
        let mapped_params = values(&[
            ("source_path", &stage.to_string_lossy()),
            ("cuts", &serde_json::to_string(&mapped).unwrap()),
            ("resolution", "source"),
        ]);
        let (mapped_args, mapped_duration) =
            stack_smartcut_args(&mapped_params, &cut_info, &stage.to_string_lossy()).unwrap();
        assert!((mapped_duration - 0.2).abs() < 0.001);
        let mapped_path = temp.0.join(format!("mapped-cut-{audio}.mkv"));
        let mapped_args =
            processing_stack::output_args(mapped_args, &mapped_path, true, None).unwrap();
        run_ffmpeg_stage(
            None,
            &state,
            mapped_args,
            mapped_duration,
            &Instant::now(),
            0.0,
            50.0,
            "mapped cut test",
        )
        .await
        .unwrap();
        let mapped_info = probe_media(mapped_path.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert!((mapped_info.duration.unwrap() - 0.2).abs() < 0.08);
        assert_eq!(mapped_info.audio_codec.is_some(), audio);
        let clipper = OperationRequest {
            input: stage.to_string_lossy().into_owned(),
            operation: "clipper".into(),
            params: values(&[
                ("crop_mode", "9:16"),
                ("vertical_layout", "split"),
                ("rotate", "0"),
                ("flip_h", "false"),
                ("flip_v", "false"),
                ("size_mode", "exact"),
                ("output_width", "90"),
                ("output_height", "160"),
                ("region_a_x", "72"),
                ("region_a_y", "2"),
                ("region_a_w", "26"),
                ("region_a_h", "30"),
                ("region_b_x", "0"),
                ("region_b_y", "0"),
                ("region_b_w", "100"),
                ("region_b_h", "100"),
                ("region_order", "a_first"),
                ("region_a_height", "30"),
            ]),
        };
        let (args, _) = build_command(&clipper, &cut_info).await.unwrap();
        let clipper_path = temp.0.join(format!("clipper-{audio}.mkv"));
        let args = processing_stack::output_args(args, &clipper_path, true, None).unwrap();
        run_ffmpeg_stage(
            None,
            &state,
            args,
            duration,
            &Instant::now(),
            40.0,
            30.0,
            "clipper test",
        )
        .await
        .unwrap();
        let clipper_info = probe_media(clipper_path.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(
            (clipper_info.width, clipper_info.height),
            (Some(90), Some(160))
        );
        // Exercise the actual fused graph with/without audio, including
        // Clipper's internally labelled split and blur filters.
        let (smart_args, _) = stack_smartcut_args(&params, &info, &input).unwrap();
        let graph = smart_args
            .windows(2)
            .find(|pair| pair[0] == "-filter_complex")
            .unwrap()[1]
            .clone();
        for layout in ["split", "blur", "fill", "original"] {
            let mut fused_request = OperationRequest {
                input: input.clone(),
                operation: clipper.operation.clone(),
                params: clipper.params.clone(),
            };
            fused_request.input = input.clone();
            fused_request
                .params
                .insert("vertical_layout".into(), layout.into());
            for (key, value) in [
                ("crop_x", "0"),
                ("crop_y", "0"),
                ("crop_w", "100"),
                ("crop_h", "100"),
                ("background_blur", "20"),
            ] {
                fused_request.params.insert(key.into(), value.into());
            }
            let (clip_args, _) = build_command(&fused_request, &info).await.unwrap();
            let fused = processing_stack::fuse_smartcut_clipper(clip_args, &graph, audio).unwrap();
            let destination = temp.0.join(format!("fused-{layout}-{audio}.mp4"));
            let fused =
                processing_stack::output_args(fused, &destination, false, Some("high")).unwrap();
            run_ffmpeg_stage(
                None,
                &state,
                fused,
                duration,
                &Instant::now(),
                0.0,
                99.0,
                "fused test",
            )
            .await
            .unwrap();
            let result = probe_media(destination.to_string_lossy().into_owned())
                .await
                .unwrap();
            assert_eq!((result.width, result.height), (Some(90), Some(160)));
            assert_eq!(result.audio_codec.is_some(), audio);
            assert_eq!(result.fps, info.fps);
            assert!((result.duration.unwrap() - duration).abs() < 0.08);
        }
        let request = OperationRequest {
            input: clipper_path.to_string_lossy().into_owned(),
            operation: "color".into(),
            params: values(&[
                ("brightness", "5"),
                ("brightness_enabled", "true"),
                ("denoise", "off"),
            ]),
        };
        let (args, _) = build_command(&request, &clipper_info).await.unwrap();
        let final_path = temp.0.join(format!("final-{audio}.mp4"));
        let args = processing_stack::output_args(args, &final_path, false, Some("high")).unwrap();
        run_ffmpeg_stage(
            None,
            &state,
            args,
            duration,
            &Instant::now(),
            50.0,
            49.0,
            "stack test",
        )
        .await
        .unwrap();
        let final_info = probe_media(final_path.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert!((final_info.duration.unwrap() - 0.5).abs() < 0.08);
        assert_eq!(final_info.codec, "h264");
        assert_eq!(final_info.audio_codec.is_some(), audio);
        state.cancelled.store(true, Ordering::Relaxed);
        assert!(run_ffmpeg_stage(
            None,
            &state,
            processing_stack::output_args(
                args_for_cancel(&input),
                &temp.0.join("cancel.mkv"),
                true,
                None
            )
            .unwrap(),
            1.0,
            &Instant::now(),
            0.0,
            99.0,
            "cancel test"
        )
        .await
        .is_err());
        assert!(state.pid.lock().unwrap().is_none());
    }
}

fn args_for_cancel(input: &str) -> Vec<String> {
    vec!["-y".into(), "-i".into(), input.into(), "output.mkv".into()]
}

#[tokio::test]
#[ignore = "set CONTAINER_STACK_BENCH_MEDIA for a real-media performance comparison"]
async fn fused_stack_real_media_benchmark() {
    let source = std::env::var("CONTAINER_STACK_BENCH_MEDIA").unwrap();
    let temp = OperationTempGuard(operation_temp_dir("stack-pair-benchmark").unwrap());
    let fixture = temp.0.join("source.mp4");
    assert!(std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i", &source, "-t", "6", "-c", "copy"])
        .arg(&fixture)
        .status()
        .unwrap()
        .success());
    let input = fixture.to_string_lossy().into_owned();
    let info = probe_media(input.clone()).await.unwrap();
    let params = values(&[
        ("source_path", &input),
        (
            "cuts",
            r#"[{"start":0,"end":1.5,"enabled":true},{"start":2.5,"end":4,"enabled":true}]"#,
        ),
        ("resolution", "source"),
    ]);
    let (cut_args, duration) = stack_smartcut_args(&params, &info, &input).unwrap();
    let state = JobState::default();
    let started = Instant::now();
    let intermediate = temp.0.join("intermediate.mkv");
    run_ffmpeg_stage(
        None,
        &state,
        processing_stack::output_args(cut_args.clone(), &intermediate, true, None).unwrap(),
        duration,
        &started,
        0.0,
        50.0,
        "baseline cut",
    )
    .await
    .unwrap();
    let intermediate_info = probe_media(intermediate.to_string_lossy().into_owned())
        .await
        .unwrap();
    let clipper_params = values(&[
        ("crop_mode", "9:16"),
        ("vertical_layout", "fill"),
        ("crop_x", "34.1797"),
        ("crop_y", "0"),
        ("crop_w", "31.6406"),
        ("crop_h", "100"),
        ("rotate", "0"),
        ("flip_h", "false"),
        ("flip_v", "false"),
        ("size_mode", "exact"),
        ("output_width", "1080"),
        ("output_height", "1920"),
        ("clipper_zoom", "100"),
        ("clipper_x", "50"),
        ("clipper_y", "50"),
    ]);
    let clipper = OperationRequest {
        input: intermediate.to_string_lossy().into_owned(),
        operation: "clipper".into(),
        params: clipper_params.clone(),
    };
    let (args, _) = build_command(&clipper, &intermediate_info).await.unwrap();
    let baseline = temp.0.join("baseline.mp4");
    run_ffmpeg_stage(
        None,
        &state,
        processing_stack::output_args(args, &baseline, false, Some("high")).unwrap(),
        duration,
        &started,
        50.0,
        49.0,
        "baseline clipper",
    )
    .await
    .unwrap();
    let baseline_elapsed = started.elapsed();
    let clipper = OperationRequest {
        input: input.clone(),
        operation: "clipper".into(),
        params: clipper_params,
    };
    let (args, _) = build_command(&clipper, &info).await.unwrap();
    let graph = &cut_args[cut_args
        .iter()
        .position(|arg| arg == "-filter_complex")
        .unwrap()
        + 1];
    let args =
        processing_stack::fuse_smartcut_clipper(args, graph, info.audio_codec.is_some()).unwrap();
    let fused = temp.0.join("fused.mp4");
    let started = Instant::now();
    run_ffmpeg_stage(
        None,
        &state,
        processing_stack::output_args(args, &fused, false, Some("high")).unwrap(),
        duration,
        &started,
        0.0,
        99.0,
        "fused clipper",
    )
    .await
    .unwrap();
    let fused_elapsed = started.elapsed();
    let a = probe_media(baseline.to_string_lossy().into_owned())
        .await
        .unwrap();
    let b = probe_media(fused.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(
        (a.width, a.height, a.codec, a.fps),
        (b.width, b.height, b.codec, b.fps)
    );
    assert!((a.duration.unwrap() - b.duration.unwrap()).abs() < 0.06);
    let comparison = std::process::Command::new("ffmpeg")
        .args(["-v", "info", "-i"])
        .arg(&baseline)
        .arg("-i")
        .arg(&fused)
        .args(["-lavfi", "[0:v][1:v]ssim", "-an", "-f", "null", "-"])
        .output()
        .unwrap();
    assert!(comparison.status.success());
    let report = String::from_utf8_lossy(&comparison.stderr);
    let ssim = report
        .lines()
        .rev()
        .find(|line| line.contains("SSIM"))
        .unwrap();
    let score: f64 = ssim
        .split("All:")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(score > 0.99, "{ssim}");
    eprintln!(
        "STACK BENCH baseline={baseline_elapsed:?} fused={fused_elapsed:?} speedup={:.2}x {ssim}",
        baseline_elapsed.as_secs_f64() / fused_elapsed.as_secs_f64()
    );
}

fn storage_test_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "container-storage-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn runtime_storage_does_not_overlap_the_installer_directory() {
    let data = app_data_directory().unwrap();
    let cache = app_cache_directory().unwrap();
    assert!(data.ends_with(Path::new(app_storage_namespace()).join("data")));
    assert!(cache.ends_with(Path::new(app_storage_namespace()).join("cache")));
    assert!(!data.starts_with(dirs::data_local_dir().unwrap().join("CONTAINER")));
    assert!(!cache.starts_with(dirs::cache_dir().unwrap().join("CONTAINER")));
    assert!(
        data.starts_with(std::env::temp_dir()),
        "tests must not write real runtime data"
    );
    assert!(
        cache.starts_with(std::env::temp_dir()),
        "tests must not write real runtime cache"
    );
}

#[test]
fn history_relocation_preserves_bytes_and_never_overwrites_existing_history() {
    let root = storage_test_root("history");
    let old = root.join("old.json");
    let new = root.join("dev.dean.container/data/download-history.json");
    let bytes = br#"[ { "path": "C:/Downloads/example.mp4", "name": "example.mp4" } ]"#;
    std::fs::write(&old, bytes).unwrap();
    assert_eq!(migrate_download_history(&old, &new).unwrap(), new);
    assert_eq!(std::fs::read(&new).unwrap(), bytes);
    assert!(!old.exists());
    assert_eq!(read_download_history(&new).unwrap().len(), 1);
    migrate_download_history(&old, &new).unwrap();
    // Downgrading/reinstalling an older release may recreate its history.
    // Keep that file, but do not replace the current history with stale data.
    std::fs::write(&old, b"[]").unwrap();
    migrate_download_history(&old, &new).unwrap();
    assert_eq!(std::fs::read(&new).unwrap(), bytes);
    assert!(old.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_or_invalid_history_relocation_keeps_the_original() {
    let root = storage_test_root("invalid");
    let old = root.join("old.json");
    let new = root.join("new/history.json");
    std::fs::write(&old, b"broken json").unwrap();
    assert!(migrate_download_history(&old, &new).is_err());
    assert!(!new.exists());
    assert_eq!(std::fs::read(&old).unwrap(), b"broken json");
    std::fs::write(&old, b"[]").unwrap();
    let blocked = root.join("file-not-directory");
    std::fs::write(&blocked, b"keep").unwrap();
    assert!(migrate_download_history(&old, &blocked.join("history.json")).is_err());
    assert_eq!(std::fs::read(&old).unwrap(), b"[]");
    assert_eq!(std::fs::read(&blocked).unwrap(), b"keep");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn session_relocation_preserves_crash_detection_and_clean_exit() {
    let root = storage_test_root("session");
    let old = root.join("CONTAINER");
    let new = root.join("dev.dean.container/data");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("running.lock"), b"123").unwrap();
    let session = prepare_session_lifecycle_at(&new, &old);
    assert!(session.previous_interrupted);
    assert!(session.marker_path.is_file());
    assert!(!old.join("running.lock").exists());
    assert!(prepare_session_lifecycle_at(&new, &old).previous_interrupted);
    std::fs::remove_file(&session.marker_path).unwrap();
    assert!(!prepare_session_lifecycle_at(&new, &old).previous_interrupted);
    std::fs::write(old.join("running.lock"), b"keep").unwrap();
    let blocked = root.join("blocked");
    std::fs::write(&blocked, b"keep").unwrap();
    assert!(prepare_session_lifecycle_at(&blocked, &old).previous_interrupted);
    assert!(old.join("running.lock").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn embedded_cache_cleanup_keeps_unknown_files_and_assets_regenerate_elsewhere() {
    let root = storage_test_root("assets");
    let old = root.join("CONTAINER");
    let cache = root.join("dev.dean.container/cache/social-tags");
    for (platform, style) in [("kick", "plain"), ("kick", "boxed"), ("twitch", "plain")] {
        social_tag_mark_path_in(&old.join("social-tags"), platform, style).unwrap();
    }
    let model = include_bytes!("../resources/silero_vad.onnx");
    std::fs::create_dir_all(old.join("models")).unwrap();
    let model_path = old
        .join("models")
        .join(format!("silero-{:x}.onnx", Sha256::digest(model)));
    std::fs::write(&model_path, model).unwrap();
    let keep = old.join("social-tags/user-keep.png");
    std::fs::write(&keep, b"user image").unwrap();
    retire_legacy_embedded_caches(&old, false);
    assert_eq!(
        std::fs::read_dir(old.join("social-tags")).unwrap().count(),
        1
    );
    assert!(model_path.exists());
    retire_legacy_embedded_caches(&old, true);
    assert!(!model_path.exists());
    assert!(!old.join("models").exists());
    assert_eq!(std::fs::read(&keep).unwrap(), b"user image");
    for (platform, style) in [("kick", "plain"), ("kick", "boxed"), ("twitch", "plain")] {
        let path = social_tag_mark_path_in(&cache, platform, style).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
        std::fs::write(&path, b"corrupt cache").unwrap();
        social_tag_mark_path_in(&cache, platform, style).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    assert_eq!(
        std::fs::read_dir(old.join("social-tags")).unwrap().count(),
        1
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn kick_banner_and_font_regenerate_from_embedded_bytes() {
    let root = storage_test_root("kick-banner-assets");
    let cache = root.join("social-tags");
    for (bytes, extension) in [
        (
            include_bytes!("../resources/social-tags/kick-banner.png").as_slice(),
            "png",
        ),
        (
            include_bytes!("../resources/social-tags/gotham-xnarrow-black.otf").as_slice(),
            "otf",
        ),
    ] {
        let path = social_tag_asset_path_in(&cache, bytes, extension).unwrap();
        assert!(path.starts_with(&cache));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, b"corrupt cache").unwrap();
        social_tag_asset_path_in(&cache, bytes, extension).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cut_end_accepts_only_the_display_rounding_gap() {
    let duration = 100.05988;
    assert_eq!(check_cut_end(100.060, duration).unwrap(), duration);
    assert_eq!(check_cut_end(duration, duration).unwrap(), duration);
    assert!(check_cut_end(100.061, duration).is_err());
    assert!(check_cut_end(-0.001, duration).is_err());
}

#[test]
fn development_executable_isolated_from_the_release_identity() {
    assert!(is_development_executable(Path::new(
        r"C:\build\container-studio-dev.exe"
    )));
    assert!(is_development_executable(Path::new(
        r"C:\build\CONTAINER-STUDIO-DEV.EXE"
    )));
    assert!(!is_development_executable(Path::new(
        r"C:\Users\User\AppData\Local\CONTAINER\container-studio.exe"
    )));
}

#[test]
fn second_launch_file_path_resolves_relative_and_ignores_nonfiles() {
    let root = std::env::temp_dir().join(format!(
        "container-second-launch-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let project = root.join("örnek.cproj");
    std::fs::write(&project, b"{}").unwrap();
    assert_eq!(
        launch_file_path(["CONTAINER.exe", "missing.mp4", "örnek.cproj"], &root),
        Some(project.clone())
    );
    assert_eq!(launch_file_path(["CONTAINER.exe"], &root), None);
    std::fs::remove_file(project).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn project_files_round_trip_and_reject_invalid_input() {
    let root = std::env::temp_dir().join(format!(
        "container-project-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let requested = root.join("unicode proje");
    let contents = r#"{"version":1,"mediaPath":"C:\\medya\\çalışma.mp4"}"#;
    write_project(
        requested.to_string_lossy().into_owned(),
        contents.to_string(),
    )
    .unwrap();
    let saved = requested.with_extension("cproj");
    let read = read_project_contents(saved.to_string_lossy().into_owned()).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&read).unwrap(),
        serde_json::from_str::<Value>(contents).unwrap()
    );
    assert!(write_project(
        root.join("invalid").to_string_lossy().into_owned(),
        "not json".into()
    )
    .is_err());
    assert!(read_project_contents(root.join("wrong.json").to_string_lossy().into_owned()).is_err());
    let legacy = root.join("older.containerproject");
    std::fs::write(&legacy, contents).unwrap();
    assert!(read_project_contents(legacy.to_string_lossy().into_owned()).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_save_does_not_modify_an_existing_hardlinked_source() {
    let root = storage_test_root("project-hardlink");
    let source = root.join("source.mp4");
    let project = root.join("çalışma (1).cproj");
    let original = b"original source bytes must never change";
    std::fs::write(&source, original).unwrap();
    std::fs::hard_link(&source, &project).unwrap();
    write_project(
        project.to_string_lossy().into_owned(),
        r#"{"version":1,"mediaPath":"source.mp4"}"#.into(),
    )
    .unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert!(read_project_contents(project.to_string_lossy().into_owned()).is_ok());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_replacement_failure_preserves_saved_bytes_and_unknown_files() {
    let root = storage_test_root("project-replace-failure");
    let project = root.join("proje.cproj");
    let unknown = root.join("unknown-user-file.tmp");
    std::fs::write(&unknown, b"keep me").unwrap();
    let original = r#"{"version":1,"mediaPath":"original.mp4"}"#;
    write_project(project.to_string_lossy().into_owned(), original.into()).unwrap();
    let saved = std::fs::read(&project).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&project)
            .unwrap();
        assert!(write_project(
            project.to_string_lossy().into_owned(),
            r#"{"version":1,"mediaPath":"replacement.mp4"}"#.into()
        )
        .is_err());
        drop(locked);
        assert_eq!(std::fs::read(&project).unwrap(), saved);
    }
    assert!(write_project(
        project.to_string_lossy().into_owned(),
        "invalid JSON".into()
    )
    .is_err());
    assert_eq!(std::fs::read(&project).unwrap(), saved);
    let blocked = root.join("blocked.cproj");
    std::fs::create_dir(&blocked).unwrap();
    assert!(write_project(blocked.to_string_lossy().into_owned(), original.into()).is_err());
    assert!(blocked.is_dir());
    assert_eq!(std::fs::read(&unknown).unwrap(), b"keep me");
    assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".container-project-")));
    write_project(
        project.to_string_lossy().into_owned(),
        r#"{"version":1,"mediaPath":"replacement.mp4"}"#.into(),
    )
    .unwrap();
    assert!(
        read_project_contents(project.to_string_lossy().into_owned())
            .unwrap()
            .contains("replacement.mp4")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn download_history_writes_replace_safely_and_clean_failed_pending_files() {
    let root = std::env::temp_dir().join(format!(
        "container-history-write-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("history.json");
    let entry = |name: &str| DownloadHistoryEntry {
        path: root.join(name).to_string_lossy().into_owned(),
        name: name.into(),
    };
    write_download_history(&path, &[entry("first.mp4")]).unwrap();
    write_download_history(&path, &[entry("second.mp4")]).unwrap();
    assert_eq!(read_download_history(&path).unwrap()[0].name, "second.mp4");
    let saved = std::fs::read(&path).unwrap();
    let oversized = DownloadHistoryEntry {
        path: "example.mp4".into(),
        name: "x".repeat(16 * 1024 * 1024),
    };
    assert!(write_download_history(&path, &[oversized]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved);

    let blocked = root.join("blocked.json");
    std::fs::create_dir(&blocked).unwrap();
    assert!(write_download_history(&blocked, &[entry("third.mp4")]).is_err());
    assert!(blocked.is_dir());
    assert!(std::fs::read_dir(&root).unwrap().all(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_none_or(|ext| ext != "tmp")
    }));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn download_history_keeps_only_existing_regular_files_in_its_folder() {
    let base =
        std::env::temp_dir().join(format!("container-download-history-{}", std::process::id()));
    let root = base.join("CONTAINER Downloads");
    std::fs::create_dir_all(&root).unwrap();
    let present = root.join("present.mp4");
    let outside = base.join("outside.mp4");
    std::fs::write(&present, b"video").unwrap();
    std::fs::write(&outside, b"other").unwrap();
    let make = |path: &Path| DownloadHistoryEntry {
        path: path.to_string_lossy().into_owned(),
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
    };
    let results = existing_download_history(
        &root,
        vec![
            make(&present),
            make(&root.join("deleted.mp4")),
            make(&outside),
            make(&present),
        ],
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, present.to_string_lossy());
    let discovered = initial_download_history(&root).unwrap();
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].name, "present.mp4");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn download_history_delete_recycles_only_a_recorded_file() {
    let base = std::env::temp_dir().join(format!(
        "container-download-delete-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let root = base.join("CONTAINER Downloads");
    std::fs::create_dir_all(&root).unwrap();
    let recorded = root.join("recorded.mp4");
    let unrecorded = root.join("unrecorded.mp4");
    std::fs::write(&recorded, b"video").unwrap();
    std::fs::write(&unrecorded, b"keep").unwrap();
    let history_path = base.join("download-history.json");
    write_download_history(
        &history_path,
        &[DownloadHistoryEntry {
            path: recorded.to_string_lossy().into_owned(),
            name: "recorded.mp4".into(),
        }],
    )
    .unwrap();
    assert!(delete_download_history_file(&root, &history_path, &unrecorded).is_err());
    assert!(unrecorded.exists());
    delete_download_history_file(&root, &history_path, &recorded).unwrap();
    assert!(!recorded.exists());
    assert!(read_download_history(&history_path).unwrap().is_empty());
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn download_history_does_not_drop_older_files_after_many_downloads() {
    let base = std::env::temp_dir().join(format!("container-download-many-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let mut entries = Vec::new();
    for index in 0..550 {
        let path = base.join(format!("download-{index:03}.mp4"));
        std::fs::write(&path, b"test").unwrap();
        entries.push(DownloadHistoryEntry {
            path: path.to_string_lossy().into_owned(),
            name: format!("download-{index:03}.mp4"),
        });
    }
    assert_eq!(existing_download_history(&base, entries).len(), 550);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn relocated_project_resolves_source_and_sidecar_relative_to_project() {
    let root = std::env::temp_dir().join(format!(
        "container-portable-project-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let original = root.join("original");
    let moved = root.join("moved");
    std::fs::create_dir_all(original.join("media")).unwrap();
    let source = original.join("media").join("clip.mp4");
    let overlay = original.join("media").join("logo.png");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&overlay, b"logo").unwrap();
    let project = original.join("edit.cproj");
    let contents = serde_json::json!({
        "version":1,"mediaPath":source.to_string_lossy(),
        "toolbox":{"selected":{"fields":[{"key":"image_path","value":overlay.to_string_lossy()}]}},
        "resources":[
            {"label":"Source media","path":source.to_string_lossy()},
            {"label":"Overlay image","path":overlay.to_string_lossy()}
        ]
    });
    write_project(project.to_string_lossy().into_owned(), contents.to_string()).unwrap();
    let stored: Value = serde_json::from_str(
        &read_project_contents(project.to_string_lossy().into_owned()).unwrap(),
    )
    .unwrap();
    assert_eq!(stored["resources"][0]["relativePath"], "media\\clip.mp4");
    std::fs::rename(&original, &moved).unwrap();
    let moved_project = moved.join("edit.cproj");
    let mut restored: Value = serde_json::from_str(
        &read_project_contents(moved_project.to_string_lossy().into_owned()).unwrap(),
    )
    .unwrap();
    resolve_project_resources(&moved_project, &mut restored);
    assert_eq!(
        restored["mediaPath"],
        dunce::canonicalize(moved.join("media").join("clip.mp4"))
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        restored["toolbox"]["selected"]["fields"][0]["value"],
        dunce::canonicalize(moved.join("media").join("logo.png"))
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_outside_media_folder_restores_preview_safe_paths_for_every_stage() {
    let root = std::env::temp_dir().join(format!(
        "container-project-sibling-media-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let desktop = root.join("Desktop");
    let downloads = root.join("Downloads");
    std::fs::create_dir_all(&desktop).unwrap();
    std::fs::create_dir_all(&downloads).unwrap();
    let source = downloads.join("kaynak çığ #1.mp4");
    let output = downloads.join("clipper.mp4");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&output, b"output").unwrap();
    let project = desktop.join("edit.cproj");
    let source_relative = PathBuf::from("..")
        .join("Downloads")
        .join("kaynak çığ #1.mp4");
    let output_relative = PathBuf::from("..").join("Downloads").join("clipper.mp4");
    let mut saved = serde_json::json!({
        "mediaPath": output.to_string_lossy(),
        "stageHistory": {"entries": [
            {"session": {"mediaPath": source.to_string_lossy()}},
            {"session": {"mediaPath": output.to_string_lossy()}}
        ]},
        "resources": [
            {"path": source.to_string_lossy(), "relativePath": source_relative.to_string_lossy()},
            {"path": output.to_string_lossy(), "relativePath": output_relative.to_string_lossy()}
        ]
    });
    resolve_project_resources(&project, &mut saved);
    for (path, expected) in [
        (
            &saved["stageHistory"]["entries"][0]["session"]["mediaPath"],
            &source,
        ),
        (&saved["mediaPath"], &output),
        (
            &saved["stageHistory"]["entries"][1]["session"]["mediaPath"],
            &output,
        ),
    ] {
        let restored = PathBuf::from(path.as_str().unwrap());
        assert_eq!(restored, dunce::canonicalize(expected).unwrap());
        assert!(!restored
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir)));
    }
    // Exercise the real serializer too: repeatedly saving and reopening
    // must not reintroduce traversal into any historical media path.
    let expected = saved.clone();
    for _ in 0..25 {
        write_project(project.to_string_lossy().into_owned(), saved.to_string()).unwrap();
        saved = serde_json::from_str(
            &read_project_contents(project.to_string_lossy().into_owned()).unwrap(),
        )
        .unwrap();
        resolve_project_resources(&project, &mut saved);
        assert_eq!(saved, expected);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn missing_project_media_can_be_detected_before_restore() {
    let path = std::env::temp_dir().join(format!(
        "container-project-media-{}-{}.mp4",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(!project_media_available(path.to_string_lossy().into_owned()).await);
    std::fs::write(&path, b"test media placeholder").unwrap();
    assert!(project_media_available(path.to_string_lossy().into_owned()).await);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn still_image_containers_are_not_misclassified_as_video() {
    for extension in ["heic", "heif", "avif", "png"] {
        assert_eq!(
            classify_media_kind(extension, true, false).unwrap(),
            "image"
        );
    }
    assert_eq!(classify_media_kind("mp4", true, true).unwrap(), "video");
    assert_eq!(classify_media_kind("wav", false, true).unwrap(), "audio");
    assert!(classify_media_kind("unknown", false, false).is_err());
}

#[cfg(target_os = "windows")]
#[test]
fn installed_truetype_font_can_be_embedded_for_preview() {
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let font = PathBuf::from(windows).join("Fonts").join("arial.ttf");
    let data = font_preview_data(font.to_string_lossy().into_owned()).unwrap();
    assert!(data.starts_with("data:font/ttf;base64,"));
    assert!(data.len() > 1_000);
}

#[cfg(target_os = "windows")]
#[test]
fn font_collections_are_not_offered_as_single_preview_faces() {
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let collection = PathBuf::from(windows).join("Fonts").join("msjhl.ttc");
    if collection.is_file() {
        assert!(font_preview_data(collection.to_string_lossy().into_owned()).is_err());
    }
}

#[test]
fn social_font_list_is_small_ordered_and_excludes_unrelated_faces() {
    let available = vec![
        FontOption {
            name: "Comic Sans MS".into(),
            path: "comic.ttf".into(),
        },
        FontOption {
            name: "Poppins Bold".into(),
            path: "poppins-bold.ttf".into(),
        },
        FontOption {
            name: "Impact".into(),
            path: "impact.ttf".into(),
        },
        FontOption {
            name: "Montserrat ExtraBold".into(),
            path: "montserrat-extrabold.ttf".into(),
        },
    ];
    let curated = curate_social_fonts(available);
    let names = curated
        .iter()
        .map(|font| font.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Impact", "Montserrat ExtraBold", "Poppins Bold"]);
}

#[tokio::test]
async fn bundled_montserrat_is_listed_and_can_be_previewed() {
    let font = bundled_text_font().unwrap();
    assert_eq!(font.name, "Montserrat ExtraBold Italic (800)");
    let bytes = std::fs::read(&font.path).unwrap();
    assert_eq!(
        bytes,
        include_bytes!("../resources/fonts/Montserrat-ExtraBoldItalic.ttf")
    );
    assert_eq!(
        font_preview_data(font.path.clone()).unwrap(),
        format!("data:font/ttf;base64,{}", BASE64.encode(&bytes))
    );
    let listed = list_system_fonts().await.unwrap();
    assert_eq!(
        listed.iter().filter(|item| item.name == font.name).count(),
        1
    );
    assert!(listed
        .iter()
        .any(|item| item.name == font.name && item.path == font.path));
}

#[test]
fn heif_tile_grid_reports_the_composed_display_dimensions() {
    let data = serde_json::json!({
        "stream_groups": [{
            "type": "Tile Grid",
            "disposition": { "default": 1 },
            "components": [{
                "width": 4032,
                "height": 3024,
                "side_data_list": [{ "rotation": -90 }]
            }]
        }]
    });
    assert_eq!(
        primary_tile_grid_display_dimensions(&data),
        Some((3024, 4032))
    );
}

// The browser audit supplies captured run_operation requests and compares
// its visible composition against frames from the actual export chain.
#[tokio::test]
#[ignore = "run via the Clipper real-media Playwright audit"]
async fn clipper_preview_export_audit() {
    let manifest = PathBuf::from(std::env::var("CONTAINER_CLIPPER_AUDIT_MANIFEST").unwrap());
    let jobs: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    for job in jobs.as_array().unwrap() {
        let request: OperationRequest = serde_json::from_value(job["request"].clone()).unwrap();
        let info = probe_media(request.input.clone()).await.unwrap();
        let (mut command, _) = build_command(&request, &info).await.unwrap();
        let output = manifest
            .parent()
            .unwrap()
            .join(format!("{}-export.png", job["name"].as_str().unwrap()));
        let encoded = output.with_extension("mp4");
        command.pop();
        command.extend([
            "-frames:v".into(),
            "1".into(),
            "-an".into(),
            encoded.to_string_lossy().into_owned(),
        ]);
        let result = hidden_command("ffmpeg")
            .args(&command)
            .output()
            .await
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = hidden_command("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-i",
                encoded.to_str().unwrap(),
                "-frames:v",
                "1",
                "-update",
                "1",
            ])
            .arg(&output)
            .output()
            .await
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[tokio::test]
async fn vertical_transform_layouts_preserve_aspect_ratio() {
    let info = MediaInfo {
        path: "source.mp4".into(),
        name: "source.mp4".into(),
        kind: "video".into(),
        duration: Some(5.0),
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        codec: "h264".into(),
        audio_codec: Some("aac".into()),
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: Some(8),
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    let request = |layout: &str| OperationRequest {
        input: info.path.clone(),
        operation: "clipper".into(),
        params: values(&[
            ("crop_mode", "9:16"),
            ("vertical_layout", layout),
            ("fit_mode", "crop"),
            ("canvas_background", "black"),
            ("canvas_color", "#202020"),
            ("crop_x", "34.1797"),
            ("crop_y", "0"),
            ("crop_w", "31.6406"),
            ("crop_h", "100"),
            ("rotate", "0"),
            ("flip_h", "false"),
            ("flip_v", "false"),
            ("size_mode", "exact"),
            ("size", "1080"),
            ("output_width", "1080"),
            ("output_height", "1920"),
            ("region_a_x", "72"),
            ("region_a_y", "2"),
            ("region_a_w", "26"),
            ("region_a_h", "30"),
            ("region_b_x", "0"),
            ("region_b_y", "0"),
            ("region_b_w", "100"),
            ("region_b_h", "100"),
            ("region_order", "a_first"),
            ("region_a_height", "30"),
            ("freecam_x", "50"),
            ("freecam_y", "2"),
            ("freecam_size", "77"),
            ("watermark_enabled", "false"),
            ("watermark_text", ""),
            ("watermark_size", "32"),
            ("watermark_opacity", "85"),
            ("watermark_background", "false"),
        ]),
    };
    let (original, _) = build_command(&request("original"), &info).await.unwrap();
    let original_filter = original
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .map(|pair| pair[1].as_str())
        .unwrap();
    assert!(original_filter.contains("scale=trunc(1080*1.0000/2)*2:-2"));
    assert!(original_filter.contains("pad=1080:1920"));
    assert!(!original_filter.contains("crop=trunc"));

    let (blur, _) = build_command(&request("blur"), &info).await.unwrap();
    let blur_filter = blur
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .map(|pair| pair[1].as_str())
        .unwrap();
    assert!(blur_filter.contains("split=2[bg][fg]"));
    assert!(blur_filter.contains("gblur=sigma=35"));
    assert!(blur_filter.contains("scale=trunc(1080*1.0000/2)*2:-2"));
    let mut reframed = request("blur");
    reframed.params.insert("clipper_zoom".into(), "175".into());
    reframed.params.insert("clipper_x".into(), "25".into());
    reframed.params.insert("clipper_y".into(), "80".into());
    reframed.params.insert("blur_strength".into(), "12".into());
    reframed.params.insert("output_width".into(), "360".into());
    reframed.params.insert("output_height".into(), "640".into());
    reframed
        .params
        .insert("social_tag_enabled".into(), "true".into());
    reframed
        .params
        .insert("social_tag_username".into(), "streamername42".into());
    reframed
        .params
        .insert("social_tag_platform".into(), "kick".into());
    reframed
        .params
        .insert("social_tag_style".into(), "kick_banner".into());
    reframed
        .params
        .insert("social_tag_size".into(), "54".into());
    let (reframed_command, _) = build_command(&reframed, &info).await.unwrap();
    let reframed_filter = reframed_command
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .map(|pair| pair[1].as_str())
        .unwrap();
    assert!(reframed_filter.contains("scale=trunc(360*1.7500/2)*2:-2"));
    assert!(reframed_filter.contains(
        "crop=360:min(ih\\,202):trunc((iw-ow)*25.00/100/2)*2:trunc((ih-oh)*80.00/100/2)*2"
    ));
    assert!(reframed_filter.contains("overlay=(W-w)/2:(H-h)/2"));
    assert!(reframed_filter.contains("gblur=sigma=12.00"));
    let rendered = hidden_command("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=640x360:r=30:d=1",
            "-vf",
            reframed_filter,
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        rendered.status.success(),
        "Blur zoom and banner render failed: {}",
        String::from_utf8_lossy(&rendered.stderr)
    );

    let (fill, _) = build_command(&request("fill"), &info).await.unwrap();
    let fill_filter = fill
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .map(|pair| pair[1].as_str())
        .unwrap();
    assert!(fill_filter.contains("crop=trunc"));
    assert!(fill_filter.contains("scale=trunc(1080/2)*2:trunc(1920/2)*2"));

    let (split, _) = build_command(&request("split"), &info).await.unwrap();
    let split_filter = split
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .map(|pair| pair[1].as_str())
        .unwrap();
    assert!(split_filter.contains("split=2[ra][rb]"));
    assert!(split_filter.contains("scale=1080:576"));
    assert!(split_filter.contains("scale=1080:1344"));
    assert!(split_filter.contains("[top][bottom]vstack=inputs=2"));

    // Exercise the whole Clipper export graph, not only the Social Tag
    // sub-filter. This caught Split preview/export disagreements that a
    // unit test of clipper_social_tag_filter could not see.
    for layout in ["split", "squares", "fill"] {
        let mut tops = Vec::new();
        for distance in ["0", "15"] {
            let mut render = request(layout);
            render.params.insert("output_width".into(), "360".into());
            render.params.insert("output_height".into(), "640".into());
            render
                .params
                .insert("social_tag_enabled".into(), "true".into());
            render
                .params
                .insert("social_tag_platform".into(), "kick".into());
            render
                .params
                .insert("social_tag_username".into(), "streamername42".into());
            render
                .params
                .insert("social_tag_style".into(), "kick_banner".into());
            render.params.insert("social_tag_size".into(), "54".into());
            render
                .params
                .insert("social_tag_seam_offset".into(), distance.into());
            let (command, _) = build_command(&render, &info).await.unwrap();
            let filter = command.windows(2).find(|pair| pair[0] == "-vf").unwrap()[1].as_str();
            let output = hidden_command("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=white:s=640x360:r=1:d=1",
                    "-vf",
                    filter,
                    "-frames:v",
                    "1",
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "-",
                ])
                .output()
                .await
                .unwrap();
            assert!(
                output.status.success(),
                "{layout} full export failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout.len(), 360 * 640 * 3);
            let top = (100..500)
                .find(|&y| {
                    output.stdout[(y * 360 + 330) * 3..][..3]
                        .iter()
                        .all(|&v| v < 40)
                })
                .unwrap_or_else(|| {
                    panic!("{layout} {distance}% has no banner bar in full output: {filter}")
                });
            tops.push(top);
        }
        assert!(
            tops[1] > tops[0] + 10,
            "{layout} full export ignored 15% distance: {tops:?}"
        );
    }

    let (squares, _) = build_command(&request("squares"), &info).await.unwrap();
    let squares_filter = squares.windows(2).find(|pair| pair[0] == "-vf").unwrap()[1].as_str();
    assert!(squares_filter.contains("split=2[ra][rb]"));
    assert!(squares_filter.contains("scale=1080:960"));
    assert!(squares_filter.contains("[camera][content]vstack=inputs=2"));
    assert!(squares.windows(2).any(|pair| pair == ["-crf", "18"]));
    assert!(squares
        .windows(2)
        .any(|pair| pair == ["-profile:v", "high"]));
    assert!(squares
        .windows(2)
        .any(|pair| pair == ["-preset", "ultrafast"]));
    assert!(!squares.windows(2).any(|pair| pair == ["-qp", "0"]));
    assert!(squares_filter.contains("fps=30.000000"));

    let (freecam, _) = build_command(&request("freecam"), &info).await.unwrap();
    let freecam_filter = freecam.windows(2).find(|pair| pair[0] == "-vf").unwrap()[1].as_str();
    assert!(freecam_filter.contains("scale=832:"));
    assert!(freecam_filter.contains("alphamerge[camera]"));
    assert!(freecam_filter.contains("overlay=(W-w)*50/100:(H-h)*2/100"));
    let rounded_mask = image::open(rounded_camera_mask(180, 102).unwrap())
        .unwrap()
        .to_luma8();
    assert_eq!(rounded_mask.get_pixel(0, 0).0[0], 0);
    assert_eq!(rounded_mask.get_pixel(90, 51).0[0], 255);
    let mut freecam_preview = request("freecam");
    freecam_preview
        .params
        .insert("output_width".into(), "360".into());
    freecam_preview
        .params
        .insert("output_height".into(), "640".into());
    let (freecam_command, _) = build_command(&freecam_preview, &info).await.unwrap();
    let freecam_preview_filter = freecam_command
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .unwrap()[1]
        .as_str();
    let freecam_render = hidden_command("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=640x360:r=30:d=2",
            "-vf",
            freecam_preview_filter,
            "-frames:v",
            "45",
            "-f",
            "framemd5",
            "-",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        freecam_render.status.success(),
        "Rounded Freecam render failed: {}",
        String::from_utf8_lossy(&freecam_render.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&freecam_render.stdout)
            .lines()
            .filter(|line| !line.starts_with('#'))
            .count(),
        45,
        "Freecam's static rounded mask must remain visible through the video"
    );
    for layout in ["split", "squares", "freecam"] {
        let mut unchanged = request(layout);
        unchanged.params.insert("clipper_zoom".into(), "250".into());
        unchanged.params.insert("clipper_x".into(), "0".into());
        unchanged.params.insert("clipper_y".into(), "100".into());
        let (command, _) = build_command(&unchanged, &info).await.unwrap();
        let filter = command.windows(2).find(|pair| pair[0] == "-vf").unwrap()[1].as_str();
        let baseline = match layout {
            "split" => split_filter,
            "squares" => squares_filter,
            _ => freecam_filter,
        };
        assert_eq!(
            filter, baseline,
            "{layout} must ignore zoom settings from other layouts"
        );
    }
    for layout in ["original", "fill"] {
        let mut zoomed = request(layout);
        zoomed.params.insert("output_width".into(), "360".into());
        zoomed.params.insert("output_height".into(), "640".into());
        zoomed.params.insert("clipper_zoom".into(), "150".into());
        zoomed.params.insert("clipper_x".into(), "25".into());
        zoomed.params.insert("clipper_y".into(), "75".into());
        let (command, _) = build_command(&zoomed, &info).await.unwrap();
        let filter = command.windows(2).find(|pair| pair[0] == "-vf").unwrap()[1].as_str();
        if layout == "fill" {
            assert!(filter.contains("crop=trunc(iw/1.5000/2)*2"));
        } else {
            assert!(filter.contains("scale=trunc(360*1.5000/2)*2:-2"));
        }
        let rendered = hidden_command("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=640x360:r=30:d=1",
                "-vf",
                filter,
                "-frames:v",
                "1",
                "-f",
                "null",
                "-",
            ])
            .output()
            .await
            .unwrap();
        assert!(
            rendered.status.success(),
            "{layout} zoom render failed: {}",
            String::from_utf8_lossy(&rendered.stderr)
        );
    }

    let mut watermarked = request("split");
    watermarked
        .params
        .insert("watermark_enabled".into(), "true".into());
    watermarked
        .params
        .insert("watermark_text".into(), "@channel".into());
    watermarked
        .params
        .insert("watermark_background".into(), "true".into());
    let (watermarked, _) = build_command(&watermarked, &info).await.unwrap();
    let watermark_filter = watermarked
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .unwrap()[1]
        .as_str();
    assert!(watermark_filter.contains("drawtext="));
    assert!(watermark_filter.contains("text='@channel'"));
    assert!(watermark_filter.contains("drawbox=x=0:y=552.000:w=iw:h=48:color=black:t=fill"));
    assert!(watermark_filter.contains("y=576.000-text_h/2"));
    assert!(watermark_filter.contains("fontcolor=white@0.850"));

    let mut plain_watermark = request("squares");
    plain_watermark
        .params
        .insert("watermark_enabled".into(), "true".into());
    plain_watermark
        .params
        .insert("watermark_text".into(), "100% ready".into());
    let (plain_watermark, _) = build_command(&plain_watermark, &info).await.unwrap();
    let plain_filter = plain_watermark
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .unwrap()[1]
        .as_str();
    assert!(!plain_filter.contains("drawbox="));
    assert!(plain_filter.contains(r"text='100%\ ready':expansion=none"));
    assert!(plain_filter.contains("y=960.000-text_h/2"));

    let mut freecam_watermark = request("freecam");
    freecam_watermark
        .params
        .insert("watermark_enabled".into(), "true".into());
    freecam_watermark
        .params
        .insert("watermark_text".into(), "@channel".into());
    freecam_watermark
        .params
        .insert("watermark_background".into(), "true".into());
    let (freecam_watermark, _) = build_command(&freecam_watermark, &info).await.unwrap();
    let freecam_watermark_filter = freecam_watermark
        .windows(2)
        .find(|pair| pair[0] == "-vf")
        .unwrap()[1]
        .as_str();
    assert!(freecam_watermark_filter.contains("drawbox=x=124.000:"));
    assert!(freecam_watermark_filter.contains(":w=832:h=48:color=black:t=fill"));
}

#[tokio::test]
async fn social_tag_renders_both_styles_and_long_names() {
    let info = MediaInfo {
        path: "source.mp4".into(),
        name: "source.mp4".into(),
        kind: "video".into(),
        duration: Some(1.0),
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        codec: "h264".into(),
        audio_codec: None,
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: Some(8),
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    for (platform, style, username, size, position) in [
        ("kick", "plain", "Example", "36", "center"),
        ("kick", "plain", "Example", "36", "left"),
        ("kick", "plain", "Example", "36", "right"),
        ("kick", "plain", "eray", "36", "left"),
        ("kick", "boxed", "eray", "36", "center"),
        ("kick", "boxed", "OHNEPIXEL", "36", "left"),
        ("kick", "boxed", "OHNEPIXEL", "36", "center"),
        ("kick", "boxed", "OHNEPIXEL", "36", "right"),
        ("kick", "boxed", "streamername42", "20", "left"),
        ("kick", "plain", "nejburi", "20", "center"),
        ("kick", "boxed", "nejburi", "20", "center"),
        ("twitch", "plain", "nejburi", "20", "center"),
        ("twitch", "boxed", "nejburi", "20", "center"),
        ("kick", "boxed", "O'Neil", "20", "center"),
        ("twitch", "plain", "name;[tag],100%", "36", "left"),
        (
            "kick",
            "boxed",
            "a_very_long_kick_username_12345",
            "36",
            "left",
        ),
        (
            "kick",
            "boxed",
            "a_very_long_kick_username_12345",
            "36",
            "right",
        ),
        ("twitch", "plain", "ohnePixel", "36", "center"),
        ("twitch", "plain", "ohnePixel", "36", "right"),
        ("twitch", "boxed", "OHNEPIXEL", "36", "left"),
        ("twitch", "boxed", "OHNEPIXEL", "50", "right"),
    ] {
        let params = values(&[
            ("social_tag_enabled", "true"),
            ("social_tag_platform", platform),
            ("social_tag_username", username),
            ("social_tag_style", style),
            (
                if style == "boxed" {
                    "social_tag_boxed_position"
                } else {
                    "social_tag_plain_position"
                },
                position,
            ),
            ("social_tag_size", size),
            ("region_a_height", "30"),
            ("region_order", "a_first"),
        ]);
        let filter = clipper_social_tag_filter(&params, "split", 360, 640, &info)
            .unwrap()
            .unwrap();
        assert!(filter.contains(&format!("text='{}'", drawtext_escape(username))));
        assert_eq!(
            filter.contains("color=black@0.88"),
            style == "boxed" && platform == "kick"
        );
        assert_eq!(filter.contains("borderw=3"), style == "plain");
        if style == "plain" && username == "eray" {
            assert!(filter.contains("y=192-text_h/2+3"), "{filter}");
        }
        if style == "boxed" && username == "eray" {
            assert!(filter.contains("-text_h)/2+3"), "{filter}");
        }
        assert!(filter.contains("flags=lanczos"));
        assert!(filter.contains("format=yuv444:eof_action=repeat:repeatlast=1"));
        if style == "plain" {
            assert!(filter.contains("y=192-text_h/2"));
        } else {
            assert!(filter.starts_with("drawbox=x="));
            let mut segments = filter.split(',');
            let icon = segments.next().unwrap();
            let plate = segments.next().unwrap();
            for key in ["y=", "h="] {
                let value = icon.split(':').find(|part| part.starts_with(key)).unwrap();
                assert!(plate.split(':').any(|part| part == value));
            }
        }
        let snapshot = std::env::var_os("CONTAINER_SOCIAL_TAG_SNAPSHOT_DIR")
            .map(PathBuf::from)
            .map(|directory| {
                directory.join(format!(
                    "social-{platform}-{style}-{position}-{username}-{size}.png"
                ))
            });
        let mut command = hidden_command("ffmpeg");
        command
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("color=c=0x303844:s=360x640:r=1:d=1")
            .args([
                "-vf",
                &format!("drawbox=x=0:y=192:w=360:h=448:color=white:t=fill,{filter}"),
                "-frames:v",
                "1",
            ]);
        if let Some(path) = snapshot.as_ref() {
            command.arg(path);
        } else {
            command.args(["-f", "null", "-"]);
        }
        let output = command.output().await.unwrap();
        assert!(
            output.status.success(),
            "{platform} {style} {position} Social Tag FFmpeg render failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let base = [
        ("social_tag_enabled", "true"),
        ("social_tag_username", "Example"),
        ("social_tag_style", "plain"),
        ("social_tag_size", "56"),
        ("region_a_height", "30"),
        ("region_order", "b_first"),
    ];
    let reversed_split = clipper_social_tag_filter(&values(&base), "split", 360, 640, &info)
        .unwrap()
        .unwrap();
    assert!(reversed_split.contains("y=192-text_h/2"));
    let squares = clipper_social_tag_filter(&values(&base), "squares", 360, 640, &info)
        .unwrap()
        .unwrap();
    assert!(squares.contains("y=320-text_h/2"));
    let freecam = clipper_social_tag_filter(
        &values(&[
            ("social_tag_enabled", "true"),
            ("social_tag_username", "Example"),
            ("social_tag_style", "boxed"),
            ("social_tag_size", "56"),
            ("region_a_x", "0"),
            ("region_a_y", "0"),
            ("region_a_w", "50"),
            ("region_a_h", "50"),
            ("freecam_x", "50"),
            ("freecam_y", "2"),
            ("freecam_size", "50"),
        ]),
        "freecam",
        360,
        640,
        &info,
    )
    .unwrap()
    .unwrap();
    assert!(freecam.starts_with("drawbox=x=92:"));
}

#[tokio::test]
async fn kick_banner_black_matches_artwork_for_full_and_limited_range() {
    let filter = clipper_kick_banner_filter(
        &HashMap::new(),
        "streamername42",
        54.0,
        KickBannerPlacement {
            width: 360,
            height: 640,
            camera_left: 0.0,
            camera_width: 360.0,
            camera_bottom: 192.0,
            seam: 192.0,
            camera_below: false,
            seam_offset: 0.0,
        },
    )
    .unwrap();
    for range in ["pc", "tv"] {
        let source = format!("color=c=0x303844:s=360x640:r=1:d=1,scale=out_range={range},format=yuv444p,setparams=range={range}");
        let output = hidden_command("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &source,
                "-vf",
                &filter,
                "-frames:v",
                "1",
                "-pix_fmt",
                "rgb24",
                "-f",
                "rawvideo",
                "-",
            ])
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 360 * 640 * 3);
        for y in 170..188 {
            for x in 310..350 {
                let pixel = &output.stdout[(y * 360 + x) * 3..(y * 360 + x) * 3 + 3];
                assert!(
                    pixel.iter().all(|v| *v <= 2),
                    "{range}: banner black {pixel:?}"
                );
            }
        }
        let background = &output.stdout[(100 * 360 + 200) * 3..(100 * 360 + 200) * 3 + 3];
        for (actual, expected) in background.iter().zip([48i16, 56, 68]) {
            assert!(
                (i16::from(*actual) - expected).abs() <= 4,
                "{range}: background changed {background:?}"
            );
        }
    }
}

#[tokio::test]
async fn kick_banner_renders_supplied_art_font_and_camera_seam() {
    let info = MediaInfo {
        path: "source.mp4".into(),
        name: "source.mp4".into(),
        kind: "video".into(),
        duration: Some(1.0),
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        codec: "h264".into(),
        audio_codec: None,
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: Some(8),
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    for (layout, order, size) in [
        ("split", "a_first", "20"),
        ("split", "a_first", "36"),
        ("split", "a_first", "45"),
        ("split", "a_first", "54"),
        ("split", "b_first", "36"),
        ("squares", "a_first", "36"),
        ("freecam", "a_first", "36"),
        ("original", "a_first", "36"),
        ("blur", "a_first", "36"),
        ("fill", "a_first", "36"),
    ] {
        let params = values(&[
            ("social_tag_enabled", "true"),
            ("social_tag_platform", "kick"),
            ("social_tag_username", "adinross"),
            ("social_tag_style", "kick_banner"),
            ("social_tag_size", size),
            ("social_tag_text_units", "4.3"),
            ("region_a_height", "30"),
            ("region_order", order),
            ("region_a_x", "0"),
            ("region_a_y", "0"),
            ("region_a_w", "50"),
            ("region_a_h", "50"),
            ("freecam_x", "50"),
            ("freecam_y", "2"),
            ("freecam_size", "50"),
        ]);
        let filter = clipper_social_tag_filter(&params, layout, 360, 640, &info)
            .unwrap()
            .unwrap();
        assert!(filter.contains("crop=1080:101:0:979"));
        assert!(filter.contains("color=black:t=fill"));
        assert!(filter.contains("text='ADINROSS'"));
        assert!(filter.contains("drawtext=fontfile="));
        let snapshot = std::env::var_os("CONTAINER_KICK_BANNER_SNAPSHOT_DIR")
            .map(PathBuf::from)
            .map(|directory| directory.join(format!("kick-banner-{layout}-{order}-{size}.png")));
        let mut command = hidden_command("ffmpeg");
        command
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("color=c=0x303844:s=360x640:r=1:d=1")
            .args(["-vf", &filter, "-frames:v", "1"]);
        if let Some(path) = snapshot.as_ref() {
            std::fs::write(path.with_extension("ffilter"), &filter).unwrap();
            command.arg("-y");
            command.arg(path);
        } else {
            command.args(["-f", "null", "-"]);
        }
        let output = command.output().await.unwrap();
        assert!(
            output.status.success(),
            "{layout} {order} {size} Kick banner FFmpeg render failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut tops = Vec::new();
    let mut font_sizes = Vec::new();
    for distance in ["0", "29"] {
        let params = values(&[
            ("social_tag_enabled", "true"),
            ("social_tag_platform", "kick"),
            ("social_tag_username", "streamername42"),
            ("social_tag_style", "kick_banner"),
            ("social_tag_size", "54"),
            ("social_tag_seam_offset", distance),
            ("region_a_height", "30"),
            ("region_order", "a_first"),
        ]);
        let filter = clipper_social_tag_filter(&params, "split", 360, 640, &info)
            .unwrap()
            .unwrap();
        if let Some(directory) = std::env::var_os("CONTAINER_KICK_BANNER_SNAPSHOT_DIR") {
            let image =
                PathBuf::from(directory).join(format!("kick-banner-long-{distance}.png"));
            let snapshot = hidden_command("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
                .arg("color=c=0x303844:s=360x640:r=1:d=1")
                .args(["-vf", &filter, "-frames:v", "1", "-y"])
                .arg(image)
                .output()
                .await
                .unwrap();
            assert!(snapshot.status.success());
        }
        font_sizes.push(
            filter
                .split("fontsize=")
                .nth(1)
                .unwrap()
                .split(':')
                .next()
                .unwrap()
                .to_string(),
        );
        let output = hidden_command("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x303844:s=360x640:r=1:d=1",
                "-vf",
                &filter,
                "-frames:v",
                "1",
                "-pix_fmt",
                "rgb24",
                "-f",
                "rawvideo",
                "-",
            ])
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "Kick banner {distance}% render failed"
        );
        assert_eq!(output.stdout.len(), 360 * 640 * 3);
        let top = (130..320)
            .find(|&y| {
                let pixel = &output.stdout[(y * 360 + 330) * 3..][..3];
                pixel.iter().all(|&value| value < 12)
            })
            .unwrap();
        let white_rows: Vec<_> = (top..top + 28)
            .filter(|&y| {
                (130..290).any(|x| {
                    output.stdout[(y * 360 + x) * 3..][..3]
                        .iter()
                        .all(|&value| value > 190)
                })
            })
            .collect();
        assert!(
            (((white_rows[0] + white_rows[white_rows.len() - 1]) as f64 / 2.0)
                - (top as f64 + 13.5))
                .abs()
                <= 2.0,
            "Kick lettering must be vertically centered in its strip at {distance}%"
        );
        let last_white_x = (130..340)
            .rev()
            .find(|&x| {
                (top..top + 28).any(|y| {
                    output.stdout[(y * 360 + x) * 3..][..3]
                        .iter()
                        .all(|&value| value > 190)
                })
            })
            .unwrap();
        assert!(
            last_white_x <= 288,
            "Kick username must stay clear of the right-side phone controls: {last_white_x}"
        );
        let below_prefix = &output.stdout[((top + 30) * 360 + 180) * 3..][..3];
        assert!(
            below_prefix.iter().any(|&value| value > 25),
            "Kick prefix must not leave a black tile below the banner at {distance}%"
        );
        tops.push(top);
    }
    assert!(
        tops[1] > tops[0] + 30,
        "29% must visibly move the rendered Kick banner: {tops:?}"
    );
    let reference = clipper_social_tag_filter(
        &values(&[
            ("social_tag_enabled", "true"),
            ("social_tag_platform", "kick"),
            ("social_tag_username", "adinross"),
            ("social_tag_style", "kick_banner"),
            ("social_tag_size", "54"),
            ("region_a_height", "30"),
            ("region_order", "a_first"),
        ]),
        "split",
        360,
        640,
        &info,
    )
    .unwrap()
    .unwrap();
    let reference_font_size = reference
        .split("fontsize=")
        .nth(1)
        .unwrap()
        .split(':')
        .next()
        .unwrap();
    assert!(font_sizes.iter().all(|size| size == reference_font_size));
    for (layout, probe_x) in [
        ("original", 330),
        ("blur", 330),
        ("squares", 330),
        ("freecam", 260),
    ] {
        let mut tops = Vec::new();
        for distance in ["0", "30"] {
            let params = values(&[
                ("social_tag_enabled", "true"),
                ("social_tag_platform", "kick"),
                ("social_tag_username", "streamername42"),
                ("social_tag_style", "kick_banner"),
                ("social_tag_size", "54"),
                ("social_tag_seam_offset", distance),
                ("region_a_x", "0"),
                ("region_a_y", "0"),
                ("region_a_w", "50"),
                ("region_a_h", "50"),
                ("freecam_x", "50"),
                ("freecam_y", "2"),
                ("freecam_size", "50"),
            ]);
            let filter = clipper_social_tag_filter(&params, layout, 360, 640, &info)
                .unwrap()
                .unwrap();
            let output = hidden_command("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
                .arg("color=c=0x303844:s=360x640:r=1:d=1")
                .args([
                    "-vf",
                    &filter,
                    "-frames:v",
                    "1",
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "-",
                ])
                .output()
                .await
                .unwrap();
            assert!(output.status.success());
            let top = (70..620)
                .find(|&y| {
                    output.stdout[(y * 360 + probe_x) * 3..][..3]
                        .iter()
                        .all(|&value| value < 12)
                })
                .unwrap();
            tops.push(top);
        }
        assert!(tops[1] > tops[0] + 30, "{layout} banner offset: {tops:?}");
    }
}

#[tokio::test]
async fn measured_plain_social_tag_is_visually_centered() {
    let info = MediaInfo {
        path: "source.mp4".into(),
        name: "source.mp4".into(),
        kind: "video".into(),
        duration: Some(1.0),
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        codec: "h264".into(),
        audio_codec: None,
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: Some(8),
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    // Chromium's Arial Black canvas measurement for "Example" at 36 px.
    let params = values(&[
        ("social_tag_enabled", "true"),
        ("social_tag_platform", "kick"),
        ("social_tag_username", "Example"),
        ("social_tag_style", "plain"),
        ("social_tag_plain_position", "center"),
        ("social_tag_size", "36"),
        ("social_tag_text_units", "4.72314"),
        ("region_a_height", "30"),
        ("region_order", "a_first"),
    ]);
    let filter = clipper_social_tag_filter(&params, "split", 360, 640, &info)
        .unwrap()
        .unwrap();
    let output = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
        .arg("color=c=0x303844:s=360x640:r=1:d=1")
        .args([
            "-vf",
            &format!("drawbox=x=0:y=192:w=360:h=448:color=white:t=fill,{filter},format=rgb24"),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut left = 360usize;
    let mut right = 0usize;
    for (index, rgb) in output.stdout.as_chunks::<3>().0.iter().enumerate() {
        let x = index % 360;
        let y = index / 360;
        if !(170..=215).contains(&y) {
            continue;
        }
        if rgb[0] < 130 && rgb[1] > 180 && rgb[2] < 130 {
            left = left.min(x);
        }
        if rgb.iter().all(|channel| *channel < 35) {
            right = right.max(x);
        }
    }
    assert!(
        left < right && right < 360,
        "missing tag pixels: {left}..{right}"
    );
    let visual_center = (left + right) as f64 / 2.0;
    assert!(
        (visual_center - 180.0).abs() <= 7.0,
        "plain tag appears off center: {left}..{right}"
    );
    for style in ["boxed", "plain"] {
        for (layout, order, direction) in [
            ("split", "a_first", 1isize),
            ("split", "b_first", -1isize),
            ("fill", "a_first", 1isize),
        ] {
            let mut green_tops = Vec::new();
            for distance in ["0", "35"] {
                let mut moved = params.clone();
                moved.insert("social_tag_style".into(), style.into());
                moved.insert("region_order".into(), order.into());
                moved.insert("social_tag_seam_offset".into(), distance.into());
                let filter = clipper_social_tag_filter(&moved, layout, 360, 640, &info)
                    .unwrap()
                    .unwrap();
                let output = hidden_command("ffmpeg")
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=0x303844:s=360x640:r=1:d=1",
                        "-vf",
                        &filter,
                        "-frames:v",
                        "1",
                        "-pix_fmt",
                        "rgb24",
                        "-f",
                        "rawvideo",
                        "-",
                    ])
                    .output()
                    .await
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{style}/{layout}/{order}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let green_top = output
                    .stdout
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .enumerate()
                    .filter(|(_, rgb)| rgb[0] < 140 && rgb[1] > 180 && rgb[2] < 140)
                    .map(|(index, _)| index / 360)
                    .min()
                    .unwrap();
                green_tops.push(green_top as isize);
            }
            assert!(
                (green_tops[1] - green_tops[0]) * direction > 20,
                "{style}/{layout}/{order} export ignored tag distance: {green_tops:?}"
            );
        }
    }
}

#[tokio::test]
async fn rasterized_text_overlay_renders_image_and_video() {
    let directory = OperationTempGuard(operation_temp_dir("text-raster-test").unwrap());
    let raster = directory.0.join("emoji.png");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/social-tags/kick-plain.png"),
        &raster,
    )
    .unwrap();
    let font = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
        .join("Fonts/arial.ttf");
    let layers = serde_json::json!([{
        "text":"Emoji 😀", "x":50, "y":50, "size":42, "color":"#ffffff",
        "opacity":100, "font_path":font
    }])
    .to_string();
    for kind in ["image", "video"] {
        let input = directory.0.join(if kind == "image" {
            "source.png"
        } else {
            "source.mp4"
        });
        let mut source = hidden_command("ffmpeg");
        source
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
            ])
            .arg("testsrc2=s=160x90:r=25:d=1");
        if kind == "image" {
            source.args(["-frames:v", "1"]);
        } else {
            source.args(["-c:v", "libx264", "-pix_fmt", "yuv420p"]);
        }
        assert!(source.arg(&input).status().await.unwrap().success());
        let info = probe_media(input.to_string_lossy().into_owned())
            .await
            .unwrap();
        let request = OperationRequest {
            input: input.to_string_lossy().into_owned(),
            operation: "text".into(),
            params: values(&[
                ("layers", &layers),
                ("text_raster_path", &raster.to_string_lossy()),
            ]),
        };
        let (args, output) = build_command(&request, &info).await.unwrap();
        assert!(args.contains(&"-filter_complex".to_string()));
        assert!(hidden_command("ffmpeg")
            .args(&args)
            .status()
            .await
            .unwrap()
            .success());
        let rendered = probe_media(output.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!((rendered.width, rendered.height), (Some(160), Some(90)));
        if kind == "video" {
            let mut reference = args.clone();
            reference.truncate(reference.iter().position(|arg| arg == "-c:v").unwrap());
            reference.extend([
                "-frames:v".into(),
                "1".into(),
                "-an".into(),
                "-pix_fmt".into(),
                rendered.pixel_format.clone().unwrap(),
                "-f".into(),
                "rawvideo".into(),
                "-".into(),
            ]);
            let expected = hidden_command("ffmpeg")
                .args(&reference)
                .output()
                .await
                .unwrap();
            let decoded = hidden_command("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&output)
                .args(["-frames:v", "1", "-an", "-f", "rawvideo", "-"])
                .output()
                .await
                .unwrap();
            assert!(
                expected.status.success(),
                "{}",
                String::from_utf8_lossy(&expected.stderr)
            );
            assert!(decoded.status.success());
            assert!(
                expected.stdout == decoded.stdout,
                "Text encoding must preserve the composed frame exactly"
            );
        }
    }
}

#[test]
fn social_tag_export_geometry_matches_preview_fixtures() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../tooling/tests/fixtures/social-tag-geometry.json"
    ))
    .unwrap();
    let info = MediaInfo {
        path: "source.mp4".into(),
        name: "source.mp4".into(),
        kind: "video".into(),
        duration: Some(1.0),
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        codec: "h264".into(),
        audio_codec: None,
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: Some(8),
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    for fixture in fixtures.as_array().unwrap() {
        let layout = fixture["layout"].as_str().unwrap();
        let style = fixture["style"].as_str().unwrap();
        let params = values(&[
            ("social_tag_enabled", "true"),
            ("social_tag_platform", "kick"),
            ("social_tag_username", fixture["username"].as_str().unwrap()),
            ("social_tag_style", style),
            (
                if style == "boxed" {
                    "social_tag_boxed_position"
                } else {
                    "social_tag_plain_position"
                },
                fixture["position"].as_str().unwrap(),
            ),
            ("social_tag_size", "36"),
            ("region_a_height", "30"),
            ("region_order", "a_first"),
            ("region_a_x", "0"),
            ("region_a_y", "0"),
            ("region_a_w", "50"),
            ("region_a_h", "50"),
            ("freecam_size", "50"),
            ("freecam_x", "50"),
            ("freecam_y", "2"),
        ]);
        let filter = clipper_social_tag_filter(&params, layout, 360, 640, &info)
            .unwrap()
            .unwrap();
        let x = fixture["x"].as_i64().unwrap();
        let y = fixture["y"].as_i64().unwrap();
        let side = fixture["side"].as_i64().unwrap();
        let font_size = fixture["fontSize"].as_f64().unwrap();
        assert!(
            filter.contains(&format!("fontsize={font_size:.0}:")),
            "{layout}/{style}: {filter}"
        );
        if style == "boxed" {
            assert!(
                filter.starts_with(&format!("drawbox=x={x}:y={y}:w={side}:h={side}:")),
                "{layout}/{style}: {filter}"
            );
        } else {
            let expected_text_x = (x as f64 + side as f64 + font_size * 0.22).round() as i64;
            let expected_center_y = y + side / 2;
            assert!(
                filter.contains(&format!(
                    ":x={expected_text_x}:y={expected_center_y}-text_h/2"
                )),
                "{layout}/{style}: {filter}"
            );
        }
    }
}

#[tokio::test]
async fn boxed_social_tag_render_uses_the_preview_badge_bounds() {
    let info = MediaInfo {
        path: "source.mp4".into(),
        name: "source.mp4".into(),
        kind: "video".into(),
        duration: Some(1.0),
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        codec: "h264".into(),
        audio_codec: None,
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: Some(8),
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    for (platform, username) in [("kick", "eray"), ("twitch", "Example")] {
        let params = values(&[
            ("social_tag_enabled", "true"),
            ("social_tag_platform", platform),
            ("social_tag_username", username),
            ("social_tag_style", "boxed"),
            ("social_tag_size", "36"),
            ("region_a_height", "30"),
            ("region_order", "a_first"),
        ]);
        let filter = clipper_social_tag_filter(&params, "split", 360, 640, &info)
            .unwrap()
            .unwrap();
        let output = hidden_command("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x303844:s=360x640:r=1:d=1",
            ])
            .args([
                "-vf",
                &format!("{filter},format=rgb24"),
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{platform}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 360 * 640 * 3);
        let icon_x: usize = filter
            .strip_prefix("drawbox=x=")
            .unwrap()
            .split(':')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let colored: Vec<(usize, usize)> = output
            .stdout
            .as_chunks::<3>()
            .0
            .iter()
            .enumerate()
            .filter_map(|(index, rgb)| {
                let badge = if platform == "kick" {
                    rgb[0] > 55 && rgb[0] < 115 && rgb[1] > 180 && rgb[2] < 70
                } else {
                    (icon_x..icon_x + 54).contains(&(index % 360))
                        && rgb.iter().all(|channel| *channel > 225)
                };
                badge.then_some((index % 360, index / 360))
            })
            .collect();
        assert!(!colored.is_empty(), "{platform} badge color is missing");
        assert_eq!(colored.iter().map(|point| point.0).min(), Some(icon_x));
        assert_eq!(colored.iter().map(|point| point.1).min(), Some(138));
        assert_eq!(colored.iter().map(|point| point.0).max(), Some(icon_x + 53));
        assert_eq!(colored.iter().map(|point| point.1).max(), Some(191));
        let text_rows: Vec<usize> = output
            .stdout
            .as_chunks::<3>()
            .0
            .iter()
            .enumerate()
            .filter_map(|(index, rgb)| {
                let x = index % 360;
                let y = index / 360;
                ((icon_x + 54..360).contains(&x)
                    && (138..192).contains(&y)
                    && rgb.iter().all(|channel| *channel > 190))
                .then_some(y)
            })
            .collect();
        assert!(!text_rows.is_empty(), "{platform} text is missing");
        let text_center = text_rows.iter().sum::<usize>() as f64 / text_rows.len() as f64;
        assert!(
            (text_center - 164.5).abs() <= 2.0,
            "{platform}/{username} text center {text_center} differs from badge center 164.5"
        );
    }
}

#[tokio::test]
async fn social_tag_movie_handles_special_paths_and_repeats_until_video_end() {
    let root = operation_temp_dir("social-mark-test").unwrap();
    let path = root.join("Türkçe ' [logo], %; cache.png");
    std::fs::copy(social_tag_mark_path("twitch", "plain").unwrap(), &path).unwrap();
    let graph = format!(
        "null[base];movie=filename={}:dec_threads=1,scale=24:24:flags=lanczos,format=rgba[mark];[base][mark]overlay=x=3:y=5:format=yuv444:eof_action=repeat:repeatlast=1,format=rgb24",
        movie_filter_path(&path)
    );
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        hidden_command("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:r=12:d=2",
                "-vf",
                &graph,
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .output(),
    )
    .await;
    let _ = std::fs::remove_dir_all(&root);
    let output = result
        .expect("still logo must not extend video duration")
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let frame_size = 64 * 64 * 3;
    assert_eq!(output.stdout.len(), frame_size * 24);
    let first = &output.stdout[..frame_size];
    assert!(
        first
            .as_chunks::<3>()
            .0
            .iter()
            .any(|rgb| rgb[2] > 180 && rgb[0] > 80),
        "Twitch logo missing"
    );
    for frame in output.stdout.chunks_exact(frame_size) {
        assert_eq!(
            frame, first,
            "logo must remain visible after its still frame ends"
        );
    }
}

#[test]
fn new_tool_capability_parsers_match_the_used_ffmpeg_paths() {
    assert!(filter_list_has(" T.. boxblur V->V\n", "boxblur"));
    assert!(!filter_list_has(" T.. gblur V->V\n", "boxblur"));
    assert!(demuxer_list_has(
        " D  concat          Virtual concatenation script\n",
        "concat"
    ));
    assert!(!demuxer_list_has(
        "  E concat          Concatenation filter output\n",
        "concat"
    ));
}

#[test]
fn merge_normalization_never_upscales_the_smallest_input() {
    assert_eq!(
        select_merge_normalization_target(&[
            (3840, 2160, 60.0),
            (1920, 1080, 30.0),
            (1280, 720, 30.0),
        ]),
        Some((1280, 720, 30.0))
    );
}

#[test]
fn fcpxml_file_urls_escape_reserved_and_unicode_characters() {
    let path = std::env::temp_dir().join("Türkçe # yüzde%.mov");
    let url = file_url(&path).unwrap();
    assert!(url.starts_with("file:///"));
    assert!(url.contains("%23"));
    assert!(url.contains("%25"));
    assert!(!url.contains('ü'));
}

#[test]
fn active_process_registration_never_leaves_or_clears_an_unrelated_pid() {
    let state = JobState::default();
    {
        let _active = ActiveProcess::register(&state, Some(41)).unwrap();
        assert_eq!(*state.pid.lock().unwrap(), Some(41));
    }
    assert_eq!(*state.pid.lock().unwrap(), None);

    let active = ActiveProcess::register(&state, Some(42)).unwrap();
    *state.pid.lock().unwrap() = Some(43);
    drop(active);
    assert_eq!(*state.pid.lock().unwrap(), Some(43));
}

#[test]
fn bundled_downloader_is_materialized_once_without_overwriting_a_replacement() {
    let root = std::env::temp_dir().join(format!(
        "container_downloader_runtime_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let bundled = root.join("bundled.exe");
    let managed = root.join("managed").join("yt-dlp.exe");
    std::fs::write(&bundled, b"official bundled build").unwrap();

    assert!(materialize_bundled_downloader(&bundled, &managed).unwrap());
    assert_eq!(std::fs::read(&managed).unwrap(), b"official bundled build");
    std::fs::write(&managed, b"user replacement").unwrap();
    assert!(!materialize_bundled_downloader(&bundled, &managed).unwrap());
    assert_eq!(std::fs::read(&managed).unwrap(), b"user replacement");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(target_os = "windows")]
fn recycle_bin_contains_test_path(root: &Path, expected: &Path) -> bool {
    // Shell metadata on a CI runner can spell the volume or profile root
    // differently from std::env::temp_dir(). The unique test directory and
    // its child path still identify the same recycled item.
    let suffix =
        PathBuf::from(root.file_name().unwrap()).join(expected.strip_prefix(root).unwrap());
    trash::os_limited::list()
        .unwrap()
        .iter()
        .any(|item| item.original_path().ends_with(&suffix))
}

#[cfg(target_os = "windows")]
#[test]
fn windows_recycle_bin_accepts_a_nested_output_folder() {
    let root = std::env::temp_dir().join(format!(
        "container_recycle_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let folder = root.join("smartcut");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("example_smartcut.mp4"), b"test fixture").unwrap();
    let entries = vec![folder];
    let root_for_worker = root.clone();
    let result = std::thread::spawn(move || recycle_output_entries(&root_for_worker, &entries))
        .join()
        .unwrap();
    assert!(
        result.is_ok(),
        "nested output folder was not recycled: {result:?}"
    );
    assert!(!root.join("smartcut").exists());
    assert!(
        recycle_bin_contains_test_path(&root, &root.join("smartcut")),
        "recycled folder was not found in the Windows Recycle Bin"
    );
    std::fs::remove_dir(&root).unwrap();
}

#[cfg(target_os = "windows")]
#[test]
fn windows_recycle_bin_recovers_after_a_locked_output_file() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = std::env::temp_dir().join(format!(
        "container_recycle_locked_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let folder = root.join("smartcut");
    std::fs::create_dir_all(&folder).unwrap();
    let locked = folder.join("open.mp4");
    let free = folder.join("ready.mp4");
    std::fs::write(&locked, b"in use").unwrap();
    std::fs::write(&free, b"ready").unwrap();
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&locked)
        .unwrap();

    let entries = vec![folder.clone()];
    let root_for_worker = root.clone();
    let first = std::thread::spawn(move || recycle_output_entries(&root_for_worker, &entries))
        .join()
        .unwrap();
    assert!(first.unwrap_err().contains("open.mp4"));
    assert!(
        locked.exists(),
        "locked output must not be deleted permanently"
    );
    assert!(
        !free.exists(),
        "unlocked output should still reach the Recycle Bin"
    );
    // The separate nested-folder test verifies Recycle Bin placement.
    // Here the shell's per-file listing is inconsistent on CI, so verify
    // the locked child stays put while the unlocked sibling is processed.

    drop(handle);
    let entries = vec![folder];
    let root_for_worker = root.clone();
    let retry = std::thread::spawn(move || recycle_output_entries(&root_for_worker, &entries))
        .join()
        .unwrap();
    assert!(
        retry.is_ok(),
        "retry after closing the file failed: {retry:?}"
    );
    assert!(!root.join("smartcut").exists());
    std::fs::remove_dir(root).unwrap();
}

#[cfg(target_os = "windows")]
#[test]
fn process_tree_cancellation_ends_a_real_worker() {
    use std::os::windows::process::CommandExt;

    let mut child = std::process::Command::new("ping.exe")
        .args(["-t", "127.0.0.1"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .unwrap();
    stop_process_tree(child.id());
    let ended = (0..20).any(|_| {
        if child.try_wait().ok().flatten().is_some() {
            true
        } else {
            std::thread::sleep(std::time::Duration::from_millis(50));
            false
        }
    });
    if !ended {
        let _ = child.kill();
    }
    assert!(ended, "cancelled worker process remained alive");
}

#[test]
fn batch_folder_scan_handles_unicode_and_respects_recursion() {
    let root = std::env::temp_dir().join(format!("container_batch_unicode_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let nested = root.join("Türkçe klasör");
    std::fs::create_dir_all(&nested).unwrap();
    let media = nested.join("görüntü.mp4");
    std::fs::write(&media, b"test").unwrap();
    std::fs::write(nested.join("atlanacak.txt"), b"test").unwrap();

    let mut files = Vec::new();
    collect_media_files(&root, false, &mut files).unwrap();
    assert!(files.is_empty());
    collect_media_files(&root, true, &mut files).unwrap();
    assert_eq!(files, vec![media.to_string_lossy().to_string()]);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn corrupt_unicode_media_is_rejected_without_panicking() {
    let path = std::env::temp_dir().join(format!(
        "container_bozuk_görüntü_{}.mp4",
        std::process::id()
    ));
    std::fs::write(&path, b"this is not a media container").unwrap();
    let error = probe_media(path.to_string_lossy().to_string())
        .await
        .expect_err("corrupt media must be rejected");
    assert!(!error.trim().is_empty());
    let _ = std::fs::remove_file(path);
}

#[test]
fn downloader_only_accepts_regular_https_urls() {
    assert_eq!(
        validated_download_url("https://www.youtube.com/watch?v=example").unwrap(),
        "https://www.youtube.com/watch?v=example"
    );
    assert!(validated_download_url("http://example.com/video").is_err());
    assert!(validated_download_url("https://user:pass@example.com/video").is_err());
    assert!(validated_download_url("https://example.com/line\nbreak").is_err());
    assert!(validated_download_url("https://localhost/video").is_err());
    assert!(validated_download_url("https://127.0.0.1/video").is_err());
    assert!(validated_download_url("https://192.168.1.10/video").is_err());
    assert!(validated_download_url("https://[::1]/video").is_err());
}

#[test]
fn downloader_rejects_disguised_private_hosts() {
    for url in [
        "https://localhost./video",
        "https://[::ffff:127.0.0.1]/video",
        "https://[::ffff:c0a8:010a]/video",
        "https://[64:ff9b::7f00:1]/video",
        "https://100.64.0.1/video",
        "https://0.1.2.3/video",
        "https://224.0.0.1/video",
        "https://198.18.0.1/video",
        "https://2130706433/video",
        "https://0x7f.1/video",
    ] {
        assert!(
            validated_download_url(url).is_err(),
            "{url} should be rejected"
        );
    }
    for url in [
        "https://93.184.216.34/video",
        "https://[2606:4700::1111]/video",
        "https://100.128.0.1/video",
        "https://vimeo.com/123",
    ] {
        assert!(
            validated_download_url(url).is_ok(),
            "{url} should be accepted"
        );
    }
}

#[test]
fn smartcut_resolution_is_allowlisted() {
    for value in ["source", "1080", "720", "480"] {
        assert!(validate_smartcut_resolution(value).is_ok());
    }
    for value in ["", "2160", "720[vout];movie=x", "-1", "720:flags=x"] {
        assert!(validate_smartcut_resolution(value).is_err(), "{value:?}");
    }
}

#[test]
fn keep_intervals_ignore_invalid_durations_without_panicking() {
    let cuts = vec![KeepInterval {
        start: 0.0,
        end: 1.0,
        enabled: true,
    }];
    assert!(normalize_keep_intervals(&cuts, f64::NAN).is_empty());
    assert!(normalize_keep_intervals(&cuts, -1.0).is_empty());
    assert!(enabled_cuts(&cuts, f64::INFINITY).is_err());
    assert_eq!(enabled_cuts(&cuts, 5.0).unwrap().len(), 1);
}

#[test]
fn output_cleanup_reservation_respects_running_jobs() {
    let state = JobState::default();
    state.cancelled.store(true, Ordering::Relaxed);
    {
        let _job = JobLease::acquire(&state, Some("render".into())).unwrap();
        assert!(JobLease::reserve(&state, "output-cleanup").is_err());
    }
    state.cancelled.store(true, Ordering::Relaxed);
    {
        let _cleanup = JobLease::reserve(&state, "output-cleanup").unwrap();
        // Reserving must not clear a pending cancellation flag.
        assert!(state.cancelled.load(Ordering::Relaxed));
        assert!(JobLease::acquire(&state, Some("render".into())).is_err());
    }
    assert!(JobLease::acquire(&state, Some("render".into())).is_ok());
}

#[test]
fn downloader_progress_uses_the_machine_readable_template() {
    let progress = parse_downloader_progress("CONTAINER_PROGRESS:42.7%|18.4MiB|3.2MiB/s")
        .expect("valid progress line");
    assert_eq!(progress.percent, 42.7);
    assert_eq!(progress.downloaded, "18.4MiB");
    assert_eq!(progress.speed, "3.2MiB/s");
    assert!(parse_downloader_progress("[download] 42.7%").is_none());
}

#[test]
fn downloader_keeps_selected_audio_audio_only() {
    assert_eq!(
        downloader_format_args("audio-format:m4a").unwrap(),
        ["-f", "bestaudio/best", "-x", "--audio-format", "m4a"]
    );
    assert_eq!(
        downloader_format_args("audio-format:opus").unwrap(),
        ["-f", "bestaudio/best", "-x", "--audio-format", "opus"]
    );
    assert_eq!(
        downloader_format_args("audio:140-1").unwrap(),
        ["-f", "140-1", "-x", "--audio-format", "m4a"]
    );
    assert_eq!(
        downloader_format_args("299").unwrap(),
        ["-f", "299+bestaudio/299"]
    );
    assert!(downloader_format_args("audio:bad+format").is_err());
}

#[test]
fn downloader_explicitly_uses_the_bundled_deno_runtime() {
    let path = Path::new(r"C:\Program Files\CONTAINER\deno.exe");
    assert_eq!(
        downloader_javascript_args(path),
        ["--js-runtimes", r"deno:C:\Program Files\CONTAINER\deno.exe"]
    );
}

#[test]
fn downloader_uses_browser_impersonation_only_for_tiktok() {
    assert_eq!(
        downloader_site_args("https://www.tiktok.com/@creator/video/123"),
        ["--impersonate", "chrome"]
    );
    assert_eq!(
        downloader_site_args("https://vm.tiktok.com/example"),
        ["--impersonate", "chrome"]
    );
    assert!(downloader_site_args("https://example.com/tiktok.com/video").is_empty());
}

#[test]
fn downloader_cleanup_only_removes_new_partial_files() {
    let root = std::env::temp_dir().join(format!(
        "container_downloader_cleanup_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let old_part = root.join("existing.mp4.part");
    let new_part = root.join("current.mp4.part");
    let finished = root.join("finished.mp4");
    std::fs::write(&old_part, b"old").unwrap();
    let state = JobState::default();
    *state.download_cleanup.lock().unwrap() = Some(DownloadCleanup {
        output_dir: root.clone(),
        existing_temporary_files: downloader_temporary_files(&root),
    });
    std::fs::write(&new_part, b"new").unwrap();
    std::fs::write(&finished, b"done").unwrap();
    cleanup_current_download(&state);
    assert!(old_part.is_file());
    assert!(!new_part.exists());
    assert!(finished.is_file());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn downloader_thumbnail_cleanup_is_limited_to_its_temp_file() {
    let thumbnail = download_thumbnail_path("https://example.com/thumbnail.jpg");
    std::fs::write(&thumbnail, b"preview").unwrap();
    assert!(remove_download_thumbnail_file(&thumbnail).unwrap());
    assert!(!thumbnail.exists());
    assert!(!remove_download_thumbnail_file(&thumbnail).unwrap());

    let unrelated = std::env::temp_dir().join(format!(
        "container-unrelated-thumbnail-{}.jpg",
        std::process::id()
    ));
    std::fs::write(&unrelated, b"keep").unwrap();
    assert!(remove_download_thumbnail_file(&unrelated).is_err());
    assert!(unrelated.is_file());
    std::fs::remove_file(unrelated).unwrap();
}

#[test]
fn bundled_ffmpeg_pair_is_materialized_outside_the_install_folder() {
    let root = std::env::temp_dir().join(format!(
        "container-runtime-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = root.join("install");
    let runtime = root.join("runtime");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("ffmpeg.exe"), b"ffmpeg").unwrap();
    std::fs::write(source.join("ffprobe.exe"), b"ffprobe").unwrap();

    assert!(migrate_runtime_pair(&source, &runtime).unwrap());
    std::fs::remove_dir_all(&source).unwrap();
    assert_eq!(
        std::fs::read(runtime.join("ffmpeg.exe")).unwrap(),
        b"ffmpeg"
    );
    assert_eq!(
        std::fs::read(runtime.join("ffprobe.exe")).unwrap(),
        b"ffprobe"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn embedded_vad_loads_from_empty_cache_and_repairs_corruption() {
    let directory = std::env::temp_dir().join(format!(
        "container-vad-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut detector = load_embedded_vad(&directory).unwrap();
    let score = detector.forward_chunk(&[0.0; 512], 16_000).unwrap();
    assert!(score[[0, 0]].is_finite());
    drop(detector);
    let path = std::fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match std::fs::write(&path, b"corrupt model") {
            Ok(()) => break,
            Err(error)
                if cfg!(target_os = "windows")
                    && error.raw_os_error() == Some(32)
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => panic!("test model could not be corrupted: {error}"),
        }
    }
    let mut detector = load_embedded_vad(&directory).unwrap();
    assert!(detector.forward_chunk(&[0.0; 512], 16_000).unwrap()[[0, 0]].is_finite());
    drop(detector);
    // ONNX Runtime can briefly retain the model file on Windows after the
    // session is dropped. Cleanup is not part of this behavior test, and
    // CI workspaces are ephemeral, so a transient sharing violation must
    // not turn a successful load-and-repair assertion into a test failure.
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(directory);
}

#[test]
fn export_uses_only_enabled_valid_cuts() {
    let cuts = vec![
        KeepInterval {
            start: 3.0,
            end: 4.0,
            enabled: true,
        },
        KeepInterval {
            start: 1.0,
            end: 2.0,
            enabled: false,
        },
        KeepInterval {
            start: -2.0,
            end: 0.5,
            enabled: true,
        },
        KeepInterval {
            start: 9.0,
            end: 9.001,
            enabled: true,
        },
    ];
    let enabled = enabled_cuts(&cuts, 5.0).unwrap();
    assert_eq!(enabled.len(), 2);
    assert_eq!(enabled[0].start, 0.0);
    assert_eq!(enabled[1].start, 3.0);
}

#[test]
fn export_normalization_merges_duplicate_overlapping_and_touching_ranges() {
    let cuts = vec![
        KeepInterval {
            start: 4.0,
            end: 5.0,
            enabled: true,
        },
        KeepInterval {
            start: 0.0,
            end: 1.0,
            enabled: true,
        },
        KeepInterval {
            start: 0.0,
            end: 1.0,
            enabled: true,
        },
        KeepInterval {
            start: 0.8,
            end: 2.0,
            enabled: true,
        },
        KeepInterval {
            start: 2.0005,
            end: 3.0,
            enabled: true,
        },
        KeepInterval {
            start: 4.2,
            end: 4.8,
            enabled: true,
        },
        KeepInterval {
            start: -5.0,
            end: -1.0,
            enabled: true,
        },
        KeepInterval {
            start: 9.0,
            end: 8.0,
            enabled: true,
        },
    ];
    let normalized = enabled_cuts(&cuts, 8.0).unwrap();
    assert_eq!(normalized.len(), 2);
    assert_eq!((normalized[0].start, normalized[0].end), (0.0, 3.0));
    assert_eq!((normalized[1].start, normalized[1].end), (4.0, 5.0));
    assert!(normalized
        .windows(2)
        .all(|pair| pair[0].end < pair[1].start));
}

#[test]
fn automatic_audio_profile_separates_noise_from_voice() {
    let mut levels = vec![-58.0; 80];
    levels.extend(vec![-14.0; 120]);
    levels.extend(vec![-55.0; 25]);
    levels.extend(vec![-16.0; 100]);
    let recommendation = recommend_from_levels(&levels, 0.02);
    assert!((0.35..=0.65).contains(&recommendation.threshold));
    assert!((0.18..=0.80).contains(&recommendation.min_silence));
    assert!((0.10..=0.30).contains(&recommendation.min_speech));
    assert!((0.35..=0.65).contains(&recommendation.minimum_pause));
    assert_eq!(recommendation.keep_before_speech, 0.10);
    assert_eq!(recommendation.keep_after_speech, 0.18);
    assert!(recommendation.speech_level_db > recommendation.noise_floor_db);
}

#[tokio::test]
async fn real_ffmpeg_audio_is_scored_and_cached_by_vad() {
    let root = std::env::temp_dir().join("container_autocut_detection_test");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("speech-shape.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=160x90:r=25:d=3",
            "-f",
            "lavfi",
            "-i",
            r"aevalsrc=if(between(t\,1\,2)\,0.5*sin(2*PI*440*t)\,0):s=48000:d=3",
            "-shortest",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
        ])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    let state = JobState::default();
    let scores = cached_vad_scores(&state, &source).await.unwrap();
    assert!(!scores.is_empty());
    let cached = cached_vad_scores(&state, &source).await.unwrap();
    assert_eq!(scores, cached);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cached_vad_probabilities_are_resegmented_without_model_rerun() {
    let mut scores = vec![0.02; 10];
    scores.extend(vec![0.9; 30]);
    scores.extend(vec![0.01; 10]);
    let keeps = keeps_from_vad_scores(&scores, 0.5, 0.1, 0.1, 0.05, 2.0);
    assert_eq!(keeps.len(), 1);
    assert!(keeps[0].start > 0.2 && keeps[0].start < 0.4);
    assert!(keeps[0].end > 1.2 && keeps[0].end < 1.4);
}

#[test]
fn vad_ignores_isolated_noise_spikes() {
    let mut scores = vec![0.02; 80];
    scores[25] = 0.99;
    scores[55] = 0.90;
    let keeps = keeps_from_vad_scores(&scores, 0.5, 0.25, 0.12, 0.1, 3.0);
    assert!(keeps.is_empty());
}

#[test]
fn vad_keeps_words_together_across_brief_dips() {
    let mut scores = vec![0.02; 10];
    scores.extend(vec![0.88; 20]);
    scores.extend(vec![0.01; 5]);
    scores.extend(vec![0.82; 20]);
    scores.extend(vec![0.01; 12]);
    let keeps = keeps_from_vad_scores(&scores, 0.5, 0.25, 0.12, 0.08, 3.0);
    assert_eq!(keeps.len(), 1);
    assert!(keeps[0].start < 0.35);
    assert!(keeps[0].end > 1.70);
}

#[test]
fn vad_matches_autocut_hysteresis_without_smoothing_or_attack() {
    // AutoCut enters immediately at 0.50, remains active at 0.40, and
    // exits only below 0.35. A one-chunk utterance is kept when the
    // requested minimum speech duration is one chunk.
    let scores = [0.0, 0.50, 0.40, 0.34, 0.0];
    let keeps = keeps_from_vad_scores(&scores, 0.50, 0.0, 0.032, 0.0, 1.0);
    assert_eq!(keeps.len(), 1);
    assert_eq!(keeps[0].start, 0.032);
    assert_eq!(keeps[0].end, 0.096);
}

#[test]
fn vad_uses_autocuts_strict_min_silence_boundary() {
    // With a 64 ms requirement, a two-chunk (64 ms) gap is a real split;
    // AutoCut merges only gaps strictly shorter than the requested value.
    let scores = [0.9, 0.0, 0.0, 0.9];
    let keeps = keeps_from_vad_scores(&scores, 0.5, 0.064, 0.032, 0.0, 1.0);
    assert_eq!(keeps.len(), 2);
    assert_eq!((keeps[0].start, keeps[0].end), (0.0, 0.032));
    assert_eq!((keeps[1].start, keeps[1].end), (0.096, 0.128));
}

#[test]
fn vad_applies_padding_after_segmentation_and_merges_overlap() {
    let scores = [0.9, 0.0, 0.0, 0.9];
    let keeps = keeps_from_vad_scores(&scores, 0.5, 0.032, 0.032, 0.04, 1.0);
    assert_eq!(keeps.len(), 1);
    assert_eq!(keeps[0].start, 0.0);
    assert_eq!(keeps[0].end, 0.168);
}

#[test]
fn strong_two_frame_interjection_survives_min_speech_filter() {
    let scores = [0.0, 0.96, 0.95, 0.0];
    let keeps = keeps_from_vad_scores(&scores, 0.5, 0.1, 0.15, 0.0, 1.0);
    assert_eq!(keeps.len(), 1);
    assert!((keeps[0].end - keeps[0].start - 0.064).abs() < 1e-9);
}

#[test]
fn editing_minimum_pause_preserves_natural_micro_pauses() {
    let mut scores = vec![0.9; 10];
    scores.extend(vec![0.0; 8]);
    scores.extend(vec![0.9; 10]);
    let samples = vec![0_i16; scores.len() * 512];
    let (balanced, _) = natural_keeps_from_vad_scores(
        &scores,
        &samples,
        0.5,
        0.1,
        0.15,
        AutoCutEditSettings {
            minimum_pause: 0.35,
            keep_before_speech: 0.1,
            keep_after_speech: 0.18,
            boundary_refinement: false,
        },
        2.0,
    );
    assert_eq!(balanced.len(), 1, "256 ms pause should remain intact");
}

#[test]
fn asymmetric_padding_uses_after_on_left_and_before_on_right() {
    let mut scores = vec![0.9; 10];
    scores.extend(vec![0.0; 20]);
    scores.extend(vec![0.9; 10]);
    let samples = vec![0_i16; scores.len() * 512];
    let (keeps, _) = natural_keeps_from_vad_scores(
        &scores,
        &samples,
        0.5,
        0.1,
        0.15,
        AutoCutEditSettings {
            minimum_pause: 0.35,
            keep_before_speech: 0.1,
            keep_after_speech: 0.18,
            boundary_refinement: false,
        },
        2.0,
    );
    assert_eq!(keeps.len(), 2);
    assert!((keeps[0].end - (0.320 + 0.180)).abs() < 1e-9);
    assert!((keeps[1].start - (0.960 - 0.100)).abs() < 1e-9);
}

#[test]
fn boundary_refinement_only_expands_toward_safe_quiet_audio() {
    let sample = 8_000;
    let mut samples = vec![0_i16; 12_000];
    for value in &mut samples[sample - 320..sample + 800] {
        *value = 8_000;
    }
    let refined_start = refine_boundary(&samples, sample, -1);
    let refined_end = refine_boundary(&samples, sample, 1);
    assert!(refined_start <= sample);
    assert!(refined_end >= sample + 800);
}

#[test]
fn editing_presets_are_centralized_and_directionally_safe() {
    assert_eq!(AUTOCUT_PRESETS[0].id, "natural");
    assert_eq!(AUTOCUT_PRESETS[1].id, "balanced");
    assert_eq!(AUTOCUT_PRESETS[2].id, "tight");
    for preset in AUTOCUT_PRESETS {
        assert!(preset.keep_after_speech > preset.keep_before_speech);
    }
}

#[tokio::test]
#[ignore = "set CONTAINER_VAD_PARITY_MEDIA to run the real-media parity report"]
async fn vad_real_media_parity_report() {
    let source = PathBuf::from(
        std::env::var("CONTAINER_VAD_PARITY_MEDIA")
            .expect("CONTAINER_VAD_PARITY_MEDIA must point to a media file"),
    );
    let duration_output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(&source)
        .output()
        .unwrap();
    assert!(duration_output.status.success());
    let duration = String::from_utf8_lossy(&duration_output.stdout)
        .trim()
        .parse::<f64>()
        .unwrap();
    let state = JobState::default();
    let (scores, samples) = cached_vad_data(&state, &source).await.unwrap();
    let v6 = keeps_from_vad_scores(&scores, 0.5, 0.1, 0.15, 0.3, duration);
    let v6_kept: f64 = v6.iter().map(|keep| keep.end - keep.start).sum();
    eprintln!(
        "V6_PARITY source={} chunks={} regions={} kept={v6_kept:.3}s removed={:.3}s duration={duration:.3}s",
        source.display(),
        scores.len(),
        v6.len(),
        duration - v6_kept
    );
    let balanced_settings = AutoCutEditSettings {
        minimum_pause: 0.35,
        keep_before_speech: 0.10,
        keep_after_speech: 0.18,
        boundary_refinement: true,
    };
    let (balanced, overlaps) = natural_keeps_from_vad_scores(
        &scores,
        &samples,
        0.5,
        0.1,
        0.15,
        balanced_settings,
        duration,
    );
    let balanced_kept: f64 = balanced.iter().map(|keep| keep.end - keep.start).sum();
    eprintln!(
        "BALANCED regions={} cuts={} kept={balanced_kept:.3}s removed={:.3}s threshold=.50 minimum_pause=.350s min_speech=.150s before=.100s after=.180s refinement=true overlaps_before_normalization={overlaps}",
        balanced.len(),
        balanced.len().saturating_sub(1),
        duration - balanced_kept
    );
    assert!(!balanced.is_empty());
    if std::env::var("CONTAINER_PARITY_EXPORT").as_deref() == Ok("1") {
        let expected_duration: f64 = balanced.iter().map(|cut| cut.end - cut.start).sum();
        let fps = 30.0_f64;
        let expected_video_duration: f64 = balanced
            .iter()
            .map(|cut| {
                let frame_duration = ((cut.end * fps).ceil() - (cut.start * fps).ceil()) / fps;
                frame_duration.max(cut.end - cut.start)
            })
            .sum();
        let rendered = export_autocut_inner(
            None,
            &state,
            AutoCutExportRequest {
                input: source.to_string_lossy().into_owned(),
                cuts: balanced.clone(),
                format: "mp4".into(),
                quality: "medium".into(),
                resolution: "source".into(),
                linked_tracks: Vec::new(),
            },
        )
        .await
        .unwrap();
        let rendered = PathBuf::from(rendered.output);
        let info = probe_media(rendered.to_string_lossy().into_owned())
            .await
            .unwrap();
        let actual_duration = info.duration.unwrap();
        eprintln!("RENDER_DURATIONS logical={expected_duration:.6} frame_aligned={expected_video_duration:.6} actual={actual_duration:.6}");
        assert!((actual_duration - expected_video_duration).abs() < 0.05);
        let mut output_cursor = 0.0;
        for cut in &balanced {
            let first_frame = (cut.start * fps).ceil();
            let frame_count = ((cut.end * fps).ceil() - first_frame).max(1.0);
            let segment_duration = (frame_count / fps).max(cut.end - cut.start);
            let middle_frame = (frame_count / 2.0).floor();
            let source_signature =
                sample_video_signature(&source, (first_frame + middle_frame) / fps);
            let output_signature =
                sample_video_signature(&rendered, output_cursor + middle_frame / fps);
            let mean_error = source_signature
                .iter()
                .zip(output_signature.iter())
                .map(|(left, right)| (*left as f64 - *right as f64).abs())
                .sum::<f64>()
                / source_signature.len() as f64;
            assert!(
                mean_error < 14.0,
                "source-range order mismatch: MAE={mean_error}"
            );
            output_cursor += segment_duration;
        }
        assert!(packet_timestamps_are_monotonic(&rendered, "v:0"));
        assert!(packet_timestamps_are_monotonic(&rendered, "a:0"));
        eprintln!(
            "RENDER_VALIDATED path={} logical_duration={expected_duration:.3}s frame_aligned_duration={expected_video_duration:.3}s actual_duration={actual_duration:.3}s regions={} video_timestamps=monotonic audio_timestamps=monotonic signatures=matched",
            rendered.display(),
            balanced.len()
        );
    }
}

#[test]
fn atempo_chain_stays_inside_ffmpeg_limits() {
    for speed in [0.05, 0.25, 0.5, 1.0, 2.0, 4.0, 16.0, 100.0] {
        for filter in atempo(speed).split(',') {
            let factor = filter.split('=').nth(1).unwrap().parse::<f64>().unwrap();
            assert!((0.5..=2.0).contains(&factor));
        }
    }
}

#[test]
fn supported_audio_formats_have_expected_extensions() {
    assert_eq!(audio_format("aac").unwrap().0, "m4a");
    assert_eq!(audio_format("mp3").unwrap().0, "mp3");
    assert_eq!(audio_format("wav").unwrap().0, "wav");
    assert_eq!(audio_format("flac").unwrap().0, "flac");
    assert_eq!(audio_format("opus").unwrap().0, "opus");
    assert!(audio_format("invalid").is_err());
}

#[test]
fn drawtext_escapes_windows_paths_and_user_text() {
    assert_eq!(
        drawtext_escape(r"C:\Windows\Fonts\impact.ttf"),
        r"C\:\\Windows\\Fonts\\impact.ttf"
    );
    assert_eq!(
        drawtext_escape("user's [text], ok"),
        r"user\'\''s\ [text],\ ok"
    );
    assert_eq!(drawtext_escape("100%"), "100%");
}

#[tokio::test]
async fn quoted_filter_values_render_literal_text_and_subtitle_paths() {
    async fn render(filter: &str) -> Vec<u8> {
        let output = hidden_command("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=640x180:r=1:d=1",
                "-vf",
                filter,
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{filter}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 640 * 180 * 3);
        output.stdout
    }
    let root = operation_temp_dir("filter-quoting-test").unwrap();
    let font = root.join("O'Neil [font], 100%.ttf");
    let system_font =
        PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
            .join("Fonts")
            .join("arialbd.ttf");
    std::fs::copy(system_font, &font).unwrap();
    for text in [
        "O'Neil",
        "user's [text], ok",
        "100% ready: %{pts}",
        r"C:\clips\O'Neil; [video], 100%",
        "  leading and trailing  ",
        "first line\nsecond's line",
    ] {
        let file = root.join("reference.txt");
        std::fs::write(&file, text).unwrap();
        let prefix = format!(
            "drawtext=fontfile='{}'",
            drawtext_escape(&font.to_string_lossy())
        );
        let suffix = ":expansion=none:fontcolor=white:fontsize=24:x=10:y=10";
        let expected = render(&format!(
            "{prefix}:textfile='{}'{suffix}",
            ffmpeg_filter_path(&file)
        ))
        .await;
        let actual = render(&format!(
            "{prefix}:text='{}'{suffix}",
            drawtext_escape(text)
        ))
        .await;
        assert!(
            expected.iter().any(|byte| *byte > 200),
            "Reference text must be visible"
        );
        assert_eq!(
            actual, expected,
            "Literal text changed during export: {text}"
        );
    }
    let subtitle = "1\n00:00:00,000 --> 00:00:01,000\nCaption\n";
    let simple = root.join("normal.srt");
    let special = root.join("Türkçe O'Neil [subs], 100%; clip.srt");
    std::fs::write(&simple, subtitle).unwrap();
    std::fs::write(&special, subtitle).unwrap();
    let expected = render(&format!(
        "subtitles=filename='{}'",
        ffmpeg_filter_path(&simple)
    ))
    .await;
    let actual = render(&format!(
        "subtitles=filename='{}'",
        ffmpeg_filter_path(&special)
    ))
    .await;
    assert_eq!(
        actual, expected,
        "Special characters in subtitle paths must not affect export"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn discord_size_plan_reserves_overhead_and_audio_budget() {
    let (target, video, audio) = discord_bitrate_plan(20.0, 60.0, 128, true).unwrap();
    assert_eq!(target, 20 * 1024 * 1024);
    assert_eq!(audio, 128_000);
    assert!(video > 2_000_000);
    let estimated = ((video + audio) as f64 / 8.0 * 60.0) as u64;
    assert!(estimated <= (target as f64 * 0.961) as u64);
    let (_, long_video, long_audio) = discord_bitrate_plan(20.0, 801.0, 128, true).unwrap();
    assert_eq!(long_audio, 128_000);
    assert!((70_000..80_000).contains(&long_video));
    assert!(discord_bitrate_plan(2.0, 3600.0, 192, true).is_err());
}

#[test]
fn discord_filter_only_downscales_and_limits_high_fps() {
    let info = MediaInfo {
        path: "portrait.mp4".into(),
        name: "portrait.mp4".into(),
        kind: "video".into(),
        duration: Some(10.0),
        width: Some(2160),
        height: Some(3840),
        fps: Some(120.0),
        codec: "h264".into(),
        audio_codec: Some("aac".into()),
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: None,
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    let filter = discord_video_filter(&info, "1080", "60", 2_000_000)
        .unwrap()
        .unwrap();
    assert!(filter.contains("scale=1080:1920"));
    assert!(filter.contains("fps=60"));
    let automatic = discord_video_filter(&info, "auto", "auto", 65_000)
        .unwrap()
        .unwrap();
    assert!(automatic.contains("scale=238:426"));
    assert!(automatic.contains("fps=15"));
}

#[test]
fn discord_smart_budget_protects_both_audio_and_picture() {
    assert_eq!(discord_audio_kbps("auto", 190_000.0, true).unwrap(), 64);
    assert_eq!(discord_audio_kbps("auto", 350_000.0, true).unwrap(), 96);
    assert_eq!(discord_audio_kbps("auto", 900_000.0, true).unwrap(), 128);
    assert_eq!(discord_audio_kbps("auto", 190_000.0, false).unwrap(), 0);
    assert_eq!(discord_auto_profile(100_000), (240, 15));
    assert_eq!(discord_auto_profile(250_000), (360, 20));
    assert_eq!(discord_auto_profile(600_000), (480, 24));
    assert_eq!(discord_auto_profile(1_000_000), (720, 30));
}

#[test]
fn discord_keeps_resolution_for_low_complexity_screen_video() {
    let screen = MediaInfo {
        path: "screen.mp4".into(),
        name: "screen.mp4".into(),
        kind: "video".into(),
        duration: Some(800.0),
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        codec: "h264".into(),
        audio_codec: Some("aac".into()),
        audio_tracks: Vec::new(),
        pixel_format: Some("yuv420p".into()),
        bits_per_raw_sample: None,
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: Some(340_000.0),
        size: 1,
        start_timecode: None,
    };
    assert!(discord_is_low_complexity(&screen));
    assert_eq!(
        discord_video_filter(&screen, "auto", "auto", 137_000).unwrap(),
        None
    );
}

#[test]
fn menu_operations_route_to_separate_categories() {
    for operation in [
        "transform",
        "text",
        "image_overlay",
        "color",
        "blur_pixelate",
        "noise",
        "screenshot",
        "image_compressor",
        "metadata_cleaner",
    ] {
        for extension in ["png", "jpg", "webp", "bmp", "tiff", "avif"] {
            assert_eq!(output_category(operation, extension), "image");
        }
    }
    assert_eq!(output_category("text", "mp4"), "text");
    assert_eq!(output_category("color", "mp4"), "effects");
    assert_eq!(output_category("extract_audio", "mp3"), "audio");
    assert_eq!(output_category("remove_audio", "mp4"), "video");
    assert_eq!(category("remove_audio"), "video");
    assert_eq!(category("interpolation"), "motion");
    assert_eq!(category("noise"), "effects");
    assert_eq!(category("extract_audio"), "audio");
    assert_eq!(category("image_ratio"), "image");
    assert_eq!(category("image_potatoify"), "image");
    assert_eq!(category("image_compressor"), "image");
    assert_eq!(category("metadata_cleaner"), "image");
    assert_eq!(category("discord_compressor"), "quality");
    assert_eq!(category("smart_quality"), "quality");
    assert_eq!(category("proxy"), "proxy");
    assert_eq!(category("upscale"), "upscale");
}

#[test]
fn every_user_tool_has_an_automated_test_path() {
    let source = include_str!("../../src/lib/tools.ts");
    let mut tool_ids = source
        .lines()
        .filter_map(|line| {
            let rest = line.get(line.find("id:")? + 3..)?.trim_start();
            let quoted = rest.strip_prefix('"')?;
            Some(quoted.split('"').next()?.to_string())
        })
        .collect::<Vec<_>>();
    tool_ids.sort();
    tool_ids.dedup();
    let tested = [
        "audio_convert",
        "audio_lab",
        "blur_pixelate",
        "clipper",
        "color",
        "compression",
        "cut",
        "discord_compressor",
        "distortion",
        "encode",
        "extract_audio",
        "file_hash",
        "fix_timestamps",
        "fps",
        "frame_blend",
        "frame_extractor",
        "gif",
        "image_compressor",
        "image_overlay",
        "image_potatoify",
        "interpolation",
        "merge_videos",
        "metadata_cleaner",
        "noise",
        "potatoify",
        "proxy",
        "remove_audio",
        "remux",
        "replace_audio",
        "screenshot",
        "speed",
        "stabilizer",
        "subtitles",
        "text",
        "transform",
        "upscale",
    ];
    assert_eq!(
        tool_ids, tested,
        "Update the real-media smoke coverage when adding a user tool."
    );
}

#[test]
fn quality_analysis_samples_video_across_its_duration() {
    assert_eq!(quality_sample_positions(1.0, 1.0), vec![0.0]);
    assert_eq!(quality_sample_positions(5.0, 2.0), vec![0.0, 3.0]);
    assert_eq!(quality_sample_positions(20.0, 2.0), vec![0.0, 9.0, 18.0]);
}

#[test]
fn quality_analysis_chooses_smallest_crf_that_meets_the_goal() {
    let candidates = vec![
        QualityCandidate {
            crf: 16,
            vmaf: 97.0,
            estimated_size_mb: 50.0,
            rating: "excellent".into(),
        },
        QualityCandidate {
            crf: 20,
            vmaf: 94.0,
            estimated_size_mb: 32.0,
            rating: "very_good".into(),
        },
        QualityCandidate {
            crf: 24,
            vmaf: 91.0,
            estimated_size_mb: 20.0,
            rating: "very_good".into(),
        },
        QualityCandidate {
            crf: 28,
            vmaf: 84.0,
            estimated_size_mb: 12.0,
            rating: "heavy_loss".into(),
        },
    ];
    assert_eq!(recommend_quality_crf(&candidates, 92.0), Some(20));
    assert_eq!(recommend_quality_crf(&candidates, 88.0), Some(24));
    assert_eq!(recommend_quality_crf(&candidates, 99.0), Some(16));
}

fn values(items: &[(&str, &str)]) -> HashMap<String, String> {
    items
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect()
}

async fn render_request(request: &OperationRequest, info: &MediaInfo) -> PathBuf {
    let (args, output) = build_command(request, info).await.unwrap();
    assert!(
        std::process::Command::new("ffmpeg")
            .args(&args)
            .status()
            .unwrap()
            .success(),
        "operation {} failed",
        request.operation
    );
    assert!(output.is_file());
    if info.kind == "image" {
        assert_eq!(
            output.parent().unwrap().file_name().unwrap(),
            "image",
            "image tool {} saved outside the image folder",
            request.operation
        );
    }
    output
}

#[tokio::test]
async fn batch_fps_lossless_setting_renders_ten_fps_and_preserves_audio() {
    let root = std::env::temp_dir().join(format!(
        "container_batch_fps_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("source.mp4");
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=160x90:r=60:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&source)
        .status()
        .unwrap()
        .success());
    let info = probe_media(source.to_string_lossy().into_owned())
        .await
        .unwrap();
    let output = render_request(
        &OperationRequest {
            input: source.to_string_lossy().into_owned(),
            operation: "fps".into(),
            params: values(&[("fps", "10"), ("crf", "0")]),
        },
        &info,
    )
    .await;
    let result = probe_media(output.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(result.fps, Some(10.0));
    assert_eq!((result.width, result.height), (info.width, info.height));
    assert_eq!(copied_audio_hash(&output), copied_audio_hash(&source));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn repeated_exports_preserve_unicode_source_and_unique_probeable_outputs() {
    let root = storage_test_root("repeated-exports");
    let source = root.join("çekim 🎬 & ' (kaynak).mp4");
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=64x48:r=30:d=0.2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p"
        ])
        .arg(&source)
        .status()
        .unwrap()
        .success());
    let original = std::fs::read(&source).unwrap();
    let info = probe_media(source.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert!(info.audio_codec.is_none());
    let mut outputs = std::collections::HashSet::new();
    for _ in 0..55 {
        let output = render_request(
            &OperationRequest {
                input: source.to_string_lossy().into_owned(),
                operation: "fps".into(),
                params: values(&[("fps", "10"), ("crf", "0")]),
            },
            &info,
        )
        .await;
        assert!(
            outputs.insert(output.clone()),
            "export overwrote a previous output"
        );
        assert!(std::fs::metadata(&output).unwrap().len() > 0);
        let rendered = probe_media(output.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(
            (rendered.width, rendered.height, rendered.fps),
            (Some(64), Some(48), Some(10.0))
        );
        assert_eq!(rendered.codec, "h264");
        assert!(rendered
            .duration
            .is_some_and(|duration| duration > 0.0 && duration < 0.5));
        assert!(rendered.audio_codec.is_none());
    }
    assert_eq!(
        std::fs::read(&source).unwrap(),
        original,
        "source was modified"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn repeated_ffmpeg_cancellation_releases_worker_and_allows_next_stage() {
    let root = storage_test_root("cancel-restart");
    let state = JobState::default();
    for delay in [0, 1000, 0] {
        state.cancelled.store(false, Ordering::Relaxed);
        let output = root.join(format!("partial-{delay}.mp4"));
        let args = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=64x48:r=30:d=2",
            "-c:v",
            "libx264",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain(std::iter::once(output.to_string_lossy().into_owned()))
        .collect();
        let started = Instant::now();
        let stage = run_ffmpeg_stage(None, &state, args, 2.0, &started, 0.0, 99.0, "test");
        let cancel = async {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    if state.pid.lock().unwrap().is_some() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            state.cancelled.store(true, Ordering::Relaxed);
            let pid = (*state.pid.lock().unwrap()).unwrap();
            tokio::task::spawn_blocking(move || stop_process_tree(pid))
                .await
                .unwrap();
        };
        let (result, ()) = tokio::join!(stage, cancel);
        assert!(result.unwrap_err().to_lowercase().contains("cancel"));
        assert!(state.pid.lock().unwrap().is_none());
        state.cancelled.store(false, Ordering::Relaxed);
        let next = root.join("after-cancel.mp4");
        let args = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=64x48:r=30:d=0.2",
            "-c:v",
            "libx264",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain(std::iter::once(next.to_string_lossy().into_owned()))
        .collect();
        run_ffmpeg_stage(
            None,
            &state,
            args,
            0.2,
            &Instant::now(),
            0.0,
            99.0,
            "restart",
        )
        .await
        .unwrap();
        assert!(probe_media(next.to_string_lossy().into_owned())
            .await
            .unwrap()
            .duration
            .is_some_and(|duration| duration > 0.0));
        assert!(state.pid.lock().unwrap().is_none());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn special_tools_render_real_media() {
    let root = std::env::temp_dir().join("container_special_tools_smoke_test");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let video = root.join("source.mp4");
    let image = root.join("source.png");
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&video)
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=120x80",
            "-frames:v",
            "1",
        ])
        .arg(&image)
        .status()
        .unwrap()
        .success());

    let digest = hash_file(image.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest
        .chars()
        .all(|character| character.is_ascii_hexdigit()));

    let state = JobState::default();
    let image_info = probe_media(image.to_string_lossy().into_owned())
        .await
        .unwrap();
    let potato = run_image_potatoify(
        None,
        &state,
        &OperationRequest {
            input: image.to_string_lossy().into_owned(),
            operation: "image_potatoify".into(),
            params: values(&[
                ("profile", "custom"),
                ("quality", "5"),
                ("times", "2"),
                ("scale", "2"),
            ]),
        },
        &image_info,
    )
    .await
    .unwrap();
    let potato_info = probe_media(potato.output.clone()).await.unwrap();
    assert_eq!(
        (potato_info.width, potato_info.height),
        (Some(60), Some(40))
    );

    let video_info = probe_media(video.to_string_lossy().into_owned())
        .await
        .unwrap();
    let discord = run_discord_compressor(
        None,
        &state,
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "discord_compressor".into(),
            params: values(&[
                ("target_mb", "2"),
                ("codec", "h264"),
                ("resolution", "source"),
                ("fps_limit", "source"),
                ("audio_kbps", "64"),
                ("preset", "veryfast"),
            ]),
        },
        &video_info,
    )
    .await
    .unwrap();
    let discord_path = PathBuf::from(&discord.output);
    assert!(discord_path.is_file());
    assert!(std::fs::metadata(&discord_path).unwrap().len() <= 2 * 1024 * 1024);

    let quality = analyze_quality_inner(
        None,
        &state,
        QualityAnalysisRequest {
            input: video.to_string_lossy().into_owned(),
            goal: "balanced".into(),
            sample_duration: 1.0,
        },
    )
    .await
    .unwrap();
    assert_eq!(quality.candidates.len(), 4);
    assert!(quality
        .candidates
        .iter()
        .all(|candidate| candidate.vmaf.is_finite()));

    let smartcut = export_autocut_inner(
        None,
        &state,
        AutoCutExportRequest {
            input: video.to_string_lossy().into_owned(),
            cuts: vec![
                KeepInterval {
                    start: 0.0,
                    end: 0.35,
                    enabled: true,
                },
                KeepInterval {
                    start: 0.55,
                    end: 0.9,
                    enabled: true,
                },
            ],
            format: "mp4".into(),
            quality: "medium".into(),
            resolution: "source".into(),
            linked_tracks: Vec::new(),
        },
    )
    .await
    .unwrap();
    let smartcut_info = probe_media(smartcut.output.clone()).await.unwrap();
    assert_eq!(smartcut_info.kind, "video");
    assert!(smartcut_info
        .duration
        .is_some_and(|duration| duration > 0.5 && duration < 0.9));

    let fcpxml = export_autocut_inner(
        None,
        &state,
        AutoCutExportRequest {
            input: video.to_string_lossy().into_owned(),
            cuts: vec![KeepInterval {
                start: 0.1,
                end: 0.8,
                enabled: true,
            }],
            format: "fcpxml".into(),
            quality: "medium".into(),
            resolution: "source".into(),
            linked_tracks: Vec::new(),
        },
    )
    .await
    .unwrap();
    let xml = std::fs::read_to_string(fcpxml.output).unwrap();
    assert!(xml.contains("CONTAINER SmartCut"));
    assert!(xml.contains("duration=\"0.700000s\""));

    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn new_video_tools_render_real_media_and_preserve_streams() {
    let root = std::env::temp_dir().join("container_yeni_araclar_ğüşi");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let video = root.join("kaynak video.mp4");
    let different = root.join("farklı.mp4");
    let rotated = root.join("döndürülmüş.mp4");
    let short_audio = root.join("kısa sesli video.mp4");
    let opus_video = root.join("opus sesli video.mkv");
    let logo = root.join("şeffaf logo.png");
    let subtitle = root.join("Türkçe altyazı.srt");
    let make_video = |path: &Path, source: &str, with_audio: bool| {
        let mut command = std::process::Command::new("ffmpeg");
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            source,
        ]);
        if with_audio {
            command.args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1",
                "-shortest",
                "-c:a",
                "aac",
            ]);
        }
        command
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(path)
            .status()
            .unwrap()
            .success()
    };
    assert!(make_video(&video, "testsrc2=s=320x180:r=25:d=1", true));
    assert!(make_video(&different, "testsrc2=s=160x120:r=30:d=1", false));
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=0.4",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&short_audio)
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "libopus",
        ])
        .arg(&opus_video)
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-display_rotation",
            "90",
            "-i",
        ])
        .arg(&video)
        .args(["-c", "copy"])
        .arg(&rotated)
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=red@0.6:s=64x32,format=rgba",
            "-frames:v",
            "1"
        ])
        .arg(&logo)
        .status()
        .unwrap()
        .success());
    std::fs::write(
        &subtitle,
        "1\n00:00:00,100 --> 00:00:00,800\nMerhaba dünya\n",
    )
    .unwrap();
    let info = probe_media(video.to_string_lossy().into_owned())
        .await
        .unwrap();
    let source_audio_hash = copied_audio_hash(&video);
    let rotated_info = probe_media(rotated.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(
        display_dimensions(&rotated, &rotated_info).await.unwrap(),
        (180, 320)
    );
    let rotated_proxy = render_request(
        &OperationRequest {
            input: rotated.to_string_lossy().into_owned(),
            operation: "proxy".into(),
            params: values(&[("resolution", "auto"), ("quality", "compact")]),
        },
        &rotated_info,
    )
    .await;
    let rotated_proxy_info = probe_media(rotated_proxy.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(
        display_dimensions(&rotated_proxy, &rotated_proxy_info)
            .await
            .unwrap(),
        (180, 320)
    );
    assert_eq!(copied_audio_hash(&rotated_proxy), source_audio_hash);
    let rotated_gif = render_request(
        &OperationRequest {
            input: rotated.to_string_lossy().into_owned(),
            operation: "gif".into(),
            params: values(&[
                ("start", "0"),
                ("end", "0.5"),
                ("height", "160"),
                ("fps", "10"),
                ("max_colors", "64"),
                ("palette_mode", "auto"),
                ("dither", "balanced"),
                ("transparency", "off"),
                ("loop", "0"),
            ]),
        },
        &rotated_info,
    )
    .await;
    let rotated_gif_info = probe_media(rotated_gif.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(
        (rotated_gif_info.width, rotated_gif_info.height),
        (Some(90), Some(160))
    );
    let overlay = render_request(
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "image_overlay".into(),
            params: values(&[
                ("image_path", &logo.to_string_lossy()),
                ("x", "80"),
                ("y", "80"),
                ("size", "20"),
                ("opacity", "60"),
                ("start", "0.1"),
                ("end", "0.8"),
            ]),
        },
        &info,
    )
    .await;
    assert!(probe_media(overlay.to_string_lossy().into_owned())
        .await
        .unwrap()
        .audio_codec
        .is_some());
    assert_eq!(copied_audio_hash(&overlay), source_audio_hash);
    let short_audio_info = probe_media(short_audio.to_string_lossy().into_owned())
        .await
        .unwrap();
    let overlay_with_short_audio = render_request(
        &OperationRequest {
            input: short_audio.to_string_lossy().into_owned(),
            operation: "image_overlay".into(),
            params: values(&[
                ("image_path", &logo.to_string_lossy()),
                ("x", "50"),
                ("y", "50"),
                ("size", "20"),
                ("opacity", "60"),
                ("start", "0"),
                ("end", "1.5"),
            ]),
        },
        &short_audio_info,
    )
    .await;
    assert!(
        probe_media(overlay_with_short_audio.to_string_lossy().into_owned())
            .await
            .unwrap()
            .duration
            .is_some_and(|duration| duration > 1.8)
    );
    assert_eq!(
        copied_audio_hash(&overlay_with_short_audio),
        copied_audio_hash(&short_audio)
    );
    let blurred = render_request(
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "blur_pixelate".into(),
            params: values(&[
                ("effect", "blur"),
                ("strength", "20"),
                ("region_x", "20"),
                ("region_y", "20"),
                ("region_w", "40"),
                ("region_h", "40"),
            ]),
        },
        &info,
    )
    .await;
    assert!(probe_media(blurred.to_string_lossy().into_owned())
        .await
        .unwrap()
        .audio_codec
        .is_some());
    assert_eq!(copied_audio_hash(&blurred), source_audio_hash);
    let pixelated = render_request(
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "blur_pixelate".into(),
            params: values(&[
                ("effect", "pixelate"),
                ("strength", "20"),
                ("region_x", "17"),
                ("region_y", "19"),
                ("region_w", "41"),
                ("region_h", "43"),
            ]),
        },
        &info,
    )
    .await;
    assert!(probe_media(pixelated.to_string_lossy().into_owned())
        .await
        .unwrap()
        .audio_codec
        .is_some());
    assert_eq!(copied_audio_hash(&pixelated), source_audio_hash);
    let rotated_blur = render_request(
        &OperationRequest {
            input: rotated.to_string_lossy().into_owned(),
            operation: "blur_pixelate".into(),
            params: values(&[
                ("effect", "blur"),
                ("strength", "20"),
                ("region_x", "10"),
                ("region_y", "15"),
                ("region_w", "30"),
                ("region_h", "35"),
            ]),
        },
        &rotated_info,
    )
    .await;
    let rotated_blur_info = probe_media(rotated_blur.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(
        display_dimensions(&rotated_blur, &rotated_blur_info)
            .await
            .unwrap(),
        (180, 320)
    );
    assert_eq!(copied_audio_hash(&rotated_blur), source_audio_hash);
    let opus_info = probe_media(opus_video.to_string_lossy().into_owned())
        .await
        .unwrap();
    let opus_blurred = render_request(
        &OperationRequest {
            input: opus_video.to_string_lossy().into_owned(),
            operation: "blur_pixelate".into(),
            params: values(&[
                ("effect", "blur"),
                ("strength", "20"),
                ("region_x", "20"),
                ("region_y", "20"),
                ("region_w", "40"),
                ("region_h", "40"),
            ]),
        },
        &opus_info,
    )
    .await;
    assert_eq!(
        probe_media(opus_blurred.to_string_lossy().into_owned())
            .await
            .unwrap()
            .audio_codec
            .as_deref(),
        Some("aac")
    );
    let added = render_request(
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "subtitles".into(),
            params: values(&[
                ("action", "add"),
                ("subtitle_path", &subtitle.to_string_lossy()),
                ("container", "mkv"),
                ("subtitle_track", ""),
            ]),
        },
        &info,
    )
    .await;
    let tracks = probe_subtitles(added.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(tracks.len(), 1);
    let added_info = probe_media(added.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(copied_audio_hash(&added), source_audio_hash);
    let extracted = render_request(
        &OperationRequest {
            input: added.to_string_lossy().into_owned(),
            operation: "subtitles".into(),
            params: values(&[
                ("action", "extract"),
                ("subtitle_path", ""),
                ("container", "mkv"),
                ("subtitle_track", &tracks[0].index.to_string()),
            ]),
        },
        &added_info,
    )
    .await;
    assert_eq!(
        extracted.extension().and_then(|value| value.to_str()),
        Some("srt")
    );
    let removed = render_request(
        &OperationRequest {
            input: added.to_string_lossy().into_owned(),
            operation: "subtitles".into(),
            params: values(&[
                ("action", "remove"),
                ("subtitle_path", ""),
                ("container", "mkv"),
                ("subtitle_track", ""),
            ]),
        },
        &added_info,
    )
    .await;
    assert!(probe_subtitles(removed.to_string_lossy().into_owned())
        .await
        .unwrap()
        .is_empty());
    assert_eq!(copied_audio_hash(&removed), source_audio_hash);
    let burned = render_request(
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "subtitles".into(),
            params: values(&[
                ("action", "burn"),
                ("subtitle_path", &subtitle.to_string_lossy()),
                ("container", "mkv"),
                ("subtitle_track", ""),
            ]),
        },
        &info,
    )
    .await;
    assert!(probe_media(burned.to_string_lossy().into_owned())
        .await
        .unwrap()
        .audio_codec
        .is_some());
    assert_eq!(copied_audio_hash(&burned), source_audio_hash);
    let state = JobState::default();
    let stabilized = run_stabilizer(
        None,
        &state,
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "stabilizer".into(),
            params: values(&[("strength", "light")]),
        },
        &info,
    )
    .await
    .unwrap();
    assert!(Path::new(&stabilized.output).is_file());
    assert_eq!(
        copied_audio_hash(Path::new(&stabilized.output)),
        source_audio_hash
    );
    let temp_stabilizers = || {
        std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("container_stabilizer_"))
            })
            .collect::<HashSet<_>>()
    };
    let before_cancel = temp_stabilizers();
    let cancelled_state = JobState::default();
    cancelled_state.cancelled.store(true, Ordering::Relaxed);
    let cancel_error = run_stabilizer(
        None,
        &cancelled_state,
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "stabilizer".into(),
            params: values(&[("strength", "strong")]),
        },
        &info,
    )
    .await
    .unwrap_err();
    assert!(cancel_error.to_ascii_lowercase().contains("cancel"));
    assert_eq!(temp_stabilizers(), before_cancel);
    let compatible_inputs = serde_json::to_string(&vec![
        video.to_string_lossy().to_string(),
        video.to_string_lossy().to_string(),
    ])
    .unwrap();
    let merged = run_merge_videos(
        None,
        &state,
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "merge_videos".into(),
            params: values(&[("mode", "auto"), ("inputs", &compatible_inputs)]),
        },
    )
    .await
    .unwrap();
    let merged_path = PathBuf::from(&merged.output);
    assert!(probe_media(merged.output)
        .await
        .unwrap()
        .duration
        .is_some_and(|duration| duration > 1.8));
    assert!(packet_timestamps_are_monotonic(&merged_path, "v:0"));
    assert!(packet_timestamps_are_monotonic(&merged_path, "a:0"));
    let mixed_inputs = serde_json::to_string(&vec![
        video.to_string_lossy().to_string(),
        different.to_string_lossy().to_string(),
    ])
    .unwrap();
    let lossless_error = run_merge_videos(
        None,
        &state,
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "merge_videos".into(),
            params: values(&[("mode", "lossless"), ("inputs", &mixed_inputs)]),
        },
    )
    .await
    .unwrap_err();
    assert!(lossless_error.contains("cannot be merged losslessly"));
    let normalized = run_merge_videos(
        None,
        &state,
        &OperationRequest {
            input: video.to_string_lossy().into_owned(),
            operation: "merge_videos".into(),
            params: values(&[
                ("mode", "auto"),
                ("container", "mp4"),
                ("inputs", &mixed_inputs),
            ]),
        },
    )
    .await
    .unwrap();
    let normalized_info = probe_media(normalized.output).await.unwrap();
    assert_eq!(
        Path::new(&normalized_info.path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("mp4")
    );
    assert_eq!(
        (normalized_info.width, normalized_info.height),
        (Some(160), Some(120))
    );
    assert!(normalized_info.audio_codec.is_some());
    assert!(packet_timestamps_are_monotonic(
        Path::new(&normalized_info.path),
        "v:0"
    ));
    assert!(packet_timestamps_are_monotonic(
        Path::new(&normalized_info.path),
        "a:0"
    ));
    std::fs::remove_dir_all(&root).unwrap();
}

fn sample_video_rgb(path: &Path, at: f64) -> [u8; 3] {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-ss"])
        .arg(format!("{at:.3}"))
        .arg("-i")
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=1:1,format=rgb24",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    [output.stdout[0], output.stdout[1], output.stdout[2]]
}

fn copied_audio_hash(path: &Path) -> String {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-c:a",
            "copy",
            "-f",
            "streamhash",
            "-hash",
            "sha256",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn copied_video_hash(path: &Path) -> String {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-c:v",
            "copy",
            "-f",
            "streamhash",
            "-hash",
            "sha256",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn sample_video_signature(path: &Path, at: f64) -> Vec<u8> {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-ss"])
        .arg(format!("{at:.6}"))
        .arg("-i")
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=16:9,format=rgb24",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

fn packet_timestamps_are_monotonic(path: &Path, stream: &str) -> bool {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            stream,
            "-show_frames",
            "-show_entries",
            "frame=best_effort_timestamp_time",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .unwrap();
    if !output.status.success() {
        return false;
    }
    let mut previous = f64::NEG_INFINITY;
    for current in String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<f64>().ok())
    {
        if current + 1e-9 < previous {
            return false;
        }
        previous = current;
    }
    previous.is_finite()
}

fn sample_audio_frequency(path: &Path, at: f64) -> f64 {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-ss"])
        .arg(format!("{at:.3}"))
        .arg("-i")
        .arg(path)
        .args([
            "-t", "0.35", "-vn", "-ac", "1", "-ar", "16000", "-f", "s16le", "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let samples = output
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let crossings = samples
        .windows(2)
        .filter(|pair| (pair[0] < 0 && pair[1] >= 0) || (pair[0] >= 0 && pair[1] < 0))
        .count();
    crossings as f64 * 16_000.0 / (2.0 * samples.len() as f64)
}

#[tokio::test]
async fn smartcut_export_never_replays_a_source_range() {
    let root = std::env::temp_dir().join("container_smartcut_replay_regression");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("abc-source.mp4");
    let video_source = "color=c=black:s=160x90:r=30:d=6,drawbox=x=0:y=0:w=iw:h=ih:color=red:t=fill:enable='between(t,0,1)',drawbox=x=0:y=0:w=iw:h=ih:color=green:t=fill:enable='between(t,2,3)',drawbox=x=0:y=0:w=iw:h=ih:color=blue:t=fill:enable='between(t,4,5)'";
    let audio_source = "aevalsrc=exprs='if(between(t,0,1),0.5*sin(2*PI*440*t),if(between(t,2,3),0.5*sin(2*PI*660*t),if(between(t,4,5),0.5*sin(2*PI*880*t),0)))':s=48000:d=6";
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg(video_source)
        .args(["-f", "lavfi", "-i"])
        .arg(audio_source)
        .args([
            "-c:v",
            "libx264",
            "-profile:v",
            "main",
            "-g",
            "180",
            "-keyint_min",
            "180",
            "-sc_threshold",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());

    let cuts = vec![
        KeepInterval {
            start: 0.0,
            end: 1.0,
            enabled: true,
        },
        KeepInterval {
            start: 2.0,
            end: 3.0,
            enabled: true,
        },
        KeepInterval {
            start: 4.0,
            end: 5.0,
            enabled: true,
        },
    ];
    // Reproduce the old exporter with two ranges that overlap by 200 ms.
    // The old `enabled_cuts` passed both ranges through and concat appended
    // the shared source time twice, producing a 2.2 s file from a 2.0 s
    // source-time union.
    let legacy_list = root.join("legacy.ffconcat");
    let escaped = source.to_string_lossy().replace('\'', "'\\''");
    std::fs::write(
        &legacy_list,
        format!(
            "ffconcat version 1.0\nfile '{escaped}'\ninpoint 0\noutpoint 1.2\nfile '{escaped}'\ninpoint 1\noutpoint 2\n"
        ),
    )
    .unwrap();
    let legacy = root.join("legacy-export.mp4");
    let legacy_status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
        ])
        .arg(&legacy_list)
        .args(["-c:v", "libx264", "-c:a", "aac"])
        .arg(&legacy)
        .status()
        .unwrap();
    assert!(legacy_status.success());
    let legacy_info = probe_media(legacy.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert!(legacy_info.duration.is_some_and(|duration| duration > 2.15));

    let rendered = export_autocut_inner(
        None,
        &JobState::default(),
        AutoCutExportRequest {
            input: source.to_string_lossy().into_owned(),
            cuts,
            format: "mp4".into(),
            quality: "medium".into(),
            resolution: "source".into(),
            linked_tracks: Vec::new(),
        },
    )
    .await
    .unwrap();
    let rendered = PathBuf::from(rendered.output);
    let colors = [
        sample_video_rgb(&rendered, 0.5),
        sample_video_rgb(&rendered, 1.5),
        sample_video_rgb(&rendered, 2.5),
    ];
    assert!(colors[0][0] > colors[0][1] * 2 && colors[0][0] > colors[0][2] * 2);
    assert!(colors[1][1] > colors[1][0] * 2 && colors[1][1] > colors[1][2] * 2);
    assert!(colors[2][2] > colors[2][0] * 2 && colors[2][2] > colors[2][1] * 2);
    let frequencies = [
        sample_audio_frequency(&rendered, 0.3),
        sample_audio_frequency(&rendered, 1.3),
        sample_audio_frequency(&rendered, 2.3),
    ];
    for (actual, expected) in frequencies.into_iter().zip([440.0, 660.0, 880.0]) {
        assert!(
            (actual - expected).abs() < 35.0,
            "actual={actual} expected={expected}"
        );
    }
    let rendered_info = probe_media(rendered.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert!(rendered_info
        .duration
        .is_some_and(|duration| (duration - 3.0).abs() < 0.08));
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn transform_rotation_and_flip_filters_follow_visible_axes() {
    let root = std::env::temp_dir().join("container_transform_matrix_test");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("matrix.png");
    let info = MediaInfo {
        path: input.to_string_lossy().into_owned(),
        name: "matrix.png".into(),
        kind: "image".into(),
        duration: None,
        width: Some(320),
        height: Some(180),
        fps: None,
        codec: "png".into(),
        audio_codec: None,
        audio_tracks: Vec::new(),
        pixel_format: Some("rgba".into()),
        bits_per_raw_sample: Some(8),
        color_transfer: None,
        color_primaries: None,
        color_space: None,
        bitrate: None,
        size: 1,
        start_timecode: None,
    };
    for (rotation, flip_h, flip_v, expected) in [
        ("0", "false", "false", None),
        ("0", "true", "true", Some("hflip,vflip")),
        ("90", "true", "false", Some("transpose=clock,hflip")),
        ("180", "false", "false", Some("hflip,vflip")),
        ("270", "false", "true", Some("transpose=cclock,vflip")),
    ] {
        let request = OperationRequest {
            input: input.to_string_lossy().into_owned(),
            operation: "transform".into(),
            params: values(&[
                ("crop_mode", "off"),
                ("crop_x", "0"),
                ("crop_y", "0"),
                ("crop_w", "100"),
                ("crop_h", "100"),
                ("rotate", rotation),
                ("flip_h", flip_h),
                ("flip_v", flip_v),
                ("size_mode", "source"),
                ("size", "180"),
                ("output_width", "320"),
                ("output_height", "180"),
                ("format", "png"),
            ]),
        };
        let (args, _) = build_command(&request, &info).await.unwrap();
        let filter = args
            .iter()
            .position(|arg| arg == "-vf")
            .map(|index| args[index + 1].as_str());
        assert_eq!(
            filter, expected,
            "rotation={rotation}, h={flip_h}, v={flip_v}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn generated_ffmpeg_commands_render_real_test_media() {
    let root = std::env::temp_dir().join("container_studio_smoke_test");
    assert!(root.starts_with(std::env::temp_dir()));
    assert_eq!(
        root.file_name().and_then(|name| name.to_str()),
        Some("container_studio_smoke_test")
    );
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let video = root.join("source.mp4");
    let audio = root.join("source.wav");
    let fixture = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=160x90:r=30:d=0.6",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=0.6",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=660:sample_rate=48000:duration=0.6",
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-map",
            "2:a:0",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&video)
        .status()
        .unwrap();
    assert!(fixture.success());
    let audio_fixture = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:sample_rate=48000:duration=0.6",
        ])
        .arg(&audio)
        .status()
        .unwrap();
    assert!(audio_fixture.success());
    let video_info = probe_media(video.to_string_lossy().to_string())
        .await
        .unwrap();
    let source_audio_hash = copied_audio_hash(&video);
    let source_video_hash = copied_video_hash(&video);
    let audio_text = audio.to_string_lossy().to_string();

    let first_stack_step = OperationRequest {
        input: video.to_string_lossy().into_owned(),
        operation: "color".into(),
        params: values(&[
            ("brightness", "5"),
            ("brightness_enabled", "true"),
            ("denoise", "off"),
        ]),
    };
    let (stage_args, _) = build_command(&first_stack_step, &video_info).await.unwrap();
    let stage_path = root.join("stack-stage.mkv");
    let stage_args = processing_stack::output_args(stage_args, &stage_path, true, None).unwrap();
    assert!(
        std::process::Command::new("ffmpeg")
            .args(&stage_args)
            .status()
            .unwrap()
            .success(),
        "Stack lossless intermediate failed"
    );
    let stage_info = probe_media(stage_path.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(stage_info.width, video_info.width);
    let second_stack_step = OperationRequest {
        input: stage_path.to_string_lossy().into_owned(),
        operation: "color".into(),
        params: values(&[
            ("contrast", "110"),
            ("contrast_enabled", "true"),
            ("denoise", "off"),
        ]),
    };
    let (final_args, _) = build_command(&second_stack_step, &stage_info)
        .await
        .unwrap();
    let final_path = unique_output(&video, "processing_stack", "mp4").unwrap();
    let final_args =
        processing_stack::output_args(final_args, &final_path, false, Some("high")).unwrap();
    assert!(
        std::process::Command::new("ffmpeg")
            .args(&final_args)
            .status()
            .unwrap()
            .success(),
        "Stack final encode failed"
    );
    let final_info = probe_media(final_path.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(final_info.kind, "video");
    assert!(!final_info.audio_tracks.is_empty());
    assert_eq!(final_info.width, video_info.width);
    for (mode, filter) in [
        (
            "burst",
            "select=isnan(prev_selected_t)+gte(t-prev_selected_t\\,0.099999)",
        ),
        ("seconds", "fps=5"),
        ("frames", "select=not(mod(n\\,3))"),
        ("even", "fps=5"),
        ("scene", "select=gt(scene\\,0.01)"),
    ] {
        let frames = root.join(format!("frames-{mode}"));
        std::fs::create_dir(&frames).unwrap();
        let destination = frames.join("frame-%06d.png");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-ss",
                "0",
                "-i",
                video.to_str().unwrap(),
                "-t",
                "0.6",
                "-vf",
                filter,
                "-fps_mode",
                "vfr",
                "-frames:v",
                "1000",
            ])
            .arg(&destination)
            .status()
            .unwrap();
        assert!(status.success(), "Frame Extractor {mode} failed");
        let count = std::fs::read_dir(&frames).unwrap().count();
        if mode != "scene" {
            assert!(count > 0, "Frame Extractor {mode} produced no frames")
        }
        if mode == "burst" {
            assert_eq!(
                count, 6,
                "100ms spacing over 600ms must extract six distinct frames"
            );
        }
    }
    let sheet = root.join("contact-sheet.png");
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            "0",
            "-i",
            video.to_str().unwrap(),
            "-t",
            "0.6",
            "-vf",
            "fps=5,scale=320:-2,tile=3x1",
            "-fps_mode",
            "vfr",
            "-frames:v",
            "1"
        ])
        .arg(&sheet)
        .status()
        .unwrap()
        .success());
    assert_eq!(
        probe_media(sheet.to_string_lossy().into_owned())
            .await
            .unwrap()
            .kind,
        "image"
    );

    let operations: Vec<(&str, HashMap<String, String>)> = vec![
        (
            "transform",
            values(&[
                ("crop_mode", "free"),
                ("crop_x", "10"),
                ("crop_y", "10"),
                ("crop_w", "80"),
                ("crop_h", "80"),
                ("rotate", "90"),
                ("flip_h", "true"),
                ("flip_v", "false"),
                ("size_mode", "height"),
                ("size", "180"),
                ("output_width", "320"),
                ("output_height", "180"),
            ]),
        ),
        ("upscale", values(&[("target_edge", "720")])),
        ("ratio", values(&[("ratio", "1:1")])),
        (
            "resize",
            values(&[("axis", "height"), ("size", "180"), ("crf", "20")]),
        ),
        ("fps", values(&[("fps", "24"), ("crf", "20")])),
        ("interpolation", values(&[("fps", "60")])),
        ("frame_blend", values(&[("fps", "24")])),
        ("speed", values(&[("speed", "2"), ("crf", "20")])),
        (
            "speed",
            values(&[
                ("speed", "1.5"),
                ("speed_mode", "lossless_video"),
                ("crf", "20"),
            ]),
        ),
        (
            "compression",
            values(&[("crf", "22"), ("preset", "ultrafast")]),
        ),
        (
            "compression",
            values(&[("mode", "bitrate"), ("mbps", "0.3")]),
        ),
        (
            "potatoify",
            values(&[
                ("profile", "custom"),
                ("fps", "12"),
                ("video_badness", "5"),
                ("audio_badness", "5"),
                ("shrink", "2"),
            ]),
        ),
        ("potatoify", values(&[("profile", "decent")])),
        (
            "text",
            values(&[(
                "layers",
                r##"[{"text":"CONTAINER","x":50,"y":50,"size":24,"color":"#ffffff","opacity":80,"outline":2,"outline_color":"#000000","shadow":3,"shadow_color":"#000000","background":true,"background_color":"#102030","background_opacity":55,"background_padding":8}]"##,
            )]),
        ),
        (
            "color",
            values(&[
                ("brightness", "2"),
                ("brightness_enabled", "true"),
                ("contrast", "108"),
                ("contrast_enabled", "true"),
                ("saturation", "110"),
                ("saturation_enabled", "true"),
                ("gamma", "100"),
                ("gamma_enabled", "false"),
                ("hue", "0"),
                ("hue_enabled", "false"),
                ("temperature", "6500"),
                ("temperature_enabled", "false"),
                ("sharpen", "25"),
                ("sharpen_enabled", "false"),
                ("blur", "20"),
                ("blur_enabled", "false"),
                ("denoise", "off"),
                ("deband", "25"),
                ("deband_enabled", "false"),
                ("vignette", "35"),
                ("vignette_enabled", "false"),
                ("grayscale", "off"),
                ("deinterlace", "off"),
            ]),
        ),
        ("noise", values(&[("amount", "3")])),
        (
            "audio_lab",
            values(&[
                ("preset", "youtube"),
                ("lufs", "-14"),
                ("true_peak", "-1"),
                ("noise_reduction", "off"),
                ("voice_enhance", "off"),
                ("compressor", "off"),
                ("fade_in", "0"),
                ("fade_out", "0"),
                ("channels", "source"),
                ("format", "flac"),
            ]),
        ),
        ("negate", values(&[])),
        ("encode", values(&[("encoder", "libx264"), ("crf", "20")])),
        (
            "encode",
            values(&[
                ("encoder", "libx264"),
                ("crf", "22"),
                ("pixel_format", "yuv420p"),
                ("audio_mode", "merge"),
            ]),
        ),
        (
            "proxy",
            values(&[("resolution", "auto"), ("quality", "compact")]),
        ),
        ("fix_timestamps", values(&[("method", "fast")])),
        ("fix_timestamps", values(&[("method", "deep")])),
        ("cut", values(&[("start", "0.1"), ("end", "0.4")])),
        (
            "cut",
            values(&[("start", "0.1"), ("end", "0.4"), ("cut_mode", "exact")]),
        ),
        (
            "cut",
            values(&[
                ("start", "0.1"),
                ("end", "0.4"),
                ("cut_mode", "lossless"),
                ("audio_mode", "all"),
            ]),
        ),
        ("remux", values(&[("format", "mkv")])),
        (
            "screenshot",
            values(&[("timestamp", "0.1"), ("format", "png")]),
        ),
        (
            "gif",
            values(&[
                ("start", "0"),
                ("end", "0.3"),
                ("height", "90"),
                ("fps", "10"),
                ("max_colors", "64"),
                ("palette_mode", "multi"),
                ("dither", "small"),
                ("transparency", "off"),
                ("loop", "-1"),
            ]),
        ),
        ("cfr", values(&[("fps", "30")])),
        ("remove_audio", values(&[])),
        ("extract_audio", values(&[("format", "mp3")])),
        (
            "extract_audio",
            values(&[("format", "copy"), ("audio_mode", "all")]),
        ),
        (
            "replace_audio",
            values(&[("audio_path", audio_text.as_str())]),
        ),
        ("distortion", values(&[("level", "1")])),
    ];

    for (operation, params) in operations {
        let cut_mode = params
            .get("cut_mode")
            .map(String::as_str)
            .unwrap_or("lossless");
        let lossless_cut = operation == "cut" && matches!(cut_mode, "lossless" | "smart");
        let exact_cut = operation == "cut" && cut_mode == "exact";
        let request = OperationRequest {
            input: video.to_string_lossy().to_string(),
            operation: operation.into(),
            params,
        };
        let (args, output) = build_command(&request, &video_info)
            .await
            .unwrap_or_else(|error| panic!("{operation}: {error}"));
        if lossless_cut {
            assert!(args.windows(2).any(|pair| pair == ["-c:v", "copy"]));
            assert!(!args.iter().any(|arg| arg == "libx264"));
            let seek = args.iter().position(|arg| arg == "-ss").unwrap();
            let input = args.iter().position(|arg| arg == "-i").unwrap();
            assert!(
                seek < input,
                "fast input seeking must happen before opening the media"
            );
        }
        if exact_cut {
            assert!(args.windows(2).any(|pair| pair == ["-c:v", "libx264"]));
            let seek = args.iter().position(|arg| arg == "-ss").unwrap();
            let input = args.iter().position(|arg| arg == "-i").unwrap();
            assert!(
                input < seek,
                "frame-accurate output seeking must happen after opening the media"
            );
        }
        let status = std::process::Command::new("ffmpeg")
            .args(&args)
            .status()
            .unwrap();
        assert!(status.success(), "operation failed: {operation}");
        assert!(output.is_file(), "output missing: {operation}");
        if operation == "audio_lab" {
            assert_eq!(
                copied_video_hash(&output),
                source_video_hash,
                "Audio Lab changed the video stream"
            );
        }
        assert!(
            std::fs::metadata(&output).unwrap().len() > 0,
            "empty output: {operation}"
        );
        if [
            "transform",
            "upscale",
            "ratio",
            "resize",
            "fps",
            "cfr",
            "interpolation",
            "frame_blend",
            "text",
            "color",
            "noise",
            "negate",
            "proxy",
        ]
        .contains(&operation)
        {
            assert_eq!(
                copied_audio_hash(&output),
                source_audio_hash,
                "audio was re-encoded by video-only operation {operation}"
            );
        }
        probe_media(output.to_string_lossy().into_owned())
            .await
            .unwrap_or_else(|error| panic!("unreadable output for {operation}: {error}"));
    }

    let audio_info = probe_media(audio.to_string_lossy().to_string())
        .await
        .unwrap();
    let request = OperationRequest {
        input: audio.to_string_lossy().to_string(),
        operation: "audio_convert".into(),
        params: values(&[("format", "flac")]),
    };
    let (args, output) = build_command(&request, &audio_info).await.unwrap();
    assert!(std::process::Command::new("ffmpeg")
        .args(&args)
        .status()
        .unwrap()
        .success());
    assert!(output.is_file());
    assert_eq!(
        probe_media(output.to_string_lossy().into_owned())
            .await
            .unwrap()
            .kind,
        "audio"
    );
    let audio_lab_request = OperationRequest {
        input: audio.to_string_lossy().into_owned(),
        operation: "audio_lab".into(),
        params: values(&[
            ("preset", "podcast"),
            ("lufs", "-16"),
            ("true_peak", "-1"),
            ("noise_reduction", "off"),
            ("voice_enhance", "off"),
            ("compressor", "off"),
            ("fade_in", "0"),
            ("fade_out", "0"),
            ("channels", "mono"),
            ("format", "flac"),
        ]),
    };
    let (lab_args, lab_output) = build_command(&audio_lab_request, &audio_info)
        .await
        .unwrap();
    assert!(
        std::process::Command::new("ffmpeg")
            .args(&lab_args)
            .status()
            .unwrap()
            .success(),
        "Audio Lab encode failed"
    );
    let lab_info = probe_media(lab_output.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(lab_info.kind, "audio");
    assert_eq!(lab_info.audio_tracks[0].channels, Some(1));

    let image = root.join("source.png");
    assert!(std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=120x80",
            "-frames:v",
            "1",
        ])
        .arg(&image)
        .status()
        .unwrap()
        .success());
    let image_info = probe_media(image.to_string_lossy().to_string())
        .await
        .unwrap();
    for (ratio, format) in [("4:5", "png"), ("191:100", "jpg")] {
        let request = OperationRequest {
            input: image.to_string_lossy().to_string(),
            operation: "image_ratio".into(),
            params: values(&[("ratio", ratio), ("format", format)]),
        };
        let (args, output) = build_command(&request, &image_info).await.unwrap();
        assert!(std::process::Command::new("ffmpeg")
            .args(&args)
            .status()
            .unwrap()
            .success());
        assert!(output.is_file());
        assert_eq!(
            probe_media(output.to_string_lossy().into_owned())
                .await
                .unwrap()
                .kind,
            "image"
        );
    }
    for format in ["png", "webp"] {
        let request = OperationRequest {
            input: image.to_string_lossy().to_string(),
            operation: "transform".into(),
            params: values(&[
                ("crop_mode", "free"),
                ("crop_x", "10"),
                ("crop_y", "10"),
                ("crop_w", "80"),
                ("crop_h", "80"),
                ("rotate", "90"),
                ("flip_h", "true"),
                ("flip_v", "false"),
                ("size_mode", "exact"),
                ("size", "100"),
                ("output_width", "80"),
                ("output_height", "100"),
                ("format", format),
            ]),
        };
        let (args, output) = build_command(&request, &image_info).await.unwrap();
        assert!(std::process::Command::new("ffmpeg")
            .args(&args)
            .status()
            .unwrap()
            .success());
        assert!(output.is_file());
        assert_eq!(
            probe_media(output.to_string_lossy().into_owned())
                .await
                .unwrap()
                .kind,
            "image"
        );
    }

    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn smart_cut_is_precise_near_lossless_and_decoder_safe() {
    let root = std::env::temp_dir().join("container_smart_cut_test");
    assert!(root.starts_with(std::env::temp_dir()));
    assert_eq!(
        root.file_name().and_then(|name| name.to_str()),
        Some("container_smart_cut_test")
    );
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("source.mkv");
    let fixture = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=30:d=12",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=12",
            "-shortest",
            "-c:v",
            "libx264",
            "-g",
            "60",
            "-keyint_min",
            "60",
            "-sc_threshold",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "libopus",
        ])
        .arg(&source)
        .status()
        .unwrap();
    assert!(fixture.success());
    let info = probe_media(source.to_string_lossy().to_string())
        .await
        .unwrap();
    let request = OperationRequest {
        input: source.to_string_lossy().to_string(),
        operation: "cut".into(),
        params: values(&[
            ("start", "6.7"),
            ("end", "10.7"),
            ("cut_mode", "smart"),
            ("audio_mode", "main"),
        ]),
    };
    let state = JobState::default();
    let result = run_smart_cut(None, &state, &request, &info).await.unwrap();
    let output = PathBuf::from(&result.output);
    assert_eq!(
        output.extension().and_then(|value| value.to_str()),
        Some("mkv")
    );
    let output_info = probe_media(result.output).await.unwrap();
    assert_eq!(output_info.audio_codec.as_deref(), Some("opus"));
    assert!(output_info
        .duration
        .is_some_and(|value| (3.8..=4.2).contains(&value)));
    let profile = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=profile",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(profile.status.success());
    assert_eq!(String::from_utf8(profile.stdout).unwrap().trim(), "High");
    let decoded_frames = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_frames",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(decoded_frames.status.success());
    let decoded_frames = String::from_utf8(decoded_frames.stdout)
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap();
    assert!(
        (118..=122).contains(&decoded_frames),
        "expected about 120 decoded frames, got {decoded_frames}"
    );
    let null_output = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let decoded = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-xerror", "-i"])
        .arg(&output)
        .args(["-map", "0:v:0", "-f", "null", null_output])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "smart-cut output did not decode cleanly: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );

    let raw_frames = |path: &Path, start: &str, frames: &str| {
        let mut command = std::process::Command::new("ffmpeg");
        command.args(["-hide_banner", "-loglevel", "error"]);
        if start != "0" {
            command.args(["-ss", start]);
        }
        let output = command
            .arg("-i")
            .arg(path)
            .args([
                "-frames:v",
                frames,
                "-map",
                "0:v:0",
                "-pix_fmt",
                "yuv420p",
                "-f",
                "rawvideo",
                "-",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        output.stdout
    };
    let source_boundary = raw_frames(&source, "6.7", "30");
    let output_boundary = raw_frames(&output, "0", "30");
    assert_eq!(source_boundary.len(), output_boundary.len());
    let mean_absolute_error = source_boundary
        .iter()
        .zip(&output_boundary)
        .map(|(left, right)| left.abs_diff(*right) as u64)
        .sum::<u64>() as f64
        / source_boundary.len().max(1) as f64;
    assert!(
        mean_absolute_error < 0.5,
        "smart-cut boundary quality regressed: MAE {mean_absolute_error:.4}"
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
#[ignore = "set CONTAINER_CAMERA_TEST_MEDIA to inspect real videos (separate several with ;)"]
async fn camera_detection_real_media_report() {
    let paths = std::env::var("CONTAINER_CAMERA_TEST_MEDIA")
        .expect("CONTAINER_CAMERA_TEST_MEDIA must point to one or more real videos");
    for path in paths.split(';').filter(|path| !path.trim().is_empty()) {
        let info = probe_media(path.to_string()).await.unwrap();
        let result = detect_camera_region(
            path.to_string(),
            info.duration.unwrap(),
            info.width.unwrap() as u32,
            info.height.unwrap() as u32,
        )
        .await;
        match result {
            Ok(result) => println!(
                "camera x={:.3} y={:.3} w={:.3} h={:.3} confidence={:.3} samples={}/{} :: {path}",
                result.x,
                result.y,
                result.width,
                result.height,
                result.confidence,
                result.matched_samples,
                result.samples
            ),
            Err(error) => println!("camera none ({error}) :: {path}"),
        }
    }
}

fn decoded_image_hash(path: &Path) -> String {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "could not decode {}",
        path.display()
    );
    format!("{:x}", Sha256::digest(&output.stdout))
}

#[tokio::test]
async fn image_workflows_render_real_formats_and_preserve_geometry() {
    let root = std::env::temp_dir().join("CONTAINER görsel test space");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let transparent = root.join("şeffaf kaynak.png");
    let photo = root.join("foto source.jpg");
    let logo = root.join("logo alpha.webp");
    for (path, source, codec) in [
        (&transparent, "color=c=black@0.0:s=640x360,format=rgba,drawbox=x=80:y=60:w=300:h=180:color=red@0.7:t=fill", "png"),
        (&photo, "testsrc2=s=360x640", "mjpeg"),
        (&logo, "color=c=black@0.0:s=120x80,format=rgba,drawbox=x=10:y=10:w=100:h=60:color=yellow@0.65:t=fill", "libwebp"),
    ] {
        assert!(std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", source, "-frames:v", "1", "-c:v", codec])
            .arg(path)
            .status().unwrap().success());
    }
    let transparent_info = probe_media(transparent.to_string_lossy().into_owned())
        .await
        .unwrap();
    let photo_info = probe_media(photo.to_string_lossy().into_owned())
        .await
        .unwrap();
    let base_transform = |ratio: &str, mode: &str, format: &str| OperationRequest {
        input: transparent.to_string_lossy().into_owned(),
        operation: "transform".into(),
        params: values(&[
            ("crop_mode", ratio),
            ("fit_mode", mode),
            ("canvas_background", "transparent"),
            ("canvas_color", "#203040"),
            ("crop_x", "0"),
            ("crop_y", "0"),
            ("crop_w", "100"),
            ("crop_h", "100"),
            ("rotate", "0"),
            ("flip_h", "false"),
            ("flip_v", "false"),
            ("size_mode", "source"),
            ("size", "1080"),
            ("output_width", "1920"),
            ("output_height", "1080"),
            ("format", format),
            ("jpeg_background", "#ffffff"),
        ]),
    };
    let portrait_fit =
        render_request(&base_transform("4:5", "contain", "png"), &transparent_info).await;
    let portrait_info = probe_media(portrait_fit.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(
        (portrait_info.width, portrait_info.height),
        (Some(640), Some(800))
    );
    assert!(image_has_alpha(&portrait_info));

    let square_request = OperationRequest {
        input: photo.to_string_lossy().into_owned(),
        ..base_transform("1:1", "contain", "png")
    };
    let square = render_request(&square_request, &photo_info).await;
    let square_info = probe_media(square.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(square_info.width, square_info.height);

    for (format, expected) in [
        ("jpg", "jpg"),
        ("webp", "webp"),
        ("bmp", "bmp"),
        ("tiff", "tiff"),
    ] {
        let converted =
            render_request(&base_transform("off", "crop", format), &transparent_info).await;
        assert_eq!(
            converted.extension().and_then(|value| value.to_str()),
            Some(expected)
        );
        let converted_info = probe_media(converted.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(
            (converted_info.width, converted_info.height),
            (Some(640), Some(360))
        );
    }
    let avif_request = OperationRequest {
        input: photo.to_string_lossy().into_owned(),
        ..base_transform("off", "crop", "avif")
    };
    let avif = render_request(&avif_request, &photo_info).await;
    assert_eq!(
        avif.extension().and_then(|value| value.to_str()),
        Some("avif")
    );
    assert_eq!(
        probe_media(avif.to_string_lossy().into_owned())
            .await
            .unwrap()
            .kind,
        "image"
    );

    let font = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
        .join("Fonts")
        .join("arial.ttf");
    assert!(font.is_file());
    let text_layers = serde_json::json!([{"text":"İstanbul test\nönizleme ile aynı satır","x":50,"y":50,"size":42,"color":"#ffffff","opacity":85,"align":"right","font_path":font,"outline":2,"outline_color":"#000000","shadow":2,"shadow_color":"#000000","background":true,"background_color":"#203040","background_opacity":60,"background_padding":8}]).to_string();
    let text = render_request(
        &OperationRequest {
            input: transparent.to_string_lossy().into_owned(),
            operation: "text".into(),
            params: values(&[("layers", &text_layers)]),
        },
        &transparent_info,
    )
    .await;
    assert_eq!(
        probe_media(text.to_string_lossy().into_owned())
            .await
            .unwrap()
            .width,
        Some(640)
    );

    for effect in ["blur", "pixelate"] {
        let output = render_request(
            &OperationRequest {
                input: transparent.to_string_lossy().into_owned(),
                operation: "blur_pixelate".into(),
                params: values(&[
                    ("effect", effect),
                    ("strength", "20"),
                    ("region_x", "20"),
                    ("region_y", "20"),
                    ("region_w", "40"),
                    ("region_h", "40"),
                ]),
            },
            &transparent_info,
        )
        .await;
        assert_eq!(
            probe_media(output.to_string_lossy().into_owned())
                .await
                .unwrap()
                .height,
            Some(360)
        );
    }
    let overlay = render_request(
        &OperationRequest {
            input: transparent.to_string_lossy().into_owned(),
            operation: "image_overlay".into(),
            params: values(&[
                ("image_path", &logo.to_string_lossy()),
                ("position", "bottom_right"),
                ("x", "50"),
                ("y", "50"),
                ("size", "25"),
                ("opacity", "55"),
                ("margin", "4"),
            ]),
        },
        &transparent_info,
    )
    .await;
    assert_eq!(
        probe_media(overlay.to_string_lossy().into_owned())
            .await
            .unwrap()
            .width,
        Some(640)
    );

    let adjusted = render_request(
        &OperationRequest {
            input: photo.to_string_lossy().into_owned(),
            operation: "color".into(),
            params: values(&[
                ("brightness", "8"),
                ("brightness_enabled", "true"),
                ("contrast", "110"),
                ("contrast_enabled", "true"),
                ("denoise", "off"),
                ("grayscale", "off"),
                ("deinterlace", "off"),
            ]),
        },
        &photo_info,
    )
    .await;
    assert_eq!(
        probe_media(adjusted.to_string_lossy().into_owned())
            .await
            .unwrap()
            .height,
        Some(640)
    );

    let tagged = root.join("metadata source.jpg");
    assert!(std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&photo)
        .args([
            "-metadata",
            "comment=private-gps-camera-data",
            "-c:v",
            "copy"
        ])
        .arg(&tagged)
        .status()
        .unwrap()
        .success());
    let tagged_info = probe_media(tagged.to_string_lossy().into_owned())
        .await
        .unwrap();
    let cleaned = render_request(
        &OperationRequest {
            input: tagged.to_string_lossy().into_owned(),
            operation: "metadata_cleaner".into(),
            params: HashMap::new(),
        },
        &tagged_info,
    )
    .await;
    assert_eq!(decoded_image_hash(&tagged), decoded_image_hash(&cleaned));
    let metadata = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format_tags", "-of", "json"])
        .arg(&cleaned)
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&metadata.stdout).contains("private-gps-camera-data"));

    let state = JobState::default();
    let quality = run_image_compressor(
        None,
        &state,
        &OperationRequest {
            input: transparent.to_string_lossy().into_owned(),
            operation: "image_compressor".into(),
            params: values(&[
                ("mode", "quality"),
                ("format", "webp"),
                ("quality", "70"),
                ("target_kb", "50"),
                ("jpeg_background", "#ffffff"),
            ]),
        },
        &transparent_info,
    )
    .await
    .unwrap();
    let quality_path = PathBuf::from(quality.output);
    assert_eq!(
        probe_media(quality_path.to_string_lossy().into_owned())
            .await
            .unwrap()
            .width,
        Some(640)
    );
    let quality_size = std::fs::metadata(&quality_path).unwrap().len();
    assert_eq!(quality_path.parent().unwrap().file_name().unwrap(), "image");
    for format in ["png", "jpg"] {
        let request = OperationRequest {
            input: transparent.to_string_lossy().into_owned(),
            operation: "image_compressor".into(),
            params: values(&[
                ("mode", "quality"),
                ("format", format),
                ("quality", "62"),
                ("target_kb", "50"),
                ("jpeg_background", "#ffffff"),
            ]),
        };
        let size = estimate_image_compression(OperationRequest {
            input: request.input.clone(),
            operation: request.operation.clone(),
            params: request.params.clone(),
        })
        .await
        .unwrap();
        let result = run_image_compressor(None, &state, &request, &transparent_info)
            .await
            .unwrap();
        assert_eq!(size, std::fs::metadata(&result.output).unwrap().len());
        assert_eq!(
            Path::new(&result.output)
                .parent()
                .unwrap()
                .file_name()
                .unwrap(),
            "image"
        );
    }
    let estimated = estimate_image_compression(OperationRequest {
        input: transparent.to_string_lossy().into_owned(),
        operation: "image_compressor".into(),
        params: values(&[
            ("mode", "quality"),
            ("format", "webp"),
            ("quality", "70"),
            ("target_kb", "1"),
            ("jpeg_background", "#ffffff"),
        ]),
    })
    .await
    .unwrap();
    assert_eq!(estimated, quality_size);
    for png_mode in ["lossless", "palette"] {
        let result = run_image_compressor(
            None,
            &state,
            &OperationRequest {
                input: transparent.to_string_lossy().into_owned(),
                operation: "image_compressor".into(),
                params: values(&[
                    ("mode", "target"),
                    ("format", "source"),
                    ("png_mode", png_mode),
                    ("quality", "82"),
                    ("target_kb", "1"),
                ]),
            },
            &transparent_info,
        )
        .await
        .unwrap();
        let png = PathBuf::from(result.output);
        assert_eq!(png.extension().unwrap(), "png");
        let decoded = image::open(&png).unwrap().into_rgba8();
        let original = image::open(&transparent).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), original.dimensions());
        assert!(
            std::fs::metadata(&png).unwrap().len()
                <= std::fs::metadata(&transparent).unwrap().len()
        );
        if png_mode == "lossless" {
            assert_eq!(decoded, original);
        }
    }
    let target = run_image_compressor(
        None,
        &state,
        &OperationRequest {
            input: transparent.to_string_lossy().into_owned(),
            operation: "image_compressor".into(),
            params: values(&[
                ("mode", "target"),
                ("format", "jpg"),
                ("quality", "80"),
                ("target_kb", "20"),
                ("jpeg_background", "#f0f0f0"),
            ]),
        },
        &transparent_info,
    )
    .await
    .unwrap();
    let target_path = PathBuf::from(target.output);
    assert!(std::fs::metadata(&target_path).unwrap().len() <= 24 * 1024);
    assert_eq!(
        probe_media(target_path.to_string_lossy().into_owned())
            .await
            .unwrap()
            .height,
        Some(360)
    );
    std::fs::remove_dir_all(&root).unwrap();
}
