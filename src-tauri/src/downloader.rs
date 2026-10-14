//! The yt-dlp link downloader: URL checks, analysis, downloads and history.

use crate::*;

pub(crate) struct DownloadCleanup {
    pub(crate) output_dir: PathBuf,
    pub(crate) existing_temporary_files: HashSet<PathBuf>,
}

pub(crate) struct DownloadInfoCache {
    pub(crate) url: String,
    pub(crate) json: Vec<u8>,
    pub(crate) created: Instant,
}

#[derive(Debug, Serialize)]
pub(crate) struct DownloaderStatus {
    pub(crate) ready: bool,
    pub(crate) version: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DownloaderResult {
    pub(crate) output_dir: String,
    pub(crate) output_file: Option<String>,
    pub(crate) details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DownloadHistoryEntry {
    pub(crate) path: String,
    pub(crate) name: String,
}

#[derive(Default)]
pub(crate) struct DownloadHistoryState(pub(crate) Mutex<()>);

#[derive(Debug, Serialize)]
pub(crate) struct DownloadAnalysis {
    pub(crate) title: String,
    pub(crate) uploader: Option<String>,
    pub(crate) duration: Option<f64>,
    pub(crate) thumbnail_path: Option<String>,
    pub(crate) formats: Vec<DownloadFormat>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DownloadFormat {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) detail: String,
    pub(crate) kind: String,
    pub(crate) codec: String,
    pub(crate) rank: u64,
    pub(crate) height: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DownloaderProgress {
    pub(crate) job_id: Option<String>,
    pub(crate) percent: f64,
    pub(crate) downloaded: String,
    pub(crate) speed: String,
}

pub(crate) fn parse_downloader_progress(line: &str) -> Option<DownloaderProgress> {
    // A dedicated template avoids depending on yt-dlp's human-facing wording,
    // which changes between releases and can otherwise leave the UI stuck on
    // "connecting" while bytes are already being transferred.
    let fields = line
        .split_once("CONTAINER_PROGRESS:")?
        .1
        .split('|')
        .collect::<Vec<_>>();
    if fields.len() != 3 {
        return None;
    }
    let percent = fields[0]
        .trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .ok()?;
    Some(DownloaderProgress {
        job_id: None,
        percent: percent.clamp(0.0, 100.0),
        downloaded: fields[1].trim().to_string(),
        speed: fields[2].trim().to_string(),
    })
}

pub(crate) fn is_downloader_temporary_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.ends_with(".part")
        || name.ends_with(".ytdl")
        || name.ends_with(".temp")
        || name.contains(".part-frag")
}

pub(crate) fn downloader_temporary_files(output_dir: &Path) -> HashSet<PathBuf> {
    std::fs::read_dir(output_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_downloader_temporary_file(path))
        .collect()
}

pub(crate) fn cleanup_current_download(state: &JobState) {
    let cleanup = state
        .download_cleanup
        .lock()
        .ok()
        .and_then(|mut value| value.take());
    if let Some(cleanup) = cleanup {
        for path in downloader_temporary_files(&cleanup.output_dir) {
            if !cleanup.existing_temporary_files.contains(&path) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

pub(crate) fn materialize_bundled_downloader(
    source: &Path,
    destination: &Path,
) -> Result<bool, String> {
    if destination.is_file() || !source.is_file() {
        return Ok(false);
    }
    materialize_runtime_file(source, destination)?;
    downloader_runtime::mark_origin(destination, false);
    Ok(true)
}

pub(crate) fn downloader_binary_path() -> Result<PathBuf, String> {
    let root = dirs::data_local_dir()
        .ok_or("Application data folder is unavailable.")?
        .join(app_storage_namespace())
        .join("downloader");
    let destination = root.join(if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    });
    if !destination.is_file() {
        if let Some(source) = downloader_runtime::bundled_binary_path() {
            materialize_bundled_downloader(&source, &destination)
                .map_err(|error| format!("Bundled yt-dlp could not be prepared: {error}"))?;
        }
    }
    Ok(destination)
}

pub(crate) fn downloader_javascript_runtime_path() -> Result<PathBuf, String> {
    let directory = std::env::current_exe()
        .map_err(|error| format!("Application location is unavailable: {error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or("Application location is unavailable.")?;
    let path = directory.join(if cfg!(target_os = "windows") {
        "deno.exe"
    } else {
        "deno"
    });
    if !path.is_file() {
        return Err("The bundled YouTube JavaScript runtime is unavailable.".into());
    }
    Ok(path)
}

pub(crate) fn downloader_javascript_args(runtime: &Path) -> Vec<String> {
    vec![
        "--js-runtimes".into(),
        format!("deno:{}", runtime.to_string_lossy()),
    ]
}

pub(crate) fn downloader_site_args(url: &str) -> Vec<&'static str> {
    let is_tiktok = Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| host == "tiktok.com" || host.ends_with(".tiktok.com"));
    if is_tiktok {
        vec!["--impersonate", "chrome"]
    } else {
        Vec::new()
    }
}

pub(crate) fn downloader_output_dir() -> Result<PathBuf, String> {
    let root = dirs::download_dir().ok_or("Downloads folder is unavailable.")?;
    let folder = root.join("CONTAINER Downloads");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    Ok(folder)
}

pub(crate) fn download_history_path() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir().ok_or("Application data folder is unavailable.")?;
    migrate_download_history(
        &base
            .join(legacy_storage_name())
            .join("download-history.json"),
        &app_data_directory()?.join("download-history.json"),
    )
}

pub(crate) fn migrate_download_history(
    legacy: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
    // All callers hold DownloadHistoryState's lock; the single-instance plugin
    // serializes application processes. Never replace an existing new history.
    std::fs::create_dir_all(destination.parent().ok_or("Invalid history path")?)
        .map_err(|error| error.to_string())?;
    if !destination
        .try_exists()
        .map_err(|error| error.to_string())?
        && legacy.try_exists().map_err(|error| error.to_string())?
    {
        // Validate first. Rename keeps the original bytes, ordering and names;
        // a failed migration leaves the old history intact for a later retry.
        let metadata = std::fs::symlink_metadata(legacy).map_err(|error| error.to_string())?;
        let regular = metadata.is_file() && !metadata.file_type().is_symlink();
        #[cfg(target_os = "windows")]
        let regular = regular && metadata.file_attributes() & 0x400 == 0;
        if !regular {
            return Err("Legacy download history is not a regular file.".into());
        }
        read_download_history(legacy)?;
        std::fs::rename(legacy, destination)
            .map_err(|error| format!("Download history could not be moved: {error}"))?;
    }
    Ok(destination.to_path_buf())
}

pub(crate) fn valid_download_history_file(root: &Path, path: &Path) -> bool {
    if path.parent() != Some(root)
        || path.file_name().is_none()
        || is_downloader_temporary_file(path)
    {
        return false;
    }
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(target_os = "windows")]
    if metadata.file_attributes() & 0x400 != 0 {
        return false;
    }
    true
}

pub(crate) fn read_download_history(path: &Path) -> Result<Vec<DownloadHistoryEntry>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err("Download history is invalid or too large.".into());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Download history could not be read: {error}"))
}

pub(crate) fn write_download_history(
    path: &Path,
    entries: &[DownloadHistoryEntry],
) -> Result<(), String> {
    let bytes = serde_json::to_vec(entries).map_err(|error| error.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Download history is too large.".into());
    }
    let pending = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = std::fs::write(&pending, bytes).and_then(|()| std::fs::rename(&pending, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result.map_err(|error| format!("Download history could not be saved: {error}"))
}

pub(crate) fn existing_download_history(
    root: &Path,
    entries: Vec<DownloadHistoryEntry>,
) -> Vec<DownloadHistoryEntry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|entry| {
            let path = Path::new(&entry.path);
            valid_download_history_file(root, path) && seen.insert(entry.path.clone())
        })
        .collect()
}

pub(crate) fn initial_download_history(root: &Path) -> Result<Vec<DownloadHistoryEntry>, String> {
    let mut files = std::fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| valid_download_history_file(root, path))
        .collect::<Vec<_>>();
    files.sort_by_key(|path| {
        std::cmp::Reverse(
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok(),
        )
    });
    Ok(files
        .into_iter()
        .map(|path| DownloadHistoryEntry {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
        })
        .collect())
}

