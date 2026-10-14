//! Downloading only part of a long video (for example one minute of a VOD).
//!
//! yt-dlp hands the range to FFmpeg, which copies just the needed media. The
//! streams are not re-encoded: exact cuts (`--force-keyframes-at-cuts`) made a
//! one-minute 1440p60 section take over 20 minutes. The section therefore
//! starts at the keyframe before the requested start, usually a few seconds
//! early, and can be trimmed precisely in the editor afterwards.

use crate::{check_range, DownloaderProgress};
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc::UnboundedSender;

/// The longest supported source: a full day of streaming.
const MAX_SECONDS: f64 = 24.0 * 60.0 * 60.0;
/// Shorter sections are almost always a typo and produce unusable files.
const MIN_LENGTH: f64 = 1.0;

/// The part of the video to download, in seconds from its start.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct SectionRequest {
    pub(crate) start: f64,
    pub(crate) end: f64,
}

/// Validated `[start, end)` range in seconds, or `None` for the whole video.
pub(crate) fn download_section(
    request: Option<SectionRequest>,
    duration: Option<f64>,
) -> Result<Option<(f64, f64)>, String> {
    let Some(SectionRequest { start, end }) = request else {
        return Ok(None);
    };
    let limit = duration
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(MAX_SECONDS)
        .min(MAX_SECONDS);
    let start = check_range(start, 0.0, limit, "Section start")?;
    let end = check_range(end, 0.0, limit + 0.5, "Section end")?.min(limit);
    if end - start < MIN_LENGTH {
        return Err("The part to download must be at least 1 second long.".into());
    }
    Ok(Some((start, end)))
}

/// Extra yt-dlp arguments for a section download.
pub(crate) fn section_args(section: Option<(f64, f64)>) -> Vec<String> {
    match section {
        Some((start, end)) => vec![
            "--download-sections".into(),
            format!("*{start:.3}-{end:.3}"),
            // `--print` puts yt-dlp in quiet mode, which also silences the
            // FFmpeg it runs for sections. Re-enable FFmpeg's status line so
            // the interface can show progress instead of 0% until the end.
            "--downloader-args".into(),
            "ffmpeg:-loglevel info -stats".into(),
        ],
        None => Vec::new(),
    }
}

/// Output name; a section download also records its range in seconds.
pub(crate) fn output_template(section: Option<(f64, f64)>) -> &'static str {
    if section.is_some() {
        "%(title).140B [%(id)s] [%(section_start)d-%(section_end)d] [%(format_id)s].%(ext)s"
    } else {
        "%(title).160B [%(id)s] [%(format_id)s].%(ext)s"
    }
}

fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (hours, minutes, rest) = (total / 3600, total % 3600 / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{rest:02}")
    } else {
        format!("{minutes}:{rest:02}")
    }
}

/// FFmpeg reports `time=HH:MM:SS.xx` while copying a section. yt-dlp's own
/// progress template does not cover this downloader, so turn the FFmpeg
/// status into the same progress event the interface already shows.
pub(crate) fn section_progress(line: &str, length: f64) -> Option<DownloaderProgress> {
    let time = line
        .rsplit("time=")
        .next()
        .filter(|_| line.contains("time="))?;
    let time = time.split_whitespace().next()?;
    let mut seconds = 0.0;
    for part in time.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    let speed = line
        .rsplit("speed=")
        .next()
        .filter(|_| line.contains("speed="))
        .and_then(|value| value.split_whitespace().next())
        .unwrap_or("")
        .to_string();
    let length = length.max(0.001);
    Some(DownloaderProgress {
        job_id: None,
        percent: (seconds / length * 100.0).clamp(0.0, 99.9),
        downloaded: format!("{} / {}", clock(seconds.min(length)), clock(length)),
        speed,
    })
}

