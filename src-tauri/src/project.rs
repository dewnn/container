//! `.cproj` project files and the session lifecycle marker.

use crate::*;

pub(crate) struct SessionLifecycle {
    pub(crate) previous_interrupted: bool,
    pub(crate) marker_path: PathBuf,
}

pub(crate) fn read_project_contents(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("cproj"))
    {
        return Err("Choose a .cproj file.".into());
    }
    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > 5 * 1024 * 1024 {
        return Err("The project file is invalid or too large.".into());
    }
    std::fs::read_to_string(path).map_err(|error| format!("Project could not be opened: {error}"))
}

pub(crate) fn project_relative_path(project: &Path, resource: &Path) -> Option<String> {
    let base = project.parent()?.canonicalize().ok()?;
    let source = resource.canonicalize().ok()?;
    let base_parts: Vec<_> = base.components().collect();
    let source_parts: Vec<_> = source.components().collect();
    if base_parts.first() != source_parts.first() {
        return None;
    }
    let common = base_parts
        .iter()
        .zip(&source_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..base_parts.len() {
        relative.push("..");
    }
    for component in source_parts.iter().skip(common) {
        relative.push(component.as_os_str());
    }
    Some(relative.to_string_lossy().into_owned())
}

pub(crate) fn replace_project_path(value: &mut Value, previous: &str, replacement: &str) {
    match value {
        Value::String(text) if text == previous => *text = replacement.to_string(),
        Value::Array(items) => {
            for item in items {
                replace_project_path(item, previous, replacement);
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                replace_project_path(item, previous, replacement);
            }
        }
        _ => {}
    }
}

pub(crate) fn resolve_project_resources(project: &Path, value: &mut Value) {
    let references: Vec<(String, String)> = value["resources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|resource| {
            Some((
                resource["path"].as_str()?.to_string(),
                resource["relativePath"].as_str()?.to_string(),
            ))
        })
        .collect();
    let Some(parent) = project.parent() else {
        return;
    };
    for (original, relative) in references {
        let relative = Path::new(&relative);
        if relative.is_absolute() {
            continue;
        }
        let candidate = parent.join(relative);
        // A project may live beside its media rather than inside the same
        // directory. Joining a saved ../ path works for ffprobe, but the
        // asset protocol deliberately rejects traversal in preview URLs.
        // Canonicalize before passing the path to the frontend or asset scope.
        if let Ok(resolved) = dunce::canonicalize(&candidate) {
            if resolved.is_file() {
                replace_project_path(value, &original, &resolved.to_string_lossy());
            }
        }
    }
}

#[tauri::command]
pub(crate) fn read_project(app: AppHandle, path: String) -> Result<String, String> {
    let contents = read_project_contents(path.clone())?;
    let mut value: Value =
        serde_json::from_str(&contents).map_err(|_| "The project data is invalid.".to_string())?;
    resolve_project_resources(Path::new(&path), &mut value);
    if let Some(media_path) = value["mediaPath"]
        .as_str()
        .map(PathBuf::from)
        .filter(|media_path| media_path.is_file())
    {
        allow_asset_file(&app, &media_path)?;
    }
    serde_json::to_string(&value).map_err(|error| error.to_string())
}

// Async so a slow or offline network path cannot freeze the window.
#[tauri::command]
pub(crate) async fn project_media_available(path: String) -> bool {
    tokio::task::spawn_blocking(move || PathBuf::from(path).is_file())
        .await
        .unwrap_or(false)
}

