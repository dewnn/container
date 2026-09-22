use crate::{app_storage_namespace, hidden_command, operation_temp_dir};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::{Duration, Instant},
};

static AVAILABLE_ENCODERS: OnceLock<Vec<String>> = OnceLock::new();
static ENCODER_ENVIRONMENT_FINGERPRINT: OnceLock<String> = OnceLock::new();

async fn detect_available_encoders() -> Vec<String> {
    if let Some(encoders) = AVAILABLE_ENCODERS.get() {
        return encoders.clone();
    }
    if let Some(cache) = read_auto_encoder_cache() {
        if cache.fingerprint == encoder_environment_fingerprint().await {
            let _ = AVAILABLE_ENCODERS.set(cache.encoders.clone());
            return cache.encoders;
        }
    }
    let candidates = [
        "libx264",
        "h264_amf",
        "h264_nvenc",
        "h264_qsv",
        "libx265",
        "hevc_amf",
        "hevc_nvenc",
        "libvpx-vp9",
        "libsvtav1",
        "av1_nvenc",
        "av1_amf",
        "av1_qsv",
    ];
    let mut checks = tokio::task::JoinSet::new();
    for encoder in candidates {
        checks.spawn(async move {
            let mut command = hidden_command("ffmpeg");
            command.args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:r=1:d=1",
                "-frames:v",
                "1",
                "-c:v",
                encoder,
                "-f",
                "null",
                "-",
            ]);
            // Capability probes are expected to fail for GPUs that are not
            // installed. Keep those normal failures out of the user's console.
            command.stdout(Stdio::null()).stderr(Stdio::null());
            #[cfg(target_os = "windows")]
            command.creation_flags(0x08000000);
            let works = tokio::time::timeout(std::time::Duration::from_secs(8), command.status())
                .await
                .ok()
                .and_then(Result::ok)
                .is_some_and(|status| status.success());
            (encoder.to_string(), works)
        });
    }
    let mut available = Vec::new();
    while let Some(result) = checks.join_next().await {
        if let Ok((encoder, true)) = result {
            available.push(encoder);
        }
    }
    available.sort();
    let _ = AVAILABLE_ENCODERS.set(available.clone());
    available
}

#[tauri::command]
pub async fn available_encoders() -> Vec<String> {
    detect_available_encoders().await
}

#[derive(Clone)]
struct AutoH264Encoder {
    key: &'static str,
    name: &'static str,
    args: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct AutoEncoderCache {
    schema: u8,
    fingerprint: String,
    key: String,
    encoders: Vec<String>,
}

struct EncoderBenchmark {
    elapsed: Duration,
    ssim: f64,
}

fn select_quality_eligible_encoder(
    measured: Vec<(AutoH264Encoder, EncoderBenchmark)>,
) -> Option<AutoH264Encoder> {
    let cpu_quality = measured
        .iter()
        .find(|(candidate, _)| candidate.key == "x264-veryfast-crf14")
        .map(|(_, result)| result.ssim);
    // The CPU result is the reference for this exact synthetic scene. Absolute
    // SSIM varies slightly between FFmpeg builds, so only compare candidates
    // against that local reference instead of imposing a higher fixed floor.
    let quality_floor = cpu_quality.map(|quality| (quality - 0.002).max(0.970));
    measured
        .into_iter()
        .filter(|(_, result)| quality_floor.is_none_or(|floor| result.ssim >= floor))
        .min_by_key(|(_, result)| result.elapsed)
        .map(|(profile, _)| profile)
}

static AUTO_H264_ENCODER: OnceLock<AutoH264Encoder> = OnceLock::new();

fn h264_encoder_candidates(available: &[String]) -> Vec<AutoH264Encoder> {
    let mut candidates = vec![AutoH264Encoder {
        key: "x264-veryfast-crf14",
        name: "CPU · x264 veryfast",
        args: vec![
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            "14".into(),
            "-preset".into(),
            "veryfast".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ],
    }];
    if available.iter().any(|encoder| encoder == "h264_nvenc") {
        candidates.push(AutoH264Encoder {
            key: "nvenc-p4-cq14",
            name: "NVIDIA · NVENC",
            args: vec![
                "-c:v".into(),
                "h264_nvenc".into(),
                "-preset".into(),
                "p4".into(),
                "-tune".into(),
                "hq".into(),
                "-rc".into(),
                "vbr".into(),
                "-cq".into(),
                "14".into(),
                "-b:v".into(),
                "0".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ],
        });
    }
    if available.iter().any(|encoder| encoder == "h264_qsv") {
        candidates.push(AutoH264Encoder {
            key: "qsv-medium-q14",
            name: "Intel · Quick Sync",
            args: vec![
                "-c:v".into(),
                "h264_qsv".into(),
                "-preset".into(),
                "medium".into(),
                "-global_quality".into(),
                "14".into(),
                "-pix_fmt".into(),
                "nv12".into(),
            ],
        });
    }
    if available.iter().any(|encoder| encoder == "h264_amf") {
        candidates.push(AutoH264Encoder {
            key: "amf-speed-qp14",
            name: "AMD · AMF",
            args: vec![
                "-c:v".into(),
                "h264_amf".into(),
                "-quality".into(),
                "speed".into(),
                "-rc".into(),
                "cqp".into(),
                "-qp_i".into(),
                "14".into(),
                "-qp_p".into(),
                "14".into(),
                "-pix_fmt".into(),
                "nv12".into(),
            ],
        });
    }
    candidates
}

fn auto_encoder_cache_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|root| {
        root.join(app_storage_namespace())
            .join("preferences")
            .join("auto-encoder.json")
    })
}