#[tauri::command]
pub(crate) fn list_download_history(
    history: State<'_, DownloadHistoryState>,
) -> Result<Vec<DownloadHistoryEntry>, String> {
    let _guard = history
        .0
        .lock()
        .map_err(|_| "Download history lock failed")?;
    let root = downloader_output_dir()?;
    let path = download_history_path()?;
    if !path.exists() {
        let initial = initial_download_history(&root)?;
        write_download_history(&path, &initial)?;
        return Ok(initial);
    }
    let entries = read_download_history(&path)?;
    let existing = existing_download_history(&root, entries.clone());
    if existing.len() != entries.len() {
        write_download_history(&path, &existing)?
    }
    Ok(existing)
}

pub(crate) fn remember_download(path: &Path, history: &DownloadHistoryState) -> Result<(), String> {
    let _guard = history
        .0
        .lock()
        .map_err(|_| "Download history lock failed")?;
    let root = downloader_output_dir()?;
    if !valid_download_history_file(&root, path) {
        return Err("Downloaded file could not be verified.".into());
    }
    let history_path = download_history_path()?;
    let entries = existing_download_history(&root, read_download_history(&history_path)?);
    let mut updated = vec![DownloadHistoryEntry {
        path: path.to_string_lossy().into_owned(),
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
    }];
    updated.extend(
        entries
            .into_iter()
            .filter(|entry| Path::new(&entry.path) != path),
    );
    write_download_history(&history_path, &updated)
}

