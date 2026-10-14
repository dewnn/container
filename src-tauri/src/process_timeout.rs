//! Time limits for short helper processes (probes, previews, capability
//! checks). A damaged file or a stuck driver must not leave the UI waiting
//! forever. The child is stopped when the limit is reached (`kill_on_drop`).

use std::process::Output;
use std::time::Duration;
use tokio::process::Command;

pub(crate) const PROBE: Duration = Duration::from_secs(60);
pub(crate) const VERSION_CHECK: Duration = Duration::from_secs(20);
pub(crate) const PREVIEW: Duration = Duration::from_secs(90);
pub(crate) const FRAME_SAMPLE: Duration = Duration::from_secs(30);
/// Waveforms decode the whole audio track, so long recordings need time.
pub(crate) const FULL_DECODE: Duration = Duration::from_secs(15 * 60);

pub(crate) async fn output_within(
    command: &mut Command,
    limit: Duration,
) -> std::io::Result<Output> {
    command.kill_on_drop(true);
    match tokio::time::timeout(limit, command.output()).await {
        Ok(result) => result,
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("no response within {} seconds", limit.as_secs()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn slow_processes_are_stopped_at_the_limit() {
        let mut command = if cfg!(target_os = "windows") {
            let mut command = Command::new("powershell");
            command.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
            command
        } else {
            let mut command = Command::new("sleep");
            command.arg("30");
            command
        };
        let started = std::time::Instant::now();
        let error = output_within(&mut command, Duration::from_millis(300))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
