// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use lib::fs::{self as file_system};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{Emitter, Listener, Manager, Url};
use tauri_plugin_fs::FsExt;
use tauri_plugin_log::{Target as LogTarget, TargetKind as LogTargetKind};

use lib::tauri_commands::{
    ffmpeg::{
        __cmd__compress_video, __cmd__compress_videos_batch, __cmd__generate_video_thumbnail,
        __cmd__get_video_info, compress_video, compress_videos_batch, generate_video_thumbnail,
        get_video_info,
    },
    ffprobe::{
        __cmd__get_audio_streams, __cmd__get_chapters, __cmd__get_container_info,
        __cmd__get_subtitle_streams, __cmd__get_video_streams, get_audio_streams, get_chapters,
        get_container_info, get_subtitle_streams, get_video_streams,
    },
    file_manager::{__cmd__show_item_in_file_manager, show_item_in_file_manager},
    fs::{
        __cmd__copy_file_to_clipboard, __cmd__delete_cache, __cmd__delete_file,
        __cmd__get_file_metadata, __cmd__get_image_dimension, __cmd__move_file,
        __cmd__read_files_from_clipboard, __cmd__read_files_from_paths, copy_file_to_clipboard,
        delete_cache, delete_file, get_file_metadata, get_image_dimension, move_file,
        read_files_from_clipboard, read_files_from_paths,
    },
};

#[cfg(target_os = "linux")]
use lib::tauri_commands::file_manager::DbusState;

#[cfg(any(windows, target_os = "linux"))]
use std::path::PathBuf;

#[cfg(debug_assertions)]
const LOG_TARGETS: [LogTarget; 1] = [LogTarget::new(LogTargetKind::Stdout)];

#[cfg(not(debug_assertions))]
const LOG_TARGETS: [LogTarget; 0] = [];

// Global storage for URLs that arrive before app setup completes.
// "Open with CompressO" triggers `application:openURLs` before app setup is complete
static EARLY_URLS: OnceLock<Mutex<Vec<Url>>> = OnceLock::new();

struct PendingFiles(Arc<Mutex<Vec<String>>>);

impl PendingFiles {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }
}

struct FrontendReady(Arc<Mutex<bool>>);

impl FrontendReady {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(false)))
    }

    fn set_ready(&self) {
        *self.0.lock().unwrap() = true;
    }

    fn is_ready(&self) -> bool {
        *self.0.lock().unwrap()
    }
}

fn emit_pending_open_with_app_files(app_handle: &tauri::AppHandle) {
    if let Some(pending) = app_handle.try_state::<PendingFiles>() {
        let pending_inner = pending.0.clone();
        let app_handle_clone = app_handle.clone();

        let mut list = pending_inner.lock().unwrap();
        if !list.is_empty() {
            let files_to_emit = list.drain(..).collect::<Vec<_>>();
            log::info!("Emitting files on frontend ready: {:?}", files_to_emit);
            drop(list);
            if let Err(e) = app_handle_clone.emit("open-with-app", files_to_emit) {
                log::error!("Failed to emit open-with-app event: {:?}", e);
            }
        }
    } 
}