#[tauri::command]
pub(crate) fn delete_download_history_entry(
    path: String,
    history: State<'_, DownloadHistoryState>,
    state: State<'_, JobState>,
) -> Result<(), String> {
    if state
        .pid
        .lock()
        .map_err(|_| "Job state is unavailable")?
        .is_some()
    {
        return Err("Wait for the current job to finish before removing a download.".into());
    }
    let _guard = history
        .0
        .lock()
        .map_err(|_| "Download history lock failed")?;
    let root = downloader_output_dir()?;
    let file = PathBuf::from(&path);
    let history_path = download_history_path()?;
    delete_download_history_file(&root, &history_path, &file)
}

pub(crate) fn delete_download_history_file(
    root: &Path,
    history_path: &Path,
    file: &Path,
) -> Result<(), String> {
    let path = file.to_string_lossy();
    let entries = read_download_history(history_path)?;
    if !entries.iter().any(|entry| entry.path == path) {
        return Err("This file is not in download history.".into());
    }
    if valid_download_history_file(root, file) {
        trash::delete(file)
            .map_err(|error| format!("File could not be moved to the Recycle Bin: {error}"))?;
    } else if file.exists() {
        return Err("Download path is not a regular file in the download folder.".into());
    }
    let remaining = existing_download_history(
        root,
        entries
            .into_iter()
            .filter(|entry| entry.path != path)
            .collect(),
    );
    write_download_history(history_path, &remaining)
}

pub(crate) async fn downloader_version_at(binary: &Path) -> Option<String> {
    downloader_version_within(binary, std::time::Duration::from_secs(5)).await
}