#[tauri::command]
pub(crate) fn write_project(path: String, contents: String) -> Result<(), String> {
    if contents.len() > 5 * 1024 * 1024 {
        return Err("The project data is too large.".into());
    }
    let mut value = serde_json::from_str::<Value>(&contents)
        .map_err(|_| "The project data is invalid.".to_string())?;
    let mut path = PathBuf::from(path);
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("cproj"))
    {
        path.set_extension("cproj");
    }
    if let Some(resources) = value.get_mut("resources").and_then(Value::as_array_mut) {
        for resource in resources {
            if let Some(source) = resource["path"].as_str() {
                if let Some(relative) = project_relative_path(&path, Path::new(source)) {
                    resource["relativePath"] = Value::String(relative);
                }
            }
        }
    }
    let serialized = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    if serialized.len() > 5 * 1024 * 1024 {
        return Err("The project data is too large.".into());
    }
    // Write beside the target before replacing it: a failed write must not
    // truncate the previous project or modify a hardlinked source file.
    let pending = path.with_file_name(format!(
        ".container-project-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|error| format!("Project could not be saved: {error}"))?;
    let result = (|| -> std::io::Result<()> {
        use std::io::Write;
        file.write_all(serialized.as_bytes())?;
        file.sync_all()
    })();
    drop(file);
    let result = result.and_then(|()| std::fs::rename(&pending, &path));
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result.map_err(|error| format!("Project could not be saved: {error}"))
}

#[tauri::command]
pub(crate) fn startup_media_path() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    launch_file_path(std::env::args_os(), &cwd).map(|path| path.to_string_lossy().to_string())
}

pub(crate) fn prepare_session_lifecycle() -> SessionLifecycle {
    let directory = app_data_directory().unwrap_or_else(|_| {
        std::env::temp_dir()
            .join(app_storage_namespace())
            .join("data")
    });
    let legacy = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(legacy_storage_name());
    retire_legacy_embedded_caches(&legacy, !is_development_build());
    prepare_session_lifecycle_at(&directory, &legacy)
}

pub(crate) fn prepare_session_lifecycle_at(directory: &Path, legacy: &Path) -> SessionLifecycle {
    let _ = std::fs::create_dir_all(directory);
    let marker_path = directory.join("running.lock");
    let legacy_marker = legacy.join("running.lock");
    let previous_interrupted = marker_path.is_file() || legacy_marker.is_file();
    if std::fs::write(&marker_path, std::process::id().to_string()).is_ok() {
        let _ = std::fs::remove_file(legacy_marker);
    }
    SessionLifecycle {
        previous_interrupted,
        marker_path,
    }
}

// These are disposable copies of bytes still embedded in the EXE, not user
// projects. Keep unknown/modified files and never recursively remove folders.
pub(crate) fn retire_legacy_embedded_caches(legacy: &Path, include_models: bool) {
    fn plain_path(path: &Path, directory: bool) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|metadata| {
            let regular = if directory {
                metadata.is_dir()
            } else {
                metadata.is_file()
            };
            #[cfg(target_os = "windows")]
            let regular = regular && metadata.file_attributes() & 0x400 == 0;
            regular && !metadata.file_type().is_symlink()
        })
    }
    if !plain_path(legacy, true) {
        return;
    }
    let mut assets: Vec<(&str, String, &[u8])> = [
        include_bytes!("../resources/social-tags/twitch.png").as_slice(),
        include_bytes!("../resources/social-tags/kick-boxed.png").as_slice(),
        include_bytes!("../resources/social-tags/kick-plain.png").as_slice(),
    ]
    .into_iter()
    .map(|bytes| {
        (
            "social-tags",
            format!("{:x}.png", Sha256::digest(bytes)),
            bytes,
        )
    })
    .collect();
    if include_models {
        let bytes = include_bytes!("../resources/silero_vad.onnx").as_slice();
        assets.push((
            "models",
            format!("silero-{:x}.onnx", Sha256::digest(bytes)),
            bytes,
        ));
    }
    for (folder, name, bytes) in assets {
        let directory = legacy.join(folder);
        let path = directory.join(name);
        if plain_path(&directory, true)
            && plain_path(&path, false)
            && std::fs::read(&path).ok().as_deref() == Some(bytes)
        {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir(directory);
        }
    }
}

#[tauri::command]
pub(crate) fn previous_session_interrupted(lifecycle: State<'_, SessionLifecycle>) -> bool {
    lifecycle.previous_interrupted
}