fn read_auto_encoder_cache() -> Option<AutoEncoderCache> {
    let cache: AutoEncoderCache =
        serde_json::from_slice(&std::fs::read(auto_encoder_cache_path()?).ok()?).ok()?;
    (cache.schema == 2).then_some(cache)
}

fn cached_h264_encoder(fingerprint: &str, available: &[String]) -> Option<AutoH264Encoder> {
    let cache = read_auto_encoder_cache()?;
    if cache.fingerprint != fingerprint {
        return None;
    }
    // Never trust command-line arguments from disk. The cache stores only a
    // small known profile key; the actual FFmpeg arguments remain in code.
    h264_encoder_candidates(available)
        .into_iter()
        .find(|candidate| candidate.key == cache.key)
}

#[cfg(not(test))]
fn persist_h264_encoder(profile: &AutoH264Encoder, encoders: &[String], fingerprint: &str) {
    let Some(path) = auto_encoder_cache_path() else {
        return;
    };
    let Some(directory) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(directory).is_err() {
        return;
    }
    let cache = AutoEncoderCache {
        schema: 2,
        fingerprint: fingerprint.into(),
        key: profile.key.into(),
        encoders: encoders.to_vec(),
    };
    if let Ok(contents) = serde_json::to_vec(&cache) {
        let _ = std::fs::write(path, contents);
    }
}

#[tauri::command]
pub async fn auto_encoder_configured() -> bool {
    let fingerprint = encoder_environment_fingerprint().await;
    read_auto_encoder_cache().is_some_and(|cache| cache.fingerprint == fingerprint)
}

async fn encoder_environment_fingerprint() -> String {
    if let Some(fingerprint) = ENCODER_ENVIRONMENT_FINGERPRINT.get() {
        return fingerprint.clone();
    }
    let ffmpeg = hidden_command("ffmpeg")
        .args(["-hide_banner", "-version"])
        .output()
        .await
        .ok()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default();
    #[cfg(target_os = "windows")]
    let hardware = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_VideoController | Sort-Object PNPDeviceID | ForEach-Object { \"$($_.PNPDeviceID)|$($_.DriverVersion)\" }",
        ])
        .output()
        .await
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    #[cfg(not(target_os = "windows"))]
    let hardware = std::env::consts::ARCH.to_string();
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(format!("encoder-benchmark-v2\n{ffmpeg}\n{hardware}"))
    );
    let _ = ENCODER_ENVIRONMENT_FINGERPRINT.set(fingerprint.clone());
    fingerprint
}

fn parse_ssim(output: &[u8]) -> Option<f64> {
    let text = String::from_utf8_lossy(output);
    let value = text.rsplit("All:").next()?.split_whitespace().next()?;
    value.parse().ok()
}

async fn create_encoder_benchmark_source(path: &Path) -> Result<(), String> {
    let status = hidden_command("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=60:duration=1",
            "-vf",
            "scale=-2:1080:flags=lanczos,format=yuv420p",
            "-an",
            "-c:v",
            "ffv1",
        ])
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| error.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or("Encoder benchmark source could not be created.".into())
}

async fn benchmark_h264_encoder(
    candidate: &AutoH264Encoder,
    source: &Path,
    output: &Path,
) -> Option<EncoderBenchmark> {
    let mut command = hidden_command("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-y", "-i"]);
    command.arg(source);
    command.arg("-an");
    command.args(&candidate.args);
    command.arg(output);
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let started = Instant::now();
    let works = tokio::time::timeout(Duration::from_secs(15), command.status())
        .await
        .ok()
        .and_then(Result::ok)
        .is_some_and(|status| status.success());
    if !works {
        return None;
    }
    let elapsed = started.elapsed();
    let comparison = tokio::time::timeout(
        Duration::from_secs(15),
        hidden_command("ffmpeg")
            .args(["-hide_banner", "-i"])
            .arg(output)
            .arg("-i")
            .arg(source)
            .args(["-lavfi", "[0:v][1:v]ssim", "-f", "null", "-"])
            .stdout(Stdio::null())
            .output(),
    )
    .await
    .ok()
    .and_then(Result::ok)?;
    Some(EncoderBenchmark {
        elapsed,
        ssim: parse_ssim(&comparison.stderr)?,
    })
}