pub(crate) async fn downloader_version_within(
    binary: &Path,
    limit: std::time::Duration,
) -> Option<String> {
    if !binary.is_file() {
        return None;
    }
    let mut command = hidden_path_command(binary);
    command
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn().ok()?;
    let output = tokio::time::timeout(limit, child.wait_with_output())
        .await
        .ok()?
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

pub(crate) async fn downloader_version() -> Option<String> {
    let binary = downloader_binary_path().ok()?;
    downloader_version_at(&binary).await
}

#[tauri::command]
pub(crate) async fn downloader_status() -> DownloaderStatus {
    let version = downloader_version().await;
    DownloaderStatus {
        ready: version.is_some() && downloader_javascript_runtime_path().is_ok(),
        version,
    }
}

#[tauri::command]
pub(crate) async fn set_downloader_binary(path: String) -> Result<DownloaderStatus, String> {
    let source = PathBuf::from(path);
    let metadata =
        std::fs::metadata(&source).map_err(|_| "The selected yt-dlp file was not found.")?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("The selected yt-dlp file is not valid.".into());
    }
    #[cfg(target_os = "windows")]
    if source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("exe"))
        != Some(true)
    {
        return Err("Select a yt-dlp executable file.".into());
    }
    let destination = downloader_binary_path()?;
    let parent = destination
        .parent()
        .ok_or("Downloader folder is unavailable.")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = destination.with_extension("download");
    std::fs::copy(&source, &temporary)
        .map_err(|error| format!("yt-dlp could not be copied: {error}"))?;
    let version = match downloader_version_at(&temporary).await {
        Some(version) => version,
        None => {
            let _ = std::fs::remove_file(&temporary);
            return Err(
                "The selected file did not behave like yt-dlp. Choose a valid yt-dlp executable."
                    .into(),
            );
        }
    };
    let backup = destination.with_extension("backup");
    let _ = std::fs::remove_file(&backup);
    let had_existing = destination.is_file();
    if had_existing {
        std::fs::rename(&destination, &backup)
            .map_err(|error| format!("The existing yt-dlp could not be replaced: {error}"))?;
    }
    if let Err(error) = std::fs::rename(&temporary, &destination) {
        if had_existing {
            let _ = std::fs::rename(&backup, &destination);
        }
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("yt-dlp could not be installed: {error}"));
    }
    let _ = std::fs::remove_file(backup);
    downloader_runtime::mark_origin(&destination, true);
    Ok(DownloaderStatus {
        ready: true,
        version: Some(version),
    })
}

pub(crate) fn validated_download_url(value: &str) -> Result<String, String> {
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err("Enter a valid HTTPS video link.".into());
    }
    let parsed = Url::parse(value.trim()).map_err(|_| "Enter a valid HTTPS video link.")?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
        || !is_public_url_host(&parsed)
    {
        return Err("Only regular HTTPS video links are accepted.".into());
    }
    Ok(parsed.into())
}

pub(crate) fn is_public_ipv4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !ip.is_private()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_broadcast()
        && !ip.is_unspecified()
        && !ip.is_multicast()
        // 0.0.0.0/8 ("this network") and 100.64.0.0/10 (carrier-grade NAT).
        && a != 0
        && !(a == 100 && (64..128).contains(&b))
        // 198.18.0.0/15 benchmarking range, often used for internal gear.
        && !(a == 198 && (b == 18 || b == 19))
}

pub(crate) fn is_public_url_host(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.to_ascii_lowercase();
            let domain = domain.trim_end_matches('.');
            domain != "localhost" && !domain.ends_with(".localhost") && !domain.ends_with(".local")
        }
        Some(Host::Ipv4(ip)) => is_public_ipv4(ip),
        Some(Host::Ipv6(ip)) => {
            // IPv4-mapped (::ffff:a.b.c.d) and NAT64 (64:ff9b::a.b.c.d) addresses
            // reach the embedded IPv4 host, so judge them by that address.
            let segments = ip.segments();
            if let Some(v4) = ip.to_ipv4_mapped() {
                return is_public_ipv4(v4);
            }
            if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                let [.., hi, lo] = segments;
                return is_public_ipv4(std::net::Ipv4Addr::new(
                    (hi >> 8) as u8,
                    hi as u8,
                    (lo >> 8) as u8,
                    lo as u8,
                ));
            }
            !ip.is_loopback()
                && !ip.is_unique_local()
                && !ip.is_unicast_link_local()
                && !ip.is_unspecified()
                && !ip.is_multicast()
        }
        None => false,
    }
}

pub(crate) fn valid_download_format_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 40
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

pub(crate) fn downloader_format_args(format: &str) -> Result<Vec<String>, String> {
    match format {
        "audio" | "audio-format:m4a" => Ok(vec![
            "-f".into(),
            "bestaudio/best".into(),
            "-x".into(),
            "--audio-format".into(),
            "m4a".into(),
        ]),
        "audio-format:opus" => Ok(vec![
            "-f".into(),
            "bestaudio/best".into(),
            "-x".into(),
            "--audio-format".into(),
            "opus".into(),
        ]),
        "1080" => Ok(vec![
            "-f".into(),
            "bv*[height<=1080]+ba/b[height<=1080]/best".into(),
        ]),
        "best" => Ok(vec!["-f".into(), "bv*+ba/best".into()]),
        audio if audio.starts_with("audio:") => {
            let id = audio.trim_start_matches("audio:");
            if !valid_download_format_id(id) {
                return Err("Unsupported download format.".into());
            }
            Ok(vec![
                "-f".into(),
                id.into(),
                "-x".into(),
                "--audio-format".into(),
                "m4a".into(),
            ])
        }
        id if valid_download_format_id(id) => Ok(vec!["-f".into(), format!("{id}+bestaudio/{id}")]),
        _ => Err("Unsupported download format.".into()),
    }
}

