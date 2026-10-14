//! Temporary PNG previews for HEIC/HEIF images.

use crate::*;

pub(crate) fn temporary_image_preview_path(source: &Path) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let identity = format!("{}:{nonce}", source.to_string_lossy());
    std::env::temp_dir().join(format!(
        "container-image-preview-{}-{:x}.png",
        std::process::id(),
        Sha256::digest(identity.as_bytes())
    ))
}

pub(crate) fn remove_temporary_image_preview_file(path: &Path) -> Result<bool, String> {
    let valid_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("container-image-preview-") && name.ends_with(".png"));
    if path.parent() != Some(std::env::temp_dir().as_path()) || !valid_name {
        return Err("Invalid temporary image preview path.".into());
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "Temporary image preview could not be removed: {error}"
        )),
    }
}

pub(crate) fn cleanup_current_image_previews() {
    let directory = std::env::temp_dir();
    let prefix = format!("container-image-preview-{}-", std::process::id());
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let matches = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".png"));
        if matches && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[tauri::command]
pub(crate) async fn prepare_image_preview(app: AppHandle, path: String) -> Result<String, String> {
    let source = PathBuf::from(path);
    if !source.is_file() {
        return Err("Image preview source was not found.".into());
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "heic" | "heif") {
        return Err("A compatibility preview is only required for HEIC/HEIF images.".into());
    }
    let output = temporary_image_preview_path(&source);
    let temporary = output.with_extension("part.png");
    let result = async {
        let process = process_timeout::output_within(
            hidden_command("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&source)
                // Do not select v:0 here. Apple HEIC files can store the primary
                // picture as a default Tile Grid made from many HEVC streams.
                // FFmpeg reconstructs that grid only when its default output is
                // allowed to select the primary image.
                .args(["-frames:v", "1", "-c:v", "png"])
                .arg(&temporary),
            process_timeout::PREVIEW,
        )
        .await
        .map_err(|error| format!("HEIC preview could not start: {error}"))?;
        if !process.status.success() {
            return Err(format!(
                "HEIC preview could not be generated: {}",
                String::from_utf8_lossy(&process.stderr).trim()
            ));
        }
        if !temporary.is_file()
            || std::fs::metadata(&temporary).map_or(true, |value| value.len() == 0)
        {
            return Err("HEIC preview generation produced no image.".into());
        }
        std::fs::rename(&temporary, &output)
            .map_err(|error| format!("HEIC preview could not be finalized: {error}"))?;
        allow_asset_file(&app, &output)?;
        Ok(output.to_string_lossy().to_string())
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
        let _ = std::fs::remove_file(&output);
    }
    result
}

#[tauri::command]
pub(crate) fn remove_image_preview(path: String) -> Result<bool, String> {
    remove_temporary_image_preview_file(Path::new(&path))
}
