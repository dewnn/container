use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use silero_vad_rust::silero_vad::model::OnnxModel;
use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};
use url::{Host, Url};

mod app_log;
mod auto_encoder;
mod download_section;
mod downloader;
mod downloader_runtime;
mod face_detection;
mod ffmpeg_escape;
mod fonts;
mod image_preview;
mod job_inspector;
mod jobs;
mod line_tail;
mod media_probe;
mod ops;
mod output;
mod png_compression;
mod premiere_bridge;
mod process_timeout;
mod processing_stack;
mod project;
mod quality;
mod runtime;
mod smartcut;
mod smartcut_audio;
mod temp_cleanup;

use downloader::*;
use ffmpeg_escape::*;
#[cfg(test)]
use fonts::*;
use image_preview::*;
use jobs::{
    progress_job_id, request_job_cancellation, reset_legacy_job_cancellation, ActiveProcess,
    JobLease, JobState,
};
use media_probe::*;
use ops::*;
use output::*;
use project::*;
#[cfg(test)]
use quality::*;
use runtime::*;
use smartcut::*;

use auto_encoder::{
    auto_encoder_configured, available_encoders, fastest_h264_encoder, warm_up_auto_encoder,
};

#[cfg(target_os = "windows")]
use std::os::windows::{fs::MetadataExt, process::CommandExt};

struct TrayMenuItems {
    open: MenuItem<tauri::Wry>,
    updates: Option<MenuItem<tauri::Wry>>,
    exit: MenuItem<tauri::Wry>,
}