/// Forwards text from `stream` split on both newlines and carriage returns. FFmpeg rewrites
/// its status line with carriage returns, which a plain line reader would
/// only deliver after the whole download.
pub(crate) async fn forward_lines<R: AsyncRead + Unpin>(
    mut stream: R,
    sender: UnboundedSender<(bool, String)>,
    is_stdout: bool,
) {
    let mut buffer = [0_u8; 8192];
    let mut pending = Vec::new();
    while let Ok(read) = stream.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        for &byte in &buffer[..read] {
            if byte == b'\n' || byte == b'\r' {
                if !pending.is_empty() {
                    let line = String::from_utf8_lossy(&pending).into_owned();
                    pending.clear();
                    if sender.send((is_stdout, line)).is_err() {
                        return;
                    }
                }
            } else if pending.len() < 64 * 1024 {
                pending.push(byte);
            }
        }
    }
    if !pending.is_empty() {
        let _ = sender.send((is_stdout, String::from_utf8_lossy(&pending).into_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_video_needs_no_extra_arguments() {
        assert_eq!(download_section(None, Some(60.0)).unwrap(), None);
        assert!(section_args(None).is_empty());
        assert!(!output_template(None).contains("section"));
    }

    #[test]
    fn valid_section_is_clamped_to_the_video() {
        let section = download_section(
            Some(SectionRequest {
                start: 21039.0,
                end: 21145.5,
            }),
            Some(30000.0),
        )
        .unwrap();
        assert_eq!(section, Some((21039.0, 21145.5)));
        assert_eq!(
            section_args(section),
            [
                "--download-sections",
                "*21039.000-21145.500",
                "--downloader-args",
                "ffmpeg:-loglevel info -stats"
            ]
        );
        // A rounded end just past the duration is accepted and clamped.
        assert_eq!(
            download_section(
                Some(SectionRequest {
                    start: 50.0,
                    end: 60.3
                }),
                Some(60.0)
            )
            .unwrap(),
            Some((50.0, 60.0))
        );
        assert!(output_template(section).contains("section_start"));
    }

    #[test]
    fn ffmpeg_status_becomes_section_progress() {
        let line = "frame= 1748 fps= 68 q=-1.0 size=   15360KiB time=00:00:28.57 bitrate=4403.6kbits/s speed=1.11x elapsed=0:00:25.77";
        let progress = section_progress(line, 71.0).unwrap();
        assert!(
            (progress.percent - 40.24).abs() < 0.05,
            "{}",
            progress.percent
        );
        assert_eq!(progress.downloaded, "0:29 / 1:11");
        assert_eq!(progress.speed, "1.11x");
        assert!(section_progress("[download] Destination: x.webm", 71.0).is_none());
        assert!(section_progress("size=1KiB time=N/A bitrate=N/A", 71.0).is_none());
    }

    #[tokio::test]
    async fn carriage_returns_split_status_lines() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let input = b"first\rframe=1 time=00:00:01.00\rframe=2 time=00:00:02.00\nlast".to_vec();
        forward_lines(std::io::Cursor::new(input), sender, false).await;
        let mut lines = Vec::new();
        while let Ok((_, line)) = receiver.try_recv() {
            lines.push(line);
        }
        assert_eq!(
            lines,
            [
                "first",
                "frame=1 time=00:00:01.00",
                "frame=2 time=00:00:02.00",
                "last"
            ]
        );
    }

    #[test]
    fn invalid_sections_are_rejected() {
        assert!(download_section(
            Some(SectionRequest {
                start: 30.0,
                end: 30.5
            }),
            Some(60.0)
        )
        .is_err());
        assert!(download_section(
            Some(SectionRequest {
                start: 40.0,
                end: 20.0
            }),
            Some(60.0)
        )
        .is_err());
        assert!(download_section(
            Some(SectionRequest {
                start: -1.0,
                end: 20.0
            }),
            Some(60.0)
        )
        .is_err());
        assert!(download_section(
            Some(SectionRequest {
                start: 10.0,
                end: 90.0
            }),
            Some(60.0)
        )
        .is_err());
        assert!(download_section(
            Some(SectionRequest {
                start: f64::NAN,
                end: 20.0
            }),
            Some(60.0)
        )
        .is_err());
    }
}
