//! Output naming under Downloads/CONTAINER Output and output cleanup.

use crate::*;

#[derive(Debug, Serialize)]
pub(crate) struct OutputCleanupResult {
    pub(crate) cleaned: bool,
    pub(crate) path: String,
}

pub(crate) fn safe_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or("media")
        .chars()
        .map(|character| {
            if "<>:\"/\\|?*".contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect()
}

pub(crate) fn category(operation: &str) -> &'static str {
    match operation {
        "transform" | "clipper" | "ratio" | "resize" => "transform",
        "upscale" => "upscale",
        "fps" | "interpolation" | "frame_blend" | "speed" | "cfr" | "stabilizer" => "motion",
        "compression" | "smart_quality" | "discord_compressor" | "potatoify" => "quality",
        "text" | "image_overlay" => "text",
        "color" | "noise" | "negate" | "blur_pixelate" => "effects",
        "remove_audio" => "video",
        "extract_audio" | "replace_audio" | "distortion" | "audio_convert" => "audio",
        "image_ratio" | "image_compressor" | "metadata_cleaner" | "image_potatoify" => "image",
        "proxy" => "proxy",
        "autocut" | "smartcut" => "smartcut",
        _ => "export",
    }
}

pub(crate) fn output_category(operation: &str, extension: &str) -> &'static str {
    if operation == "frame_extractor"
        || matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" | "avif" | "heic" | "heif"
        )
    {
        "image"
    } else {
        category(operation)
    }
}

pub(crate) fn unique_output(
    input: &Path,
    operation: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    let input_parent = input
        .parent()
        .ok_or("Input folder could not be resolved.")?;
    #[cfg(test)]
    let output_root = input_parent.to_path_buf();
    #[cfg(not(test))]
    let output_root = dirs::download_dir().unwrap_or_else(|| input_parent.to_path_buf());
    let folder = output_root
        .join("CONTAINER Output")
        .join(output_category(operation, extension));
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let base = format!("{}_{}", safe_stem(input), operation);
    let mut output = folder.join(format!("{base}.{extension}"));
    let mut counter = 1;
    while output.exists() {
        output = folder.join(format!("{base} ({counter}).{extension}"));
        counter += 1;
    }
    Ok(output)
}

pub(crate) fn operation_output_source(request: &OperationRequest) -> Result<PathBuf, String> {
    let Some(original) = request.params.get("__source_path") else {
        return Ok(PathBuf::from(&request.input));
    };
    let original = PathBuf::from(original);
    let extension = original
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !original.is_file() || !matches!(extension.as_str(), "heic" | "heif") {
        return Err("Invalid original HEIC source path.".into());
    }
    Ok(original)
}

// Shell recycling of a whole directory can abort if one child is in use. Keep
// the blocked child in place, but still recycle everything else for a retry.
pub(crate) fn recycle_output_entry(path: &Path, blocked: &mut Vec<PathBuf>) {
    let _ = trash::delete(path);
    let metadata = match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(_) => {
            blocked.push(path.to_path_buf());
            return;
        }
        Ok(metadata) => metadata,
    };
    let is_regular_dir = metadata.is_dir() && !metadata.file_type().is_symlink();
    #[cfg(target_os = "windows")]
    let is_regular_dir = is_regular_dir && metadata.file_attributes() & 0x400 == 0;
    if !is_regular_dir {
        blocked.push(path.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        blocked.push(path.to_path_buf());
        return;
    };
    let mut children = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => children.push(entry.path()),
            Err(_) => {
                blocked.push(path.to_path_buf());
                return;
            }
        }
    }
    let failures_before = blocked.len();
    for child in children {
        recycle_output_entry(&child, blocked);
    }
    if blocked.len() == failures_before {
        let _ = trash::delete(path);
        if std::fs::symlink_metadata(path).is_ok() {
            blocked.push(path.to_path_buf());
        }
    }
}

pub(crate) fn recycle_output_entries(root: &Path, entries: &[PathBuf]) -> Result<(), String> {
    let mut blocked = Vec::new();
    for entry in entries {
        recycle_output_entry(entry, &mut blocked);
    }
    if blocked.is_empty() {
        return Ok(());
    }
    let names = blocked
        .iter()
        .take(5)
        .map(|path| {
            path.strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "{} output item(s) could not be moved to the Recycle Bin, possibly because they are open in another app. Close apps using them and retry. Remaining: {names}",
        blocked.len()
    ))
}

#[tauri::command]
pub(crate) async fn clean_output_folder(
    state: State<'_, JobState>,
) -> Result<OutputCleanupResult, String> {
    // Hold the job lease for the whole cleanup. `pid` alone is empty between
    // FFmpeg stages (PNG optimisation, Stack/Discord passes, frame export), so
    // it cannot tell whether a job is still writing into the output folder.
    // The lease also stops a new job from starting while files are recycled.
    let _cleanup_lease = JobLease::reserve(&state, "output-cleanup").map_err(|_| {
        "Wait for the current media job to finish before cleaning output.".to_string()
    })?;
    if state
        .pid
        .lock()
        .map_err(|_| "Job state is unavailable.".to_string())?
        .is_some()
    {
        return Err("Wait for the current media job to finish before cleaning output.".into());
    }

    let downloads = dirs::download_dir().ok_or("Downloads folder is unavailable.")?;
    let root = downloads.join("CONTAINER Output");
    let display = root.to_string_lossy().to_string();
    if !root.exists() {
        return Ok(OutputCleanupResult {
            cleaned: false,
            path: display,
        });
    }
    if root.parent() != Some(downloads.as_path()) {
        return Err("Output folder could not be validated.".into());
    }
    let metadata = std::fs::symlink_metadata(&root).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Output path is not a regular folder.".into());
    }
    #[cfg(target_os = "windows")]
    if metadata.file_attributes() & 0x400 != 0 {
        return Err("Output path is a Windows reparse point and was not cleaned.".into());
    }

    let entries = std::fs::read_dir(&root)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|value| value.path())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if entries.is_empty() {
        return Ok(OutputCleanupResult {
            cleaned: false,
            path: display,
        });
    }

    tokio::task::spawn_blocking(move || recycle_output_entries(&root, &entries))
        .await
        .map_err(|error| format!("Output cleanup task failed: {error}"))??;
    Ok(OutputCleanupResult {
        cleaned: true,
        path: display,
    })
}