fn handle_open_with_app(app_handle: &tauri::AppHandle, urls: Vec<Url>) -> Result<(), String> {

    let fs_scope = app_handle.fs_scope();
    let asset_scope = app_handle.asset_protocol_scope();

    let new_files: Vec<String> = urls
        .iter()
        .filter_map(|url| url.to_file_path().ok())
        .map(|p| p.to_string_lossy().replace('\\', "\\\\"))
        .collect();

    for url in &urls {
        if let Ok(path) = url.to_file_path() {
            if let Err(e) = fs_scope.allow_file(&path) {
                let path_str = path.to_string_lossy().to_string();
                return Err(format!("Failed to allow file in fs_scope: {}", path_str));
            }
            if let Err(e) = asset_scope.allow_file(&path) {
                let path_str = path.to_string_lossy().to_string();
                return Err(format!("Failed to allow file in asset_scope: {}", path_str));
            }
        }
    }

    if let Some(pending) = app_handle.try_state::<PendingFiles>() {
        let frontend_ready = app_handle.try_state::<FrontendReady>();
        let pending_inner = pending.0.clone();

        {
            let mut list = pending_inner
                .lock()
                .map_err(|e| format!("Failed to lock pending files: {:?}", e))?;
            list.extend(new_files);
        }

        if let Some(ready_state) = frontend_ready {
            let is_ready = ready_state.is_ready();
            if is_ready {
                let app_handle_clone = app_handle.clone();
                let mut list = pending_inner.lock().unwrap();
                log::info!("Frontend is ready, emitting {} pending files", list.len());
                if !list.is_empty() {
                    let files_to_emit = list.drain(..).collect::<Vec<_>>();
                    drop(list);
                    if let Err(e) = app_handle_clone.emit("open-with-app", files_to_emit) {
                        log::error!("Failed to emit open-with-app event: {:?}", e);
                    }
                }
            } else {
                log::info!("Frontend not ready yet, files stored in pending list");
            }
        }
    } else {
        log::info!(
            "PendingFiles state not ready, storing {} URLs for later",
            urls.len()
        );
        let early_urls = EARLY_URLS.get_or_init(|| Mutex::new(Vec::new()));
        let mut stored = early_urls.lock().unwrap();
        stored.extend(urls);
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(LOG_TARGETS)
                .build(),
        )
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            app.manage(PendingFiles::new());
            app.manage(FrontendReady::new());

            #[cfg(target_os = "linux")]
            app.manage(DbusState(Mutex::new(
                dbus::blocking::SyncConnection::new_session().ok(),
            )));

            file_system::setup_app_data_dir(app)?;

            let app_handle = app.handle().clone();
            app.once("frontend-ready", move |_| {
                log::info!("Frontend is ready, checking for pending files");

                if let Some(ready_state) = app_handle.try_state::<FrontendReady>() {
                    ready_state.set_ready();
                }

                emit_pending_open_with_app_files(&app_handle);
            });

            if let Some(early_urls) = EARLY_URLS.get() {
                let urls_to_process = {
                    let mut stored = early_urls.lock().unwrap();
                    if stored.is_empty() {
                        None
                    } else {
                        Some(stored.drain(..).collect::<Vec<_>>())
                    }
                };

                if let Some(urls) = urls_to_process {
                    if let Err(e) = handle_open_with_app(app.handle(), urls) {
                        log::error!("Failed to handle early open with app URLs: {:?}", e);
                    }
                }
            }

            #[cfg(any(windows, target_os = "linux"))]
            {
                let args: Vec<String> = std::env::args().collect();

                let mut urls = Vec::new();
                for maybe_file in args.iter().skip(1) {
                    if maybe_file.starts_with('-') {
                        continue;
                    }

                    if cfg!(target_os = "windows") {
                        let path = PathBuf::from(maybe_file);

                        match Url::from_file_path(&path) {
                            Ok(url) => {
                                urls.push(url);
                            }
                            Err(_) => {
                                log::error!("Failed to convert Windows path: {:?}", path);
                            }
                        }
                    } else {
                        if let Ok(url) = Url::parse(maybe_file) {
                            if let Ok(_path) = url.to_file_path() {
                                urls.push(url);
                            }
                        } else {
                            let path = PathBuf::from(maybe_file);
                            if let Ok(url) = Url::from_file_path(&path) {
                                urls.push(url);
                            }
                        }
                    }
                }
                if let Err(e) = handle_open_with_app(app.handle(), urls) {
                    log::error!("Failed to handle open with app (platform args): {:?}", e);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            compress_video,
            compress_videos_batch,
            generate_video_thumbnail,
            get_video_info,
            get_image_dimension,
            get_file_metadata,
            move_file,
            delete_file,
            delete_cache,
            show_item_in_file_manager,
            copy_file_to_clipboard,
            read_files_from_clipboard,
            read_files_from_paths,
            get_audio_streams,
            get_chapters,
            get_container_info,
            get_subtitle_streams,
            get_video_streams,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(
            #[allow(unused_variables)]
            |app_handle, event| {
                #[cfg(any(target_os = "macos"))]
                if let tauri::RunEvent::Opened { urls } = event {
                    if let Err(e) = handle_open_with_app(app_handle, urls) {
                        log::error!("Failed to handle open with app: {:?}", e);
                    }
                }
            },
        );
}