pub(crate) fn download_thumbnail_path(url: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "container-download-thumbnail-{}-{:x}.jpg",
        std::process::id(),
        Sha256::digest(url.as_bytes())
    ))
}

pub(crate) fn remove_download_thumbnail_file(path: &Path) -> Result<bool, String> {
    let expected_parent = std::env::temp_dir();
    let valid_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.starts_with("container-download-thumbnail-") && name.ends_with(".jpg")
        });
    if path.parent() != Some(expected_parent.as_path()) || !valid_name {
        return Err("Invalid temporary thumbnail path.".into());
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("Temporary thumbnail could not be removed: {error}")),
    }
}

pub(crate) fn cleanup_legacy_download_thumbnail_cache() {
    let Some(directory) =
        dirs::cache_dir().map(|path| path.join(legacy_storage_name()).join("download-thumbnails"))
    else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let _ = std::fs::remove_dir(directory);
}

#[tauri::command]
pub(crate) fn remove_download_thumbnail(path: String) -> Result<bool, String> {
    remove_download_thumbnail_file(Path::new(&path))
}

pub(crate) async fn cache_download_thumbnail(app: &AppHandle, url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" || !is_public_url_host(&parsed) {
        return None;
    }
    let response = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 {
                attempt.stop()
            } else if attempt.url().scheme() == "https" && is_public_url_host(attempt.url()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .ok()?
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    if !response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("image/"))
    {
        return None;
    }
    if response.content_length().unwrap_or(0) > 5 * 1024 * 1024 {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() > 5 * 1024 * 1024 {
        return None;
    }
    let path = download_thumbnail_path(url);
    std::fs::write(&path, bytes).ok()?;
    allow_asset_file(app, &path).ok()?;
    Some(path.to_string_lossy().to_string())
}