pub(crate) async fn fastest_h264_encoder() -> (&'static str, Vec<String>) {
    if let Some(profile) = AUTO_H264_ENCODER.get() {
        return (profile.name, profile.args.clone());
    }
    let fingerprint = encoder_environment_fingerprint().await;
    let available = detect_available_encoders().await;
    if let Some(profile) = cached_h264_encoder(&fingerprint, &available) {
        let _ = AUTO_H264_ENCODER.set(profile.clone());
        return (profile.name, profile.args);
    }
    let temp = operation_temp_dir("encoder-benchmark").ok();
    let source = temp.as_ref().map(|directory| directory.join("source.mkv"));
    let source_ready = match source.as_deref() {
        Some(path) => create_encoder_benchmark_source(path).await.is_ok(),
        None => false,
    };
    let mut measured = Vec::new();
    if source_ready {
        for (index, candidate) in h264_encoder_candidates(&available).into_iter().enumerate() {
            let output = temp
                .as_ref()
                .expect("encoder benchmark directory exists")
                .join(format!("candidate-{index}.mp4"));
            if let Some(result) = benchmark_h264_encoder(
                &candidate,
                source.as_ref().expect("encoder benchmark source exists"),
                &output,
            )
            .await
            {
                measured.push((candidate, result));
            }
        }
    }
    if let Some(directory) = &temp {
        let _ = std::fs::remove_dir_all(directory);
    }
    #[cfg(test)]
    for (candidate, result) in &measured {
        eprintln!(
            "encoder candidate {}: {:?}, SSIM {:.6}",
            candidate.name, result.elapsed, result.ssim
        );
    }
    let selected = select_quality_eligible_encoder(measured).unwrap_or_else(|| AutoH264Encoder {
        key: "x264-veryfast-crf14",
        name: "CPU · x264 fallback",
        args: vec![
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            "14".into(),
            "-preset".into(),
            "veryfast".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ],
    });
    // Unit/integration media tests must never change the real user's first-run
    // preference. Only packaged application runs persist the benchmark result.
    #[cfg(not(test))]
    persist_h264_encoder(&selected, &available, &fingerprint);
    let _ = AUTO_H264_ENCODER.set(selected.clone());
    (selected.name, selected.args)
}

#[tauri::command]
pub async fn warm_up_auto_encoder() -> String {
    fastest_h264_encoder().await.0.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(key: &'static str) -> AutoH264Encoder {
        AutoH264Encoder {
            key,
            name: key,
            args: Vec::new(),
        }
    }

    #[test]
    fn parses_ffmpeg_ssim_summary() {
        let output = b"[Parsed_ssim_0] SSIM Y:0.99 U:0.98 V:0.97 All:0.987654 (18.9)";
        assert_eq!(parse_ssim(output), Some(0.987654));
    }

    #[test]
    fn rejects_a_faster_encoder_below_the_cpu_quality_floor() {
        let selected = select_quality_eligible_encoder(vec![
            (
                profile("x264-veryfast-crf14"),
                EncoderBenchmark {
                    elapsed: Duration::from_millis(500),
                    ssim: 0.993,
                },
            ),
            (
                profile("nvenc-p4-cq14"),
                EncoderBenchmark {
                    elapsed: Duration::from_millis(100),
                    ssim: 0.980,
                },
            ),
        ])
        .expect("CPU profile remains eligible");
        assert_eq!(selected.key, "x264-veryfast-crf14");
    }

    #[test]
    fn selects_the_fastest_encoder_that_preserves_quality() {
        let selected = select_quality_eligible_encoder(vec![
            (
                profile("x264-veryfast-crf14"),
                EncoderBenchmark {
                    elapsed: Duration::from_millis(500),
                    ssim: 0.993,
                },
            ),
            (
                profile("qsv-medium-q14"),
                EncoderBenchmark {
                    elapsed: Duration::from_millis(120),
                    ssim: 0.992,
                },
            ),
        ])
        .expect("quality-equivalent hardware profile is eligible");
        assert_eq!(selected.key, "qsv-medium-q14");
    }

    #[tokio::test]
    async fn cpu_encoder_benchmark_measures_quality_and_cleans_up() {
        let temp = operation_temp_dir("encoder-quality-test").unwrap();
        let work = async {
            let source = temp.join("source.mkv");
            let output = temp.join("candidate.mp4");
            create_encoder_benchmark_source(&source).await?;
            let mut candidates = h264_encoder_candidates(&[]);
            let candidate = candidates.remove(0);
            benchmark_h264_encoder(&candidate, &source, &output)
                .await
                .ok_or_else(|| "bundled x264 benchmark should succeed".to_string())
        }
        .await;
        std::fs::remove_dir_all(&temp).unwrap();
        assert!(!temp.exists());
        let result = work.unwrap();
        assert!(result.ssim >= 0.980, "unexpected SSIM: {}", result.ssim);
    }

    #[tokio::test]
    async fn automatic_encoder_selects_a_verified_profile_on_this_pc() {
        let (name, args) = fastest_h264_encoder().await;
        eprintln!("verified automatic encoder: {name}");
        assert!(!name.is_empty());
        assert!(args.windows(2).any(|pair| pair[0] == "-c:v"));
    }
}
