//! Paths, bundled programs (FFmpeg, FFprobe) and hidden child processes.

use crate::*;

#[cfg(target_os = "windows")]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) const FFMPEG_RUNTIME_VERSION: &str = env!("CONTAINER_FFMPEG_RUNTIME_VERSION");

pub(crate) fn is_development_executable(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("container-studio-dev.exe"))
}

pub(crate) fn is_development_build() -> bool {
    std::env::current_exe()
        .ok()
        .is_some_and(|path| is_development_executable(&path))
}

pub(crate) fn app_storage_namespace() -> &'static str {
    if is_development_build() {
        "dev.dean.container.dev"
    } else {
        "dev.dean.container"
    }
}

// Old releases used the product name, which collides with the NSIS install
// folder on Windows. Only use this name when reading/migrating legacy data.
pub(crate) fn legacy_storage_name() -> &'static str {
    if is_development_build() {
        "CONTAINER DEV"
    } else {
        "CONTAINER"
    }
}

#[cfg(test)]
pub(crate) fn test_runtime_root() -> PathBuf {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        std::env::temp_dir().join(format!(
            "container-test-runtime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    })
    .clone()
}

#[cfg(not(test))]
pub(crate) fn app_data_directory() -> Result<PathBuf, String> {
    Ok(dirs::data_local_dir()
        .ok_or("Application data folder is unavailable.")?
        .join(app_storage_namespace())
        .join("data"))
}

#[cfg(test)]
pub(crate) fn app_data_directory() -> Result<PathBuf, String> {
    Ok(test_runtime_root()
        .join(app_storage_namespace())
        .join("data"))
}

#[cfg(not(test))]
pub(crate) fn app_cache_directory() -> Result<PathBuf, String> {
    Ok(dirs::cache_dir()
        .ok_or("User cache directory unavailable")?
        .join(app_storage_namespace())
        .join("cache"))
}

#[cfg(test)]
pub(crate) fn app_cache_directory() -> Result<PathBuf, String> {
    Ok(test_runtime_root()
        .join(app_storage_namespace())
        .join("cache"))
}

// FFmpeg, FFprobe and taskkill are background workers. On Windows, spawning a
// console program from the GUI without this flag briefly opens a CMD window.
// stdout/stderr remain available, so progress and error reporting are intact.
pub(crate) fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(resolve_program(program));
    // Tokio otherwise lets a spawned child continue when the future owning it
    // is dropped (for example on a timeout or application shutdown).
    command.kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    command
}

pub(crate) fn hidden_path_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    command
}

pub(crate) fn resolve_program(program: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let override_name = match program {
            "ffmpeg" => Some("CONTAINER_FFMPEG"),
            "ffprobe" => Some("CONTAINER_FFPROBE"),
            _ => None,
        };
        if let Some(path) = override_name.and_then(std::env::var_os).map(PathBuf::from) {
            if path.is_file() {
                return path;
            }
        }
        if override_name.is_some() {
            if let Ok(directory) = std::env::current_exe().and_then(|path| {
                path.parent()
                    .map(Path::to_path_buf)
                    .ok_or(std::io::ErrorKind::NotFound.into())
            }) {
                for filename in [
                    format!("{program}.exe"),
                    format!("{program}-x86_64-pc-windows-msvc.exe"),
                ] {
                    let candidate = directory.join(filename);
                    if candidate.is_file() {
                        return candidate;
                    }
                }
            }
            if let Some(candidate) =
                managed_runtime_dir().map(|path| path.join(format!("{program}.exe")))
            {
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from(program)
}

pub(crate) fn managed_runtime_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|directory| {
        directory
            .join(app_storage_namespace())
            .join("runtime")
            .join(format!("ffmpeg-{FFMPEG_RUNTIME_VERSION}"))
    })
}

pub(crate) fn materialize_runtime_file(source: &Path, destination: &Path) -> Result<(), String> {
    let source_length = std::fs::metadata(source)
        .map_err(|error| error.to_string())?
        .len();
    if let Ok(metadata) = std::fs::metadata(destination) {
        if metadata.is_file() && metadata.len() == source_length && source_length > 0 {
            return Ok(());
        }
        std::fs::remove_file(destination).map_err(|error| error.to_string())?;
    }
    let parent = destination
        .parent()
        .ok_or("Runtime destination folder could not be resolved.")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("component"),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temporary);
    if std::fs::hard_link(source, &temporary).is_err() {
        std::fs::copy(source, &temporary).map_err(|error| error.to_string())?;
    }
    match std::fs::rename(&temporary, destination) {
        Ok(()) => Ok(()),
        Err(_) if destination.is_file() => {
            let _ = std::fs::remove_file(temporary);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(temporary);
            Err(error.to_string())
        }
    }
}

pub(crate) fn migrate_runtime_pair(
    source_directory: &Path,
    runtime_directory: &Path,
) -> Result<bool, String> {
    let ffmpeg = source_directory.join("ffmpeg.exe");
    let ffprobe = source_directory.join("ffprobe.exe");
    if !ffmpeg.is_file() || !ffprobe.is_file() {
        return Ok(false);
    }
    materialize_runtime_file(&ffmpeg, &runtime_directory.join("ffmpeg.exe"))?;
    materialize_runtime_file(&ffprobe, &runtime_directory.join("ffprobe.exe"))?;
    Ok(true)
}

