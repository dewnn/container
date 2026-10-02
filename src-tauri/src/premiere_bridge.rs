//! Opt-in CEP bridge. Same-user file mailbox, no ports and no executable payloads.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const OWNER: &str = "CONTAINER Premiere Bridge 1";
const ASSETS: &[(&str, &str)] = &[
    (
        "CSXS/manifest.xml",
        include_str!("../premiere/manifest.xml"),
    ),
    ("index.html", include_str!("../premiere/index.html")),
    ("host.jsx", include_str!("../premiere/host.jsx")),
    ("bridge.js", include_str!("../premiere/bridge.js")),
];

pub struct PremiereBridge {
    session: String,
    transfer: Arc<Mutex<()>>,
    files: Arc<Mutex<()>>,
}
impl Default for PremiereBridge {
    fn default() -> Self {
        Self {
            session: identifier("session"),
            transfer: Arc::new(Mutex::new(())),
            files: Arc::new(Mutex::new(())),
        }
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn identifier(label: &str) -> String {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{label}:{serial}:{}:{}:{:?}",
                std::process::id(),
                now(),
                std::time::Instant::now()
            )
            .as_bytes()
        )
    )
}
fn root() -> Result<PathBuf, String> {
    let path = dirs::config_dir()
        .ok_or("User configuration directory unavailable")?
        .join("dev.dean.container")
        .join("premiere-bridge");
    regular_path(&path)?;
    regular_path(path.parent().ok_or("Bridge parent unavailable")?)?;
    Ok(path)
}
fn extension() -> Result<PathBuf, String> {
    let path = dirs::config_dir()
        .ok_or("User configuration directory unavailable")?
        .join("Adobe/CEP/extensions/dev.dean.container.premiere");
    regular_path(&path)?;
    regular_path(&path.join("CSXS"))?;
    Ok(path)
}
fn regular_path(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err(
                        "Bridge path is a Windows reparse point; it was not changed.".into(),
                    );
                }
            }
            if meta.file_type().is_symlink() {
                return Err("Bridge path is a symbolic link; it was not changed.".into());
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    regular_path(path)?;
    // Reader retries incomplete reads. No partial file is ever treated as a command.
    std::fs::write(path, serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> Option<T> {
    read_json_limited(path, 32_768)
}
fn read_json_limited<T: for<'a> Deserialize<'a>>(path: &Path, limit: u64) -> Option<T> {
    if std::fs::metadata(path).ok()?.len() > limit {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}
fn installed() -> bool {
    extension().ok().is_some_and(|p| {
        std::fs::read_to_string(p.join("container-owner.txt"))
            .ok()
            .as_deref()
            == Some(OWNER)
    })
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct BridgeStatus {
    #[serde(default)]
    pub protocol: u32,
    pub supported: bool,
    pub installed: bool,
    pub connected: bool,
    pub project: String,
    pub sequence_id: String,
    pub sequence_name: String,
}
#[derive(Deserialize)]
struct Heartbeat {
    #[serde(default)]
    protocol: u32,
    session: String,
    updated: u64,
    connected: bool,
    project: String,
    sequence_id: String,
    sequence_name: String,
}
fn status_at(path: &Path, session: &str, time: u64) -> BridgeStatus {
    let mut result = BridgeStatus {
        supported: cfg!(target_os = "windows"),
        installed: true,
        ..Default::default()
    };
    if let Some(beat) = read_json::<Heartbeat>(&path.join("heartbeat.json")) {
        if beat.session == session
            && beat.updated <= time + 1000
            && time.saturating_sub(beat.updated) <= 5000
            && beat.connected
        {
            result.connected = true;
            result.protocol = beat.protocol;
            result.project = beat.project;
            result.sequence_id = beat.sequence_id;
            result.sequence_name = beat.sequence_name;
        }
    }
    result
}
#[tauri::command]
pub fn premiere_status(state: tauri::State<'_, PremiereBridge>) -> Result<BridgeStatus, String> {
    if !cfg!(target_os = "windows") || !installed() {
        return Ok(BridgeStatus {
            supported: cfg!(target_os = "windows"),
            ..Default::default()
        });
    }
    // This short synchronous command must not block the WebView thread while
    // the background Recycle Bin operation holds the file lock.
    let _lock = state
        .files
        .try_lock()
        .map_err(|_| "Bridge files are busy")?;
    let path = root()?;
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    write_json(
        &path.join("session.json"),
        &serde_json::json!({"id": state.session, "updated": now()}),
    )?;
    Ok(status_at(&path, &state.session, now()))
}

#[cfg(target_os = "windows")]
#[derive(Serialize, Deserialize)]
struct DebugBackup {
    bytes: Vec<u8>,
    kind: u32,
}

#[tauri::command]
pub fn premiere_install(
    state: tauri::State<'_, PremiereBridge>,
    consent: bool,
) -> Result<(), String> {
    if !consent {
        return Err("Explicit consent is required to install the Premiere bridge and enable unsigned CEP extensions.".into());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        Err("Premiere integration currently supports Windows only.".into())
    }
    #[cfg(target_os = "windows")]
    {
        use winreg::{enums::*, RegKey};
        let _lock = state.files.lock().map_err(|_| "Bridge unavailable")?;
        let folder = extension()?;
        if folder.exists() && !installed() {
            return Err("An unrecognized extension already occupies the CONTAINER bridge directory. It was not changed.".into());
        }
        std::fs::create_dir_all(folder.join("CSXS")).map_err(|e| e.to_string())?;
        regular_path(&folder.join("container-owner.txt"))?;
        std::fs::write(folder.join("container-owner.txt"), OWNER).map_err(|e| e.to_string())?;
        for (name, body) in ASSETS {
            regular_path(&folder.join(name))?;
            std::fs::write(folder.join(name), body).map_err(|e| e.to_string())?;
        }
        let config = root()?;
        std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey("Software\\Adobe\\CSXS.12")
            .map_err(|e| e.to_string())?;
        let backup = config.join("debug-backup.json");
        if !backup.exists() {
            let previous = match key.get_raw_value("PlayerDebugMode") {
                Ok(value) => Some(DebugBackup {
                    bytes: value.bytes,
                    kind: value.vtype as u32,
                }),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            };
            write_json(&backup, &previous)?;
        } else if read_json::<Option<DebugBackup>>(&backup).is_none() {
            return Err("Previous Adobe debug setting backup is unreadable. No registry setting was changed.".into());
        }
        key.set_value("PlayerDebugMode", &"1")
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
pub fn premiere_remove(state: tauri::State<'_, PremiereBridge>) -> Result<(), String> {
    let _transfer = state
        .transfer
        .try_lock()
        .map_err(|_| "Wait for the Premiere transfer to finish.")?;
    let _lock = state.files.lock().map_err(|_| "Bridge unavailable")?;
    if !installed() {
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        use winreg::{enums::*, RegKey, RegValue};
        let config = root()?;
        let previous = read_json::<Option<DebugBackup>>(&config.join("debug-backup.json"))
            .ok_or("Previous Adobe debug setting backup is unreadable; bridge was not removed.")?;
        {
            let key = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags("Software\\Adobe\\CSXS.12", KEY_READ | KEY_WRITE)
                .map_err(|e| e.to_string())?;
            // Do not overwrite a value changed by another program since installation.
            if key
                .get_value::<String, _>("PlayerDebugMode")
                .ok()
                .as_deref()
                == Some("1")
            {
                if let Some(value) = previous {
                    let kind = match value.kind {
                        1 => REG_SZ,
                        2 => REG_EXPAND_SZ,
                        3 => REG_BINARY,
                        4 => REG_DWORD,
                        7 => REG_MULTI_SZ,
                        11 => REG_QWORD,
                        _ => {
                            return Err(
                                "Unknown previous Adobe setting; bridge was not removed.".into()
                            )
                        }
                    };
                    key.set_raw_value(
                        "PlayerDebugMode",
                        &RegValue {
                            bytes: value.bytes,
                            vtype: kind,
                        },
                    )
                    .map_err(|e| e.to_string())?;
                } else {
                    key.delete_value("PlayerDebugMode")
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        let folder = extension()?;
        // Delete only our explicit files. No recursive removal of Adobe directories.
        for (name, _) in ASSETS {
            let file = folder.join(name);
            if file.exists() {
                std::fs::remove_file(file).map_err(|e| e.to_string())?;
            }
        }
        std::fs::remove_file(folder.join("container-owner.txt")).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir(folder.join("CSXS"));
        let _ = std::fs::remove_dir(&folder);
        for name in [
            "session.json",
            "heartbeat.json",
            "request.json",
            "response.json",
            "debug-backup.json",
        ] {
            let _ = std::fs::remove_file(config.join(name));
        }
    }
    Ok(())
}

fn owned_project_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let Some(body) = name
        .strip_prefix("CONTAINER-")
        .and_then(|s| s.strip_suffix(".prproj"))
    else {
        return false;
    };
    let Some((prefix, id)) = body.rsplit_once('-') else {
        return false;
    };
    let time = prefix.rsplit_once('-').map_or(prefix, |(_, time)| time);
    time.len() == 13
        && time.bytes().all(|c| c.is_ascii_digit())
        && id.len() == 12
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn project_filename(output: &Path, time: u64, id: &str) -> String {
    let stem: String = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Video")
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(80)
        .collect();
    let stem = stem.trim().trim_matches('.');
    format!(
        "CONTAINER-{}-{time}-{}.prproj",
        if stem.is_empty() { "Video" } else { stem },
        &id[..12]
    )
}

#[tauri::command]
pub async fn premiere_clean_projects(
    state: tauri::State<'_, PremiereBridge>,
) -> Result<usize, String> {
    let transfer = state.transfer.clone();
    let files = state.files.clone();
    tauri::async_runtime::spawn_blocking(move || clean_projects(&transfer, &files))
        .await
        .map_err(|e| e.to_string())?
}

fn clean_projects(transfer: &Mutex<()>, files: &Mutex<()>) -> Result<usize, String> {
    let _transfer = transfer
        .try_lock()
        .map_err(|_| "Wait for the Premiere transfer or cleanup to finish.")?;
    let _files = files.lock().map_err(|_| "Bridge unavailable")?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("tasklist.exe")
            .args([
                "/FI",
                "IMAGENAME eq Adobe Premiere Pro.exe",
                "/FO",
                "CSV",
                "/NH",
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("Could not verify Premiere is closed.".into());
        }
        if String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains("adobe premiere pro.exe")
        {
            return Err("Close Premiere before cleaning project files. / Proje dosyalarını temizlemeden önce Premiere’i kapat.".into());
        }
    }
    #[cfg(not(target_os = "windows"))]
    return Err("Premiere integration currently supports Windows only.".into());
    #[cfg(target_os = "windows")]
    {
        let folder = root()?;
        let ledger = folder.join("created-projects.json");
        regular_path(&ledger)?;
        let mut paths: Vec<String> = if ledger.exists() {
            read_json_limited(&ledger, 4 * 1024 * 1024)
                .ok_or("Project ownership list is unreadable; nothing was deleted.")?
        } else {
            Vec::new()
        };
        // Compatibility with projects created before the ownership ledger existed.
        let legacy = dirs::document_dir()
            .ok_or("Documents folder unavailable")?
            .join("Adobe/Premiere Pro/26.0");
        for ancestor in legacy.ancestors() {
            regular_path(ancestor)?;
        }
        if legacy.exists() {
            for entry in std::fs::read_dir(&legacy).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                // Only the old exact timestamp-only scheme is inferred without a ledger.
                if owned_project_name(&path)
                    && path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n.len() == 43)
                {
                    paths.push(path.to_string_lossy().into_owned());
                }
            }
        }
        paths.sort();
        paths.dedup();
        let mut targets = Vec::new();
        for value in &paths {
            let path = PathBuf::from(value);
            if !path.is_absolute() || !owned_project_name(&path) {
                return Err("Invalid owned project entry; nothing was deleted.".into());
            }
            if !path.exists() {
                continue;
            }
            // Reject junctions and symbolic links anywhere in the target's ancestry.
            for ancestor in path.ancestors() {
                regular_path(ancestor)?;
            }
            if !path.is_file() {
                return Err("Project target is not a regular file; nothing was deleted.".into());
            }
            targets.push(path);
        }
        // Windows may report an aborted operation even after moving all targets.
        // Never retry or permanently delete: reconcile the actual remaining files.
        let recycle_error = if targets.is_empty() {
            None
        } else {
            trash::delete_all(&targets).err().map(|e| e.to_string())
        };
        let (removed, remaining) = cleanup_remaining(&targets, |path| path.try_exists())?;
        let pending: Vec<String> = remaining
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        if folder.exists() {
            write_json(&ledger, &pending).map_err(|e| format!("{removed} project files were removed, but the remaining-project list could not be updated: {e}"))?;
        }
        if !remaining.is_empty() {
            let detail =
                recycle_error.unwrap_or_else(|| "Windows left these files in place.".into());
            return Err(format!("{removed} project files removed; {} could not be moved to Recycle Bin and remain listed for retry. Close apps using these files and try again. / {removed} proje kaldırıldı; {} proje taşınamadı ve tekrar denemek için listede tutuldu. Dosyaları kullanan uygulamaları kapat. Details: {detail}", remaining.len(), remaining.len()));
        }
        Ok(removed)
    }
}

fn cleanup_remaining(
    targets: &[PathBuf],
    mut exists: impl FnMut(&Path) -> std::io::Result<bool>,
) -> Result<(usize, Vec<PathBuf>), String> {
    let mut remaining = Vec::new();
    for path in targets {
        // An inaccessible path is not evidence of successful removal.
        if exists(path).map_err(|e| {
            format!("Could not verify project cleanup; ownership list was retained: {e}")
        })? {
            remaining.push(path.clone());
        }
    }
    Ok((targets.len() - remaining.len(), remaining))
}

fn validate_output(path: &Path, output_root: &Path) -> Result<PathBuf, String> {
    let path = dunce::canonicalize(path).map_err(|_| "Rendered file is missing.")?;
    let output_root =
        dunce::canonicalize(output_root).map_err(|_| "CONTAINER output folder is missing.")?;
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !path.is_file()
        || !path.starts_with(output_root)
        || !matches!(
            extension.as_str(),
            "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v"
        )
    {
        return Err("Only rendered videos inside CONTAINER Output can be sent to Premiere.".into());
    }
    Ok(path)
}
#[derive(Serialize, Deserialize)]
struct TransferResult {
    #[serde(default)]
    ownership_record_failed: bool,
    #[serde(default)]
    project_id: String,
    #[serde(default)]
    completed_at: u64,
    session: String,
    id: String,
    ok: bool,
    code: String,
}

#[tauri::command]
pub async fn premiere_send(
    state: tauri::State<'_, PremiereBridge>,
    path: String,
    project: String,
    sequence_id: String,
) -> Result<(), String> {
    if !cfg!(target_os = "windows") || !installed() {
        return Err("Premiere bridge is not installed.".into());
    }
    let session = state.session.clone();
    let transfer = state.transfer.clone();
    tauri::async_runtime::spawn_blocking(move || {
    // A single outstanding command is allowed. UI and backend both prevent double-clicks.
    let _guard = transfer.try_lock().map_err(|_| "A Premiere transfer is already in progress.")?;
    let output_root = dirs::download_dir().ok_or("Downloads folder unavailable")?.join("CONTAINER Output");
    let output = validate_output(Path::new(&path), &output_root)?;
    // A new sequence derives its tracks from the clip in Premiere; no extra FFprobe process is needed.
    let has_audio = true;
    let folder = root()?;
    let status = status_at(&folder, &session, now());
    if !status.connected { return Err("Premiere is disconnected. No transfer was made.".into()); }
    if status.protocol != 3 { return Err("Update the Premiere connection in Settings and restart Premiere before sending. / Ayarlardan Premiere bağlantısını güncelle ve Premiere’i yeniden başlat.".into()); }
    if status.project != project || status.sequence_id != sequence_id { return Err("The active Premiere timeline changed. Check the destination and try again.".into()); }
    let id = identifier("transfer");
    let project_path = {
        let directory = dirs::document_dir().ok_or("Documents folder unavailable")?.join("Adobe").join("Premiere Pro").join("CONTAINER Projects");
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        directory.join(project_filename(&output, now(), &id)).to_string_lossy().into_owned()
    };
    write_json(&folder.join("request.json"), &serde_json::json!({"session":session,"id":id,"operation":"append","path":output.to_string_lossy(),"project":project,"sequence_id":sequence_id,"project_path":project_path,"has_audio":has_audio,"separate_project":true,"expires":now()+30_000}))?;
    // This entire operation runs on the blocking pool, not the UI/async runtime.
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        let mut confirmation_started = None;
        let mut last_session_refresh = std::time::Instant::now() - Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            // The UI intentionally pauses status requests during a transfer, not session liveness.
            if last_session_refresh.elapsed() >= Duration::from_secs(2) {
                write_json(&folder.join("session.json"), &serde_json::json!({"id":session,"updated":now()}))?;
                last_session_refresh = std::time::Instant::now();
            }
            if let Some(result) = read_json::<TransferResult>(&folder.join("response.json")) {
                if result.session == session && result.id == id {
                    if result.ok {
                        // Require a fresh host reply after completion, not merely a callback before a crash.
                        if let Some(beat) = read_json::<Heartbeat>(&folder.join("heartbeat.json")) {
                            if beat.session == session && beat.connected && beat.updated >= result.completed_at
                                && !result.project_id.is_empty() && beat.project == result.project_id
                                && !beat.sequence_id.is_empty() && status_at(&folder, &session, now()).connected {
                                if result.ownership_record_failed {
                                    return Err("The video was transferred, but its project could not be recorded for cleanup. Do not resend; save/check the project in Premiere. / Video aktarıldı fakat proje temizlik listesine kaydedilemedi. Tekrar gönderme; projeyi Premiere’de kontrol et.".into());
                                }
                                return Ok(());
                            }
                        }
                        let started = confirmation_started.get_or_insert_with(std::time::Instant::now);
                        if started.elapsed() >= Duration::from_secs(5) {
                            return Err("Premiere reported the transfer, but did not respond afterward. Check its project before resending; it may have closed. / Premiere aktarımı bildirdi ama ardından cevap vermedi; tekrar göndermeden projeyi kontrol et.".into());
                        }
                    } else { return {
                    Err(match result.code.as_str() {
                        "sequence_created_save_failed" => "The render is already in a new Premiere timeline, but saving could not be confirmed. Save it manually in Premiere; do not resend. / Video timeline’a eklendi; Premiere’de elle kaydet, tekrar gönderme.".into(),
                        "sequence_created_open_failed" => "The new Premiere timeline was created but could not be opened. Open it from the Project panel; do not resend. / Timeline oluşturuldu; Project panelinden aç, tekrar gönderme.".into(),
                        _ => format!("Premiere could not confirm the transfer ({}). Check its timeline before trying again.", result.code)
                    })
                }; }
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        if read_json::<serde_json::Value>(&folder.join("request.json")).is_some_and(|r| r["id"] == id) { let _ = std::fs::remove_file(folder.join("request.json")); }
        Err("Premiere did not confirm the transfer. Check its timeline before trying again; the render file is unchanged.".into())
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_only_accepts_generated_project_names() {
        assert!(owned_project_name(Path::new(
            "CONTAINER-1790955755400-fd2678f7dffd.prproj"
        )));
        for name in [
            "CONTAINER-my-edit.prproj",
            "MyProject.prproj",
            "CONTAINER-1790955755400-fd2678f7dffd.mp4",
            "CONTAINER-1790955755400-fd2678f7dffd-copy.prproj",
            "CONTAINER-1790955755400-xxxxxxxxxxxx.prproj",
        ] {
            assert!(!owned_project_name(Path::new(name)));
        }
    }
    #[test]
    fn cleanup_reconciles_aborted_complete_partial_and_unreadable_results() {
        let targets = vec![
            PathBuf::from("first.prproj"),
            PathBuf::from("second.prproj"),
        ];
        assert_eq!(
            cleanup_remaining(&targets, |_| Ok(false)).unwrap(),
            (2, vec![])
        );
        assert_eq!(
            cleanup_remaining(&targets, |p| Ok(p == targets[1])).unwrap(),
            (1, vec![targets[1].clone()])
        );
        assert_eq!(
            cleanup_remaining(&targets, |_| Ok(true)).unwrap(),
            (0, targets.clone())
        );
        assert!(cleanup_remaining(&targets, |_| Err(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied
        )))
        .is_err());
        assert_eq!(
            cleanup_remaining(&[], |_| panic!("no targets")).unwrap(),
            (0, vec![])
        );
    }
    #[test]
    fn cleanup_ledger_supports_many_projects_without_relaxing_mailbox_limits() {
        let folder = std::env::temp_dir().join(identifier("ledger-test"));
        std::fs::create_dir(&folder).unwrap();
        let ledger = folder.join("created-projects.json");
        let entries: Vec<String> = (0..500)
            .map(|i| {
                format!(
                    "C:/Projects/CONTAINER-{}-1790955755400-fd2678f7dffd.prproj",
                    "video-name".repeat(10) + &i.to_string()
                )
            })
            .collect();
        write_json(&ledger, &entries).unwrap();
        assert!(read_json::<Vec<String>>(&ledger).is_none());
        assert_eq!(
            read_json_limited::<Vec<String>>(&ledger, 4 * 1024 * 1024).unwrap(),
            entries
        );
        assert!(read_json_limited::<Vec<String>>(&ledger, 1).is_none());
        std::fs::remove_file(ledger).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn cleanup_rejects_an_active_transfer_before_touching_any_project() {
        let transfer = Mutex::new(());
        let files = Mutex::new(());
        let _guard = transfer.lock().unwrap();
        assert!(clean_projects(&transfer, &files)
            .unwrap_err()
            .contains("Wait for"));
    }
    #[test]
    fn project_names_keep_video_identity_and_uniqueness() {
        let ids: std::collections::HashSet<_> = (0..1000).map(|_| identifier("test")).collect();
        assert_eq!(ids.len(), 1000);
        let name = project_filename(
            Path::new("my-video_clipper.mp4"),
            1790955755400,
            "fd2678f7dffdxxxx",
        );
        assert_eq!(
            name,
            "CONTAINER-my-video_clipper-1790955755400-fd2678f7dffd.prproj"
        );
        assert!(owned_project_name(Path::new(&name)));
        assert_ne!(
            name,
            project_filename(
                Path::new("my-video_clipper.mp4"),
                1790955755401,
                "fd2678f7dffdxxxx"
            )
        );
    }
    #[test]
    fn heartbeat_requires_current_session_and_fresh_host_response() {
        let folder = std::env::temp_dir().join(identifier("test"));
        std::fs::create_dir(&folder).unwrap();
        write_json(&folder.join("heartbeat.json"), &serde_json::json!({"session":"current","updated":10000,"connected":true,"project":"p","sequence_id":"s","sequence_name":"Timeline"})).unwrap();
        assert!(status_at(&folder, "current", 11000).connected);
        assert!(!status_at(&folder, "previous", 11000).connected);
        assert!(!status_at(&folder, "current", 16000).connected);
        assert!(!status_at(&folder, "current", 1000).connected);
        std::fs::remove_file(folder.join("heartbeat.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn transfer_rejects_nonvideo_and_files_outside_output() {
        let folder = std::env::temp_dir().join(identifier("test"));
        std::fs::create_dir(&folder).unwrap();
        let video = folder.join("clip.mp4");
        let text = folder.join("clip.txt");
        std::fs::write(&video, b"fixture").unwrap();
        std::fs::write(&text, b"fixture").unwrap();
        assert!(validate_output(&video, &folder).is_ok());
        assert!(validate_output(&text, &folder).is_err());
        let other = folder.join("other");
        std::fs::create_dir(&other).unwrap();
        assert!(validate_output(&video, &other).is_err());
        std::fs::remove_file(video).unwrap();
        std::fs::remove_file(text).unwrap();
        std::fs::remove_dir(other).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
