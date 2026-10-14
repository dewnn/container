//! A small rotating log file for bug reports.
//!
//! Release builds have no console, so panics, background warnings and the
//! errors shown to the user are written to
//! `%LOCALAPPDATA%\<app>\logs\container.log`. The file rotates at 1 MB and one
//! previous file is kept. Logging never fails the caller.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_MESSAGE_CHARS: usize = 4000;
static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn log_directory() -> Option<PathBuf> {
    #[cfg(test)]
    {
        Some(crate::test_runtime_root().join("logs"))
    }
    #[cfg(not(test))]
    {
        dirs::data_local_dir().map(|root| root.join(crate::app_storage_namespace()).join("logs"))
    }
}

fn timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    // UTC calendar date from the Unix day count (civil-from-days algorithm).
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

fn clean(message: &str) -> String {
    let mut text: String = message.chars().take(MAX_MESSAGE_CHARS).collect();
    if message.chars().count() > MAX_MESSAGE_CHARS {
        text.push_str(" …");
    }
    text.replace('\r', "")
}

pub(crate) fn write(level: &str, message: &str) {
    let Some(directory) = log_directory() else {
        return;
    };
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if std::fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join("container.log");
    if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > MAX_BYTES) {
        let _ = std::fs::rename(&path, directory.join("container.previous.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{} [{level}] {}", timestamp(), clean(message));
    }
}

pub(crate) fn info(message: &str) {
    write("info", message);
}

pub(crate) fn warn(message: &str) {
    write("warn", message);
}

/// Records panics with their location before the default handler runs.
pub(crate) fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let location = panic
            .location()
            .map(|location| format!(" at {}:{}", location.file(), location.line()))
            .unwrap_or_default();
        let payload = panic
            .payload()
            .downcast_ref::<&str>()
            .map(|value| value.to_string())
            .or_else(|| panic.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        write("panic", &format!("{payload}{location}"));
        previous(panic);
    }));
}

/// Errors that the interface showed to the user.
#[tauri::command]
pub(crate) fn log_ui_error(message: String) {
    write("ui-error", &message);
}

#[tauri::command]
pub(crate) fn open_log_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let directory = log_directory().ok_or("Log folder is unavailable.")?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    app.opener()
        .open_path(directory.to_string_lossy(), None::<&str>)
        .map_err(|error| format!("Log folder could not be opened: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_bounded_and_single_line_safe() {
        let long = "x".repeat(MAX_MESSAGE_CHARS + 50);
        assert_eq!(clean(&long).chars().count(), MAX_MESSAGE_CHARS + 2);
        assert_eq!(clean("a\r\nb"), "a\nb");
    }

    #[test]
    fn timestamp_is_iso_like() {
        let value = timestamp();
        assert_eq!(value.len(), 20);
        assert_eq!(&value[4..5], "-");
        assert!(value.ends_with('Z'));
    }

    #[test]
    fn write_appends_to_the_log_file() {
        write("info", "log test entry");
        let contents =
            std::fs::read_to_string(log_directory().unwrap().join("container.log")).unwrap();
        assert!(contents.contains("[info] log test entry"));
    }
}