pub(crate) fn migrate_installed_ffmpeg_runtime() -> Result<bool, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let source = executable
        .parent()
        .ok_or("Application folder could not be resolved.")?;
    // Portable packages keep their sidecars beside the executable. Only an
    // installed build has an uninstaller and needs a runtime outside $INSTDIR.
    if !source.join("uninstall.exe").is_file() {
        return Ok(false);
    }
    let runtime = managed_runtime_dir().ok_or("Local application data folder is unavailable.")?;
    migrate_runtime_pair(source, &runtime)
}

pub(crate) fn allow_asset_file(app: &AppHandle, path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("Preview file was not found.".into());
    }
    app.asset_protocol_scope()
        .allow_file(path)
        .map_err(|error| format!("Preview access could not be granted: {error}"))
}

#[derive(Default)]
pub(crate) struct RuntimeMigrationState {
    pub(crate) error: Mutex<Option<String>>,
}

pub(crate) fn stop_process_tree(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/T", "/F"]);
        command.creation_flags(CREATE_NO_WINDOW);
        let _ = command.status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct FfmpegStatus {
    pub(crate) ready: bool,
    pub(crate) ffmpeg_version: Option<String>,
    pub(crate) ffprobe_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FfmpegCapabilities {
    pub(crate) vidstab: bool,
    pub(crate) subtitles: bool,
    pub(crate) overlay: bool,
    pub(crate) blur: bool,
    pub(crate) concat: bool,
}

pub(crate) async fn component_version(program: &str) -> Option<String> {
    let output = process_timeout::output_within(
        hidden_command(program).arg("-version"),
        process_timeout::VERSION_CHECK,
    )
    .await
    .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
}

#[tauri::command]
pub(crate) async fn ffmpeg_status() -> FfmpegStatus {
    let (ffmpeg_version, ffprobe_version) =
        tokio::join!(component_version("ffmpeg"), component_version("ffprobe"));
    FfmpegStatus {
        ready: ffmpeg_version.is_some() && ffprobe_version.is_some(),
        ffmpeg_version,
        ffprobe_version,
    }
}

pub(crate) async fn ffmpeg_filter_names() -> Result<String, String> {
    let output = process_timeout::output_within(
        hidden_command("ffmpeg").args(["-hide_banner", "-filters"]),
        process_timeout::VERSION_CHECK,
    )
    .await
    .map_err(|error| format!("FFmpeg capability check could not start: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) async fn ffmpeg_demuxer_names() -> Result<String, String> {
    let output = process_timeout::output_within(
        hidden_command("ffmpeg").args(["-hide_banner", "-demuxers"]),
        process_timeout::VERSION_CHECK,
    )
    .await
    .map_err(|error| format!("FFmpeg demuxer check could not start: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) fn filter_list_has(filters: &str, name: &str) -> bool {
    filters
        .lines()
        .any(|line| line.split_whitespace().nth(1) == Some(name))
}

pub(crate) fn demuxer_list_has(demuxers: &str, name: &str) -> bool {
    demuxers.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some("D")
            && fields
                .next()
                .is_some_and(|names| names.split(',').any(|value| value == name))
    })
}

#[tauri::command]
pub(crate) async fn ffmpeg_capabilities() -> Result<FfmpegCapabilities, String> {
    let (filters, demuxers) = tokio::try_join!(ffmpeg_filter_names(), ffmpeg_demuxer_names())?;
    Ok(FfmpegCapabilities {
        vidstab: filter_list_has(&filters, "vidstabdetect")
            && filter_list_has(&filters, "vidstabtransform"),
        subtitles: filter_list_has(&filters, "subtitles") || filter_list_has(&filters, "ass"),
        overlay: filter_list_has(&filters, "overlay"),
        blur: filter_list_has(&filters, "boxblur"),
        concat: demuxer_list_has(&demuxers, "concat"),
    })
}

pub(crate) fn record_runtime_migration(state: &RuntimeMigrationState) -> Result<bool, String> {
    match migrate_installed_ffmpeg_runtime() {
        Ok(migrated) => {
            *state
                .error
                .lock()
                .map_err(|_| "FFmpeg runtime state is unavailable.".to_string())? = None;
            Ok(migrated)
        }
        Err(error) => {
            *state
                .error
                .lock()
                .map_err(|_| "FFmpeg runtime state is unavailable.".to_string())? =
                Some(error.clone());
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) fn ffmpeg_runtime_error(
    state: State<'_, RuntimeMigrationState>,
) -> Result<Option<String>, String> {
    state
        .error
        .lock()
        .map(|error| error.clone())
        .map_err(|_| "FFmpeg runtime state is unavailable.".to_string())
}

#[tauri::command]
pub(crate) fn repair_ffmpeg_runtime(
    state: State<'_, RuntimeMigrationState>,
) -> Result<bool, String> {
    record_runtime_migration(&state)
}
