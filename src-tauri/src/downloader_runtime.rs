//! Keeps the managed yt-dlp copy current.
//!
//! The installer ships yt-dlp beside the executable, but the downloader runs a
//! managed copy under LocalAppData so the user can replace it. Earlier builds
//! only copied the bundled file once, so app updates never refreshed it. The
//! origin marker records whether the managed copy came from the bundle or was
//! chosen by the user; only bundled copies are refreshed automatically.

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

use crate::{
    downloader_binary_path, downloader_version_at, downloader_version_within, hidden_path_command,
    materialize_runtime_file, DownloaderStatus, JobLease, JobState,
};

const ORIGIN_BUNDLED: &str = "bundled";
const ORIGIN_USER: &str = "user";
const SELF_UPDATE_TIMEOUT: Duration = Duration::from_secs(180);
const BROKEN_CHECK_TIMEOUT: Duration = Duration::from_secs(30);

fn origin_marker(binary: &Path) -> PathBuf {
    binary.with_extension("origin")
}

pub(crate) fn mark_origin(binary: &Path, user_selected: bool) {
    let _ = std::fs::write(
        origin_marker(binary),
        if user_selected {
            ORIGIN_USER
        } else {
            ORIGIN_BUNDLED
        },
    );
}

fn user_selected(binary: &Path) -> bool {
    std::fs::read_to_string(origin_marker(binary))
        .map(|value| value.trim() == ORIGIN_USER)
        .unwrap_or(false)
}

pub(crate) fn bundled_binary_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|directory| {
            directory.join(if cfg!(target_os = "windows") {
                "yt-dlp.exe"
            } else {
                "yt-dlp"
            })
        })
        .filter(|path| path.is_file())
}

/// yt-dlp versions are dates (`2026.08.19`), optionally with a nightly build
/// suffix (`2026.08.19.232703`). Anything else is treated as unknown.
fn parse_version(value: &str) -> Option<Vec<u64>> {
    let parts = value
        .trim()
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    (parts.len() >= 3).then_some(parts)
}

fn is_older(installed: &str, bundled: &str) -> bool {
    match (parse_version(installed), parse_version(bundled)) {
        (Some(installed), Some(bundled)) => installed < bundled,
        _ => false,
    }
}

fn should_replace(user_selected: bool, installed: Option<&str>, bundled: &str) -> bool {
    if user_selected {
        return false;
    }
    // A managed copy that no longer runs is replaced too, so a broken or
    // half-written file recovers on the next start.
    installed.is_none_or(|installed| is_older(installed, bundled))
}

/// Replaces an outdated bundled copy with the newer one shipped in this build.
/// Returns true when the managed copy was replaced.
pub(crate) async fn refresh_bundled_downloader() -> bool {
    let (Some(bundled), Ok(managed)) = (bundled_binary_path(), downloader_binary_path()) else {
        return false;
    };
    if !managed.is_file() || user_selected(&managed) {
        return false;
    }
    let Some(bundled_version) = downloader_version_at(&bundled).await else {
        return false;
    };
    // A slow first start (antivirus scan, cold disk) must not look like a
    // broken copy, or a user-updated yt-dlp would be rolled back.
    let installed_version = match downloader_version_at(&managed).await {
        Some(version) => Some(version),
        None => downloader_version_within(&managed, BROKEN_CHECK_TIMEOUT).await,
    };
    if !should_replace(false, installed_version.as_deref(), &bundled_version) {
        if !origin_marker(&managed).is_file() {
            mark_origin(&managed, false);
        }
        return false;
    }
    let replaced = tokio::task::spawn_blocking(move || {
        let staged = managed.with_extension("refresh");
        let _ = std::fs::remove_file(&staged);
        materialize_runtime_file(&bundled, &staged)?;
        let previous = managed.with_extension("previous");
        let _ = std::fs::remove_file(&previous);
        // Renaming works even when an older yt-dlp is still running.
        std::fs::rename(&managed, &previous).map_err(|error| error.to_string())?;
        if let Err(error) = std::fs::rename(&staged, &managed) {
            let _ = std::fs::rename(&previous, &managed);
            let _ = std::fs::remove_file(&staged);
            return Err(error.to_string());
        }
        let _ = std::fs::remove_file(&previous);
        mark_origin(&managed, false);
        Ok::<(), String>(())
    })
    .await;
    matches!(replaced, Ok(Ok(())))
}

/// Runs yt-dlp's own updater (`-U`) on the managed copy. yt-dlp verifies the
/// release checksums itself before replacing the executable.
#[tauri::command]
pub(crate) async fn update_downloader(
    state: tauri::State<'_, JobState>,
) -> Result<DownloaderStatus, String> {
    let _lease = JobLease::reserve(&state, "downloader-update")
        .map_err(|_| "Wait for the current job to finish before updating yt-dlp.".to_string())?;
    let binary = downloader_binary_path()?;
    if downloader_version_at(&binary).await.is_none() {
        return Err("yt-dlp is not ready. Select the official yt-dlp executable first.".into());
    }
    let mut command: Command = hidden_path_command(&binary);
    command
        .arg("-U")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = command
        .spawn()
        .map_err(|error| format!("yt-dlp update could not start: {error}"))?;
    let output = tokio::time::timeout(SELF_UPDATE_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "yt-dlp update timed out. Check your internet connection.".to_string())?
        .map_err(|error| format!("yt-dlp update failed: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("unknown error")
            .trim()
            .to_string();
        return Err(format!("yt-dlp update failed: {detail}"));
    }
    let version = downloader_version_at(&binary).await;
    Ok(DownloaderStatus {
        ready: version.is_some(),
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_versions_compare_numerically() {
        assert!(is_older("2025.12.08", "2026.08.19"));
        assert!(is_older("2026.8.9", "2026.08.19"));
        assert!(is_older("2026.08.19", "2026.08.19.120000"));
        assert!(!is_older("2026.08.19", "2026.08.19"));
        assert!(!is_older("2026.09.01", "2026.08.19"));
        assert!(!is_older("nightly", "2026.08.19"));
        assert!(!is_older("2026.08.19", "unknown"));
    }

    #[test]
    fn only_outdated_or_broken_bundled_copies_are_replaced() {
        assert!(should_replace(false, Some("2025.01.01"), "2026.08.19"));
        assert!(should_replace(false, None, "2026.08.19"));
        assert!(!should_replace(false, Some("2026.09.01"), "2026.08.19"));
        assert!(!should_replace(true, Some("2025.01.01"), "2026.08.19"));
        assert!(!should_replace(true, None, "2026.08.19"));
    }

    #[test]
    fn origin_marker_round_trips() {
        let root = std::env::temp_dir().join(format!(
            "container-ytdlp-origin-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let binary = root.join("yt-dlp.exe");
        assert!(!user_selected(&binary));
        mark_origin(&binary, true);
        assert!(user_selected(&binary));
        mark_origin(&binary, false);
        assert!(!user_selected(&binary));
        let _ = std::fs::remove_dir_all(root);
    }
}