#[tauri::command]
fn set_tray_language(language: &str, items: State<'_, TrayMenuItems>) -> Result<(), String> {
    let turkish = language == "tr";
    items
        .open
        .set_text(if turkish {
            "CONTAINER'ı Aç"
        } else {
            "Open CONTAINER"
        })
        .map_err(|error| error.to_string())?;
    if let Some(updates) = &items.updates {
        updates
            .set_text(if turkish {
                "Güncellemeleri Denetle"
            } else {
                "Check for Updates"
            })
            .map_err(|error| error.to_string())?;
    }
    items
        .exit
        .set_text(if turkish { "Çıkış" } else { "Exit" })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn launch_file_path<I, S>(args: I, cwd: &Path) -> Option<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    args.into_iter()
        .skip(1)
        .map(|argument| {
            let path = PathBuf::from(argument.as_ref());
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .find(|path| path.is_file())
}

pub fn run() {
    app_log::install_panic_hook();
    app_log::info(&format!(
        "{} {} started",
        if is_development_build() {
            "CONTAINER DEV"
        } else {
            "CONTAINER"
        },
        env!("CARGO_PKG_VERSION")
    ));
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            show_main_window(app);
            if let Some(path) = launch_file_path(args, Path::new(&cwd)) {
                let _ = app.emit("container-open-path", path.to_string_lossy().to_string());
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(JobState::default())
        .manage(DownloadHistoryState::default())
        .manage(RuntimeMigrationState::default())
        .manage(premiere_bridge::PremiereBridge::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            job_inspector::job_inspector,
            premiere_bridge::premiere_status,
            premiere_bridge::premiere_install,
            premiere_bridge::premiere_remove,
            premiere_bridge::premiere_clean_projects,
            premiere_bridge::premiere_send,
            media_probe::probe_media,
            media_probe::authorize_media_preview,
            project::read_project,
            project::write_project,
            project::project_media_available,
            media_probe::probe_subtitles,
            runtime::ffmpeg_status,
            runtime::ffmpeg_capabilities,
            runtime::ffmpeg_runtime_error,
            runtime::repair_ffmpeg_runtime,
            downloader::downloader_status,
            downloader::set_downloader_binary,
            downloader_runtime::update_downloader,
            downloader::analyze_download_url,
            downloader::remove_download_thumbnail,
            downloader::download_media,
            downloader::list_download_history,
            downloader::delete_download_history_entry,
            image_preview::prepare_image_preview,
            image_preview::remove_image_preview,
            fonts::list_system_fonts,
            fonts::font_preview_data,
            available_encoders,
            auto_encoder_configured,
            warm_up_auto_encoder,
            media_probe::hash_file,
            media_probe::list_media_files,
            smartcut::compute_autocut_waveform,
            smartcut::compute_video_filmstrip,
            smartcut::autocut_presets,
            smartcut::recommend_autocut_settings,
            smartcut::analyze_autocut,
            smartcut::export_autocut,
            quality::analyze_quality,
            ops::image_tools::estimate_image_compression,
            ops::run::run_operation,
            ops::clipper::detect_camera_region,
            ops::run::cancel_job,
            project::startup_media_path,
            project::previous_session_interrupted,
            output::clean_output_folder,
            set_tray_language,
            app_log::log_ui_error,
            app_log::open_log_folder
        ])
        .setup(|app| {
            // Migration failures keep the legacy file; DWLNDR reports them on
            // access rather than making an otherwise usable editor fail startup.
            if let Ok(_guard) = app.state::<DownloadHistoryState>().0.lock() {
                let _ = download_history_path();
            }
            let open = MenuItem::with_id(app, "tray-open", "Open CONTAINER", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "tray-exit", "Exit", true, None::<&str>)?;
            let updates = if is_development_build() {
                None
            } else {
                Some(MenuItem::with_id(
                    app,
                    "tray-updates",
                    "Check for Updates",
                    true,
                    None::<&str>,
                )?)
            };
            let menu = if let Some(updates_item) = &updates {
                Menu::with_items(app, &[&open, updates_item, &exit])?
            } else {
                Menu::with_items(app, &[&open, &exit])?
            };
            let icon = app
                .default_window_icon()
                .ok_or("Missing default window icon")?
                .clone();
            TrayIconBuilder::new()
                .icon(icon)
                .tooltip(if is_development_build() {
                    "CONTAINER DEV"
                } else {
                    "CONTAINER"
                })
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "tray-open" => show_main_window(app),
                    "tray-updates" => {
                        show_main_window(app);
                        let _ = app.emit("tray-check-updates", ());
                    }
                    "tray-exit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            app.manage(TrayMenuItems {
                open,
                updates,
                exit,
            });
            cleanup_legacy_download_thumbnail_cache();
            tauri::async_runtime::spawn(downloader_runtime::refresh_bundled_downloader());
            tauri::async_runtime::spawn_blocking(temp_cleanup::remove_stale_temporary_files);
            let migration = app.state::<RuntimeMigrationState>();
            let _ = record_runtime_migration(&migration);
            app.manage(prepare_session_lifecycle());
            if let Some(path) = startup_media_path().map(PathBuf::from) {
                app.asset_protocol_scope().allow_file(path)?;
            }
            if let Some(window) = app.get_webview_window("main") {
                let title = if is_development_build() {
                    "CONTAINER DEV"
                } else {
                    "CONTAINER"
                };
                let _ = window.set_title(title);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building CONTAINER")
        .run(|app, event| {
            if let tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } = &event
            {
                if label == "main" {
                    api.prevent_close();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                        let _ = window.emit("container-tray-hidden", ());
                    }
                }
            }
            if matches!(event, tauri::RunEvent::Exit) {
                cleanup_current_image_previews();
                // yt-dlp must be tied to this application session. Without
                // explicitly ending its process tree, Windows can keep a
                // download alive after the UI has been closed.
                let state = app.state::<JobState>();
                state.cancelled.store(true, Ordering::Relaxed);
                if let Ok(mut active_pid) = state.pid.lock() {
                    if let Some(pid) = active_pid.take() {
                        stop_process_tree(pid);
                    }
                }
                cleanup_current_download(&state);
                let lifecycle = app.state::<SessionLifecycle>();
                let _ = std::fs::remove_file(&lifecycle.marker_path);
            }
        });
}

#[cfg(test)]
mod tests;
