//! Cancellable PCM decoding for SmartCut analysis.
//!
//! FFmpeg output is converted to samples while it streams, so a long recording
//! is held in memory once instead of as raw bytes plus a decoded copy. The
//! process is registered with the job state, so `cancel_job` can stop it.

use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::{hidden_command, ActiveProcess, JobState};

pub(crate) const CANCELLED: &str = "SmartCut analysis was cancelled.";
const STDERR_TAIL_LINES: usize = 20;

pub(crate) fn was_cancelled(state: &JobState) -> bool {
    state.cancelled.load(Ordering::Relaxed)
}

/// Decodes the first audio stream of `input` to mono PCM at `sample_rate`.
/// `format` is an FFmpeg raw format whose sample size is `N` bytes.
pub(crate) async fn decode_mono<T, const N: usize>(
    state: &JobState,
    input: &Path,
    sample_rate: u32,
    format: &str,
    parse: fn([u8; N]) -> T,
) -> Result<Vec<T>, String> {
    let mut child = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar"])
        .arg(sample_rate.to_string())
        .args(["-f", format, "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("FFmpeg audio analysis could not start: {error}"))?;
    let _process = ActiveProcess::register(state, child.id())?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or("FFmpeg audio output is unavailable.")?;
    let stderr = child.stderr.take();
    let stderr_task = tokio::spawn(async move {
        let mut tail = std::collections::VecDeque::with_capacity(STDERR_TAIL_LINES);
        if let Some(stderr) = stderr {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tail.len() == STDERR_TAIL_LINES {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        }
        tail.into_iter().collect::<Vec<_>>().join("\n")
    });

    let mut samples = Vec::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut carry = [0_u8; N];
    let mut carried = 0;
    loop {
        if was_cancelled(state) {
            let _ = child.start_kill();
            return Err(CANCELLED.into());
        }
        let read = stdout
            .read(&mut buffer)
            .await
            .map_err(|error| format!("FFmpeg audio could not be read: {error}"))?;
        if read == 0 {
            break;
        }
        let mut bytes = &buffer[..read];
        if carried > 0 {
            let needed = (N - carried).min(bytes.len());
            carry[carried..carried + needed].copy_from_slice(&bytes[..needed]);
            carried += needed;
            bytes = &bytes[needed..];
            if carried < N {
                continue;
            }
            samples.push(parse(carry));
        }
        let (chunks, rest) = bytes.as_chunks::<N>();
        samples.extend(chunks.iter().map(|chunk| parse(*chunk)));
        carry[..rest.len()].copy_from_slice(rest);
        carried = rest.len();
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("FFmpeg audio analysis failed: {error}"))?;
    let stderr = stderr_task.await.unwrap_or_default();
    if was_cancelled(state) {
        return Err(CANCELLED.into());
    }
    if !status.success() {
        let message = stderr.trim();
        return Err(if message.is_empty() {
            "FFmpeg could not decode the audio for analysis.".into()
        } else {
            message.to_string()
        });
    }
    samples.shrink_to_fit();
    Ok(samples)
}