#[tauri::command]
pub(crate) async fn analyze_download_url(
    app: AppHandle,
    state: State<'_, JobState>,
    url: String,
) -> Result<DownloadAnalysis, String> {
    let url = validated_download_url(&url)?;
    let binary = downloader_binary_path()?;
    if !binary.is_file() || downloader_version().await.is_none() {
        return Err("yt-dlp is not ready. Select the official yt-dlp executable first.".into());
    }
    let javascript_runtime = downloader_javascript_runtime_path()?;
    let mut command = hidden_path_command(&binary);
    command
        .args(["--ignore-config", "--no-plugin-dirs", "-J", "--no-playlist"])
        .args(downloader_javascript_args(&javascript_runtime))
        .args(downloader_site_args(&url))
        .arg(&url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command
        .spawn()
        .map_err(|error| format!("yt-dlp could not start: {error}"))?;
    let output = tokio::time::timeout(std::time::Duration::from_secs(45), child.wait_with_output())
        .await
        .map_err(|_| "Link analysis timed out.")?
        .map_err(|error| format!("yt-dlp analysis failed: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .lines()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(if detail.is_empty() {
            "Link analysis failed.".into()
        } else {
            detail
        });
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "yt-dlp returned invalid media information.")?;
    // TikTok extraction can intermittently fail for the same public URL. Keep
    // the successful analysis briefly in memory so starting the download does
    // not have to extract the page a second time. Nothing is persisted.
    if !downloader_site_args(&url).is_empty() {
        *state
            .download_info
            .lock()
            .map_err(|_| "Download analysis state lock failed")? = Some(DownloadInfoCache {
            url: url.clone(),
            json: output.stdout.clone(),
            created: Instant::now(),
        });
    }
    let title = metadata
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or("This link did not contain a downloadable title.")?
        .trim()
        .to_string();
    let thumbnail = metadata
        .get("thumbnail")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("https://"))
        .map(str::to_string);
    let mut formats = metadata
        .get("formats")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let id = entry.get("format_id")?.as_str()?.to_string();
                    let audio = entry
                        .get("acodec")
                        .and_then(Value::as_str)
                        .filter(|value| *value != "none")
                        .is_some();
                    let video = entry
                        .get("vcodec")
                        .and_then(Value::as_str)
                        .filter(|value| *value != "none")
                        .is_some();
                    if !audio && !video {
                        return None;
                    }
                    let label = if video {
                        format!(
                            "{}×{}",
                            entry.get("width").and_then(Value::as_u64).unwrap_or(0),
                            entry.get("height").and_then(Value::as_u64).unwrap_or(0)
                        )
                    } else {
                        entry
                            .get("ext")
                            .and_then(Value::as_str)
                            .unwrap_or("audio")
                            .to_uppercase()
                    };
                    let codec = if video {
                        entry
                            .get("vcodec")
                            .and_then(Value::as_str)
                            .unwrap_or("video")
                    } else {
                        entry
                            .get("acodec")
                            .and_then(Value::as_str)
                            .unwrap_or("audio")
                    };
                    let size = entry
                        .get("filesize_approx")
                        .or_else(|| entry.get("filesize"))
                        .and_then(Value::as_f64)
                        .map(|value| format!("{:.1} MB", value / 1_048_576.0))
                        .unwrap_or_else(|| "size unknown".into());
                    Some(DownloadFormat {
                        id,
                        label,
                        detail: format!("{} · {}", codec, size),
                        kind: if video {
                            "video".into()
                        } else {
                            "audio".into()
                        },
                        codec: codec.to_string(),
                        rank: if video {
                            entry.get("height").and_then(Value::as_u64).unwrap_or(0) * 1_000
                                + entry.get("fps").and_then(Value::as_u64).unwrap_or(0)
                        } else {
                            entry.get("abr").and_then(Value::as_u64).unwrap_or(0)
                        },
                        height: if video {
                            entry.get("height").and_then(Value::as_u64)
                        } else {
                            None
                        },
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    formats.sort_by_key(|item| std::cmp::Reverse(item.rank));
    formats.truncate(40);
    Ok(DownloadAnalysis {
        title,
        uploader: metadata
            .get("uploader")
            .or_else(|| metadata.get("channel"))
            .and_then(Value::as_str)
            .map(str::to_string),
        duration: metadata.get("duration").and_then(Value::as_f64),
        thumbnail_path: cache_download_thumbnail(&app, thumbnail.as_deref()).await,
        formats,
    })
}

#[tauri::command]
pub(crate) async fn download_media(
    app: AppHandle,
    state: State<'_, JobState>,
    history: State<'_, DownloadHistoryState>,
    url: String,
    format: String,
    job_id: Option<String>,
    section: Option<download_section::SectionRequest>,
) -> Result<DownloaderResult, String> {
    let _job = JobLease::acquire(&state, job_id)?;
    let url = validated_download_url(&url)?;
    let binary = downloader_binary_path()?;
    if !binary.is_file() || downloader_version().await.is_none() {
        return Err("yt-dlp is not ready. Select the official yt-dlp executable first.".into());
    }
    let javascript_runtime = downloader_javascript_runtime_path()?;
    let output_dir = downloader_output_dir()?;
    let ffmpeg = resolve_program("ffmpeg");
    let ffmpeg_directory = ffmpeg
        .parent()
        .filter(|path| path.is_dir())
        .ok_or("Bundled FFmpeg folder is unavailable.")?;
    let format_args = downloader_format_args(&format)?;
    let cached_info = state
        .download_info
        .lock()
        .map_err(|_| "Download analysis state lock failed")?
        .as_ref()
        .filter(|info| info.url == url && info.created.elapsed() < Duration::from_secs(10 * 60))
        .map(|info| info.json.clone());
    let known_duration = cached_info
        .as_deref()
        .and_then(|json| serde_json::from_slice::<Value>(json).ok())
        .and_then(|info| info.get("duration").and_then(Value::as_f64));
    let section = download_section::download_section(section, known_duration)?;
    reset_legacy_job_cancellation(&state);
    let existing_temporary_files = downloader_temporary_files(&output_dir);
    if state.cancelled.load(Ordering::Relaxed) {
        return Err("Job cancelled.".into());
    }
    let mut command = hidden_path_command(&binary);
    command
        .args([
            "--ignore-config",
            "--no-plugin-dirs",
            "--no-playlist",
            "--restrict-filenames",
            "--windows-filenames",
            "--newline",
            "-o",
            download_section::output_template(section),
            // --print enables quiet mode in yt-dlp. Explicitly restore progress
            // output so the UI receives live transfer information.
            "--progress",
            "--progress-template",
            "download:CONTAINER_PROGRESS:%(progress._percent_str)s|%(progress._downloaded_bytes_str)s|%(progress._speed_str)s",
        ])
        .args(downloader_javascript_args(&javascript_runtime))
        // `-P` consumes its next token. It must follow the runtime option or
        // yt-dlp treats `deno:...` as the URL to download.
        .arg("-P")
        .arg(&output_dir)
        .arg("--ffmpeg-location")
        .arg(ffmpeg_directory)
        .args(&format_args)
        .args(download_section::section_args(section))
        .arg("--print")
        .arg("after_move:filepath");
    if cached_info.is_some() {
        command
            .args(["--load-info-json", "-"])
            .stdin(Stdio::piped());
    } else {
        command.args(downloader_site_args(&url)).arg(&url);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("yt-dlp could not start: {error}"))?;
    if let Some(json) = cached_info {
        let mut stdin = child
            .stdin
            .take()
            .ok_or("yt-dlp input stream is unavailable.")?;
        stdin
            .write_all(&json)
            .await
            .map_err(|error| format!("yt-dlp analysis could not be reused: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("yt-dlp input stream could not be closed: {error}"))?;
    }
    *state
        .download_cleanup
        .lock()
        .map_err(|_| "Download cleanup state lock failed")? = Some(DownloadCleanup {
        output_dir: output_dir.clone(),
        existing_temporary_files,
    });
    let active_process = ActiveProcess::register(&state, child.id())?;
    let stdout = child
        .stdout
        .take()
        .ok_or("yt-dlp output stream is unavailable.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("yt-dlp progress stream is unavailable.")?;
    let (stream_tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let stdout_tx = stream_tx.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if stdout_tx.send((true, line)).is_err() {
                break;
            }
        }
    });
    tokio::spawn(download_section::forward_lines(stderr, stream_tx, false));
    let mut output_lines = line_tail::LineTail::new(line_tail::DEFAULT_LINES);
    let mut error_lines = line_tail::LineTail::new(line_tail::DEFAULT_LINES);
    while let Some((is_stdout, line)) = stream_rx.recv().await {
        if state.cancelled.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            break;
        }
        let section_status =
            section.and_then(|(start, end)| download_section::section_progress(&line, end - start));
        if let Some(mut progress) = parse_downloader_progress(&line).or(section_status) {
            progress.job_id = progress_job_id(&state);
            let _ = app.emit("downloader-progress", progress);
        } else if is_stdout {
            output_lines.push(line);
        } else if !line.trim().is_empty() {
            error_lines.push(line);
        }
    }
    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            cleanup_current_download(&state);
            return Err(error.to_string());
        }
    };
    drop(active_process);
    if state.cancelled.load(Ordering::Relaxed) {
        cleanup_current_download(&state);
        return Err("Download cancelled.".into());
    }
    if !status.success() {
        cleanup_current_download(&state);
        let detail = error_lines
            .into_vec()
            .into_iter()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(if detail.is_empty() {
            "Download failed.".into()
        } else {
            detail
        });
    }
    if let Ok(mut cleanup) = state.download_cleanup.lock() {
        cleanup.take();
    }
    let output_lines = output_lines.into_vec();
    let details = output_lines
        .last()
        .map(String::as_str)
        .unwrap_or("Download finished.")
        .to_string();
    let output_file = output_lines
        .iter()
        .rev()
        .find(|line| {
            let path = Path::new(line.as_str());
            path.parent() == Some(output_dir.as_path()) && path.is_file()
        })
        .cloned();
    if let Some(path) = &output_file {
        if let Err(error) = remember_download(Path::new(path), &history) {
            app_log::warn(&format!(
                "Download finished but history could not be updated: {error}"
            ));
        }
    }
    Ok(DownloaderResult {
        output_dir: output_dir.to_string_lossy().to_string(),
        output_file,
        details,
    })
}
