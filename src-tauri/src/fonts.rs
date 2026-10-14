//! System and bundled fonts for text layers, watermark and Social Tag.

use crate::*;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FontOption {
    pub(crate) name: String,
    pub(crate) path: String,
}

pub(crate) const SOCIAL_FONT_NAMES: [&str; 11] = [
    "Impact",
    "Arial Black",
    "Montserrat Black",
    "Montserrat ExtraBold",
    "Montserrat ExtraBold Italic (800)",
    "Montserrat Bold",
    "Montserrat SemiBold",
    "Poppins Black",
    "Poppins ExtraBold",
    "Poppins Bold",
    "Poppins SemiBold",
];

pub(crate) fn bundled_text_font() -> Result<FontOption, String> {
    let path = social_tag_asset_path_in(
        &app_cache_directory()?.join("text-fonts"),
        include_bytes!("../resources/fonts/Montserrat-ExtraBoldItalic.ttf"),
        "ttf",
    )?;
    Ok(FontOption {
        name: "Montserrat ExtraBold Italic (800)".into(),
        path: path.to_string_lossy().into_owned(),
    })
}

pub(crate) fn curate_social_fonts(fonts: Vec<FontOption>) -> Vec<FontOption> {
    SOCIAL_FONT_NAMES
        .iter()
        .filter_map(|wanted| {
            fonts
                .iter()
                .find(|font| font.name.eq_ignore_ascii_case(wanted))
                .cloned()
        })
        .collect()
}

#[tauri::command]
pub(crate) async fn list_system_fonts() -> Result<Vec<FontOption>, String> {
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let system_fonts = PathBuf::from(&windows).join("Fonts");
    let user_fonts =
        dirs::data_local_dir().map(|path| path.join("Microsoft").join("Windows").join("Fonts"));
    let mut fonts = Vec::new();

    #[cfg(target_os = "windows")]
    for key in [
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
        r"HKCU\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
    ] {
        if let Ok(output) = process_timeout::output_within(
            hidden_command("reg").args(["query", key]),
            process_timeout::VERSION_CHECK,
        )
        .await
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let Some((raw_name, raw_path)) = line
                    .split_once("REG_EXPAND_SZ")
                    .or_else(|| line.split_once("REG_SZ"))
                else {
                    continue;
                };
                let raw_path = raw_path.trim().replace("%WINDIR%", &windows);
                let supplied = PathBuf::from(&raw_path);
                let path = if supplied.is_absolute() {
                    supplied
                } else {
                    let user_candidate = user_fonts.as_ref().map(|dir| dir.join(&supplied));
                    if user_candidate.as_ref().is_some_and(|path| path.is_file()) {
                        user_candidate.unwrap()
                    } else {
                        system_fonts.join(supplied)
                    }
                };
                let supported = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| {
                        matches!(value.to_ascii_lowercase().as_str(), "ttf" | "otf")
                    });
                if path.is_file() && supported {
                    let name = raw_name
                        .trim()
                        .replace(" (TrueType)", "")
                        .replace(" (OpenType)", "");
                    fonts.push(FontOption {
                        name,
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }

    if fonts.is_empty() {
        for directory in std::iter::once(system_fonts).chain(user_fonts) {
            let Ok(entries) = std::fs::read_dir(directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                if !matches!(extension.to_ascii_lowercase().as_str(), "ttf" | "otf") {
                    continue;
                }
                fonts.push(FontOption {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
    fonts.push(bundled_text_font()?);
    fonts.sort_by_key(|font| font.name.to_ascii_lowercase());
    fonts.dedup_by(|left, right| left.name.eq_ignore_ascii_case(&right.name));
    Ok(curate_social_fonts(fonts))
}

#[tauri::command]
pub(crate) fn font_preview_data(path: String) -> Result<String, String> {
    let canonical = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("Font file could not be opened: {error}"))?;
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let mut roots = vec![PathBuf::from(windows).join("Fonts")];
    if let Some(user_fonts) =
        dirs::data_local_dir().map(|root| root.join("Microsoft").join("Windows").join("Fonts"))
    {
        roots.push(user_fonts);
    }
    let bundled = bundled_text_font()?;
    let allowed = PathBuf::from(bundled.path).canonicalize().ok().as_ref() == Some(&canonical)
        || roots
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .any(|root| canonical.starts_with(root));
    if !allowed {
        return Err("Font preview is limited to installed or bundled fonts.".into());
    }
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "ttf" | "otf") {
        return Err("This installed font format cannot be previewed.".into());
    }
    let metadata = std::fs::metadata(&canonical)
        .map_err(|error| format!("Font metadata could not be read: {error}"))?;
    if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
        return Err("The selected font file is invalid or too large.".into());
    }
    let bytes = std::fs::read(&canonical)
        .map_err(|error| format!("Font file could not be read: {error}"))?;
    let mime = match extension.as_str() {
        "otf" => "font/otf",
        _ => "font/ttf",
    };
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}
