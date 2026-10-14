//! Removes temporary files left behind by CONTAINER sessions that crashed or
//! were killed. Every temporary path embeds the owning process id, so entries
//! whose process is no longer running can be removed safely, even though the
//! installed app and CONTAINER DEV share the same %TEMP% folder.

use std::path::Path;

/// Temporary names and the separator that follows the process id.
/// `container_<name>_<pid>_<stamp>` folders come from `operation_temp_dir`,
/// quality analysis and Discord pass logs; the others are preview caches.
const PREFIXES: [(&str, char); 2] = [
    ("container-image-preview-", '-'),
    ("container-download-thumbnail-", '-'),
];

fn owner_pid(name: &str) -> Option<u32> {
    if let Some(rest) = name.strip_prefix("container_") {
        // The operation name itself never contains `_`; it may contain `-`.
        let mut parts = rest.splitn(3, '_');
        let _operation = parts.next()?;
        let pid = parts.next()?;
        parts.next()?;
        return pid.parse().ok();
    }
    PREFIXES.iter().find_map(|(prefix, separator)| {
        let rest = name.strip_prefix(prefix)?;
        let (pid, _) = rest.split_once(*separator)?;
        pid.parse().ok()
    })
}

#[cfg(target_os = "windows")]
fn process_is_running(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: plain Win32 calls; the handle is closed before returning.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // Access denied means the process exists but belongs to someone
            // else; keep its files.
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0_u32;
        let alive = GetExitCodeProcess(handle, &mut code) != 0 && code == STILL_ACTIVE as u32;
        CloseHandle(handle);
        alive
    }
}

#[cfg(not(target_os = "windows"))]
fn process_is_running(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

fn remove_stale_entries(
    directory: &Path,
    current_pid: u32,
    is_running: impl Fn(u32) -> bool,
) -> usize {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(owner_pid) else {
            continue;
        };
        if pid == current_pid || is_running(pid) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // Never follow links out of the temp folder.
        let result = if kind.is_dir() {
            std::fs::remove_dir_all(entry.path())
        } else if kind.is_file() {
            std::fs::remove_file(entry.path())
        } else {
            continue;
        };
        if result.is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Removes leftovers from earlier sessions. Runs off the UI thread at startup.
pub(crate) fn remove_stale_temporary_files() -> usize {
    remove_stale_entries(
        &std::env::temp_dir(),
        std::process::id(),
        process_is_running,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_pid_reads_every_temporary_name() {
        assert_eq!(
            owner_pid("container_processing-stack_4242_1700000000"),
            Some(4242)
        );
        assert_eq!(owner_pid("container_quality_17_1700000000"), Some(17));
        assert_eq!(owner_pid("container_discord_99_1700-0.log"), Some(99));
        assert_eq!(owner_pid("container-image-preview-55-abcdef.png"), Some(55));
        assert_eq!(owner_pid("container-download-thumbnail-8-ff.jpg"), Some(8));
        assert_eq!(owner_pid("container_processing-stack"), None);
        assert_eq!(owner_pid("container-test-runtime-1-2"), None);
        assert_eq!(owner_pid("other_tool_12_34"), None);
    }

    #[test]
    fn only_entries_of_finished_processes_are_removed() {
        let root = std::env::temp_dir().join(format!(
            "container-temp-cleanup-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let stale_dir = root.join("container_processing-stack_111_1");
        std::fs::create_dir_all(&stale_dir).unwrap();
        std::fs::write(stale_dir.join("stage-1.mkv"), b"x").unwrap();
        let stale_file = root.join("container-image-preview-111-aa.png");
        std::fs::write(&stale_file, b"x").unwrap();
        let running = root.join("container_processing-stack_222_1");
        std::fs::create_dir_all(&running).unwrap();
        let current = root.join("container_quality_333_1");
        std::fs::create_dir_all(&current).unwrap();
        let unrelated = root.join("something_else_111_1");
        std::fs::create_dir_all(&unrelated).unwrap();

        let removed = remove_stale_entries(&root, 333, |pid| pid == 222);

        assert_eq!(removed, 2);
        assert!(!stale_dir.exists());
        assert!(!stale_file.exists());
        assert!(running.exists());
        assert!(current.exists());
        assert!(unrelated.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn current_process_is_reported_running() {
        assert!(process_is_running(std::process::id()));
    }
}
