use crate::domain::{
    BatchCompressionIndividualCompressionResult, BatchCompressionProgress, BatchCompressionResult,
    CancelInProgressCompressionPayload, CompressionResult, CustomEvents, TauriEvents,
    VideoCompressionConfig, VideoCompressionProgress, VideoInfo, VideoMetadataConfig,
    VideoThumbnail,
};
use crate::fs::get_file_metadata;
use crossbeam_channel::{Receiver, Sender};
use nanoid::nanoid;
use regex::Regex;
use serde_json::Value;
use shared_child::SharedChild;
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use strum::EnumProperty;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_shell::ShellExt;

pub struct FFMPEG {
    app: AppHandle,
    ffmpeg: Command,
    assets_dir: PathBuf,
}

const EXTENSIONS: [&str; 5] = ["mp4", "mov", "webm", "avi", "mkv"];

impl FFMPEG {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        match app.shell().sidecar("compresso_ffmpeg") {
            Ok(command) => {
                let app_data_dir = match app.path().app_data_dir() {
                    Ok(path_buf) => path_buf,
                    Err(_) => {
                        return Err(String::from(
                            "Application app directory is not setup correctly.",
                        ));
                    }
                };
                let assets_dir: PathBuf = [PathBuf::from(&app_data_dir), PathBuf::from("assets")]
                    .iter()
                    .collect();

                Ok(Self {
                    app: app.to_owned(),
                    ffmpeg: Command::from(command),
                    assets_dir,
                })
            }
            Err(err) => Err(format!("[ffmpeg-sidecar]: {:?}", err.to_string())),
        }
    }

    /// Compresses a video from a path
    pub async fn compress_video(
        &mut self,
        video_path: &str,
        convert_to_extension: &str,
        preset_name: Option<&str>,
        video_id: &str,
        batch_id: Option<&str>,
        should_mute_video: bool,
        quality: u16,
        dimensions: Option<(u32, u32)>,
        fps: Option<&str>,
        transforms_history: Option<&Vec<Value>>,
        metadata_config: Option<&VideoMetadataConfig>,
        custom_thumbnail_path: Option<&str>,
    ) -> Result<CompressionResult, String> {
        if !EXTENSIONS.contains(&convert_to_extension) {
            return Err(String::from("Invalid convert to extension."));
        }

        let video_id_clone1 = video_id.to_owned();
        let video_id_clone2 = video_id.to_owned();

        let batch_id = match batch_id {
            Some(id) => String::from(id),
            None => nanoid!(),
        };
        let batch_id_clone1 = batch_id.clone();
        let batch_id_clone2 = batch_id.clone();

        let file_name = format!("{}.{}", video_id, convert_to_extension);
        let file_name_clone = file_name.clone();

        let output_file: PathBuf = [self.assets_dir.clone(), PathBuf::from(&file_name)]
            .iter()
            .collect();

        let output_path = &output_file.display().to_string();

        let max_crf: u16 = 36;
        let min_crf: u16 = 24; // Lower the CRF, higher the quality
        let default_crf: u16 = 28;
        let compression_quality = if (0..=100).contains(&quality) {
            let diff = (max_crf - min_crf) - ((max_crf - min_crf) * quality) / 100;
            format!("{}", min_crf + diff)
        } else {
            format!("{default_crf}")
        };
        let compression_quality_str = compression_quality.as_str();

        let codec = "libx264";

        let mut cmd = match preset_name {
            Some(preset) => match preset {
                "thunderbolt" => {
                    let mut args = vec!["-i", &video_path];

                    if convert_to_extension != "webm" {
                        if let Some(thumb_path) = custom_thumbnail_path {
                            if thumb_path.len() > 0 {
                                args.extend_from_slice(&["-i", thumb_path]);
                            }
                        }
                    }

                    args.extend_from_slice(&[
                        "-hide_banner",
                        "-progress",
                        "-",
                        "-nostats",
                        "-loglevel",
                        "error",
                        "-c:v:0",
                        codec,
                        "-crf",
                        compression_quality_str,
                    ]);
                    args
                }
                _ => {
                    let mut args = vec!["-i", &video_path];

                    if convert_to_extension != "webm" {
                        if let Some(thumb_path) = custom_thumbnail_path {
                            if thumb_path.len() > 0 {
                                args.extend_from_slice(&["-i", thumb_path]);
                            }
                        }
                    }

                    args.extend_from_slice(&[
                        "-hide_banner",
                        "-progress",
                        "-",
                        "-nostats",
                        "-loglevel",
                        "error",
                        "-pix_fmt:v:0",
                        "yuv420p",
                        "-c:v:0",
                        codec,
                        "-b:v:0",
                        "0",
                        "-movflags",
                        "+faststart",
                        "-preset",
                        "slow",
                        "-crf",
                        compression_quality_str,
                    ]);
                    args
                }
            },
            None => {
                let mut args = vec!["-i", &video_path];

                if convert_to_extension != "webm" {
                    if let Some(thumb_path) = custom_thumbnail_path {
                        if thumb_path.len() > 0 {
                            args.extend_from_slice(&["-i", thumb_path]);
                        }
                    }
                }

                args.extend_from_slice(&[
                    "-hide_banner",
                    "-progress",
                    "-",
                    "-nostats",
                    "-loglevel",
                    "error",
                    "-c:v:0",
                    codec,
                    "-crf",
                    compression_quality_str,
                ]);
                args
            }
        };
        // Transforms
        let transform_filters = if let Some(transforms) = transforms_history {
            self.build_ffmpeg_filters(transforms)
        } else {
            String::from("")
        };

        // Dimensions
        let padding = "pad=ceil(iw/2)*2:ceil(ih/2)*2";
        let pad_filter = if let Some((width, height)) = dimensions {
            format!("scale={}:{},{}", width, height, padding)
        } else {
            format!("{}", padding)
        };

        let mut vf_filter = String::new();

        if !transform_filters.is_empty() {
            vf_filter.push_str(&transform_filters);
            vf_filter.push_str(",")
        }

        vf_filter.push_str(&pad_filter);

        cmd.push("-filter:v:0");
        cmd.push(&vf_filter);

        // FPS
        if let Some(fps_val) = fps {
            cmd.push("-r");
            cmd.push(fps_val);
        }

        // Webm
        if convert_to_extension == "webm" {
            cmd.push("-c:v:0");
            cmd.push("libvpx-vp9");
        }

        // Mute Audio
        if should_mute_video {
            cmd.push("-an")
        }

        let mut metadata_args: Vec<String> = Vec::new();
        if let Some(metadata) = metadata_config {
            if let Some(ref title) = metadata.title {
                if title.len() > 0 {
                    metadata_args.push("-metadata".to_string());
                    metadata_args.push(format!("title={}", title));
                }
            }
            if let Some(ref artist) = metadata.artist {
                if artist.len() > 0 {
                    metadata_args.push("-metadata".to_string());
                    metadata_args.push(format!("artist={}", artist));
                }
            }
            if let Some(ref album) = metadata.album {
                if album.len() > 0 {
                    metadata_args.push("-metadata".to_string());
                    metadata_args.push(format!("album={}", album));
                }
            }
            if let Some(ref year) = metadata.year {
                if year.len() > 0 {
                    metadata_args.push("-metadata".to_string());
                    metadata_args.push(format!("date={}", year));
                }
            }
            if let Some(ref comment) = metadata.comment {
                if comment.len() > 0 {
                    metadata_args.push("-metadata".to_string());
                    metadata_args.push(format!("comment={}", comment));
                }
            }
            if let Some(ref genre) = metadata.genre {
                if genre.len() > 0 {
                    metadata_args.push("-metadata".to_string());
                    metadata_args.push(format!("genre={}", genre));
                }
            }
            if let Some(ref creation_time) = metadata.creation_time {
                if creation_time.len() > 0 {
                    metadata_args.push("-metadata".to_string());
                    metadata_args.push(format!("creation_time={}", creation_time));
                }
            }
        }

        if custom_thumbnail_path.is_some() && convert_to_extension != "webm" {
            if let Some(thumb_path) = custom_thumbnail_path {
                if thumb_path.len() > 0 {
                    metadata_args.push("-c:v:1".to_string());
                    metadata_args.push("copy".to_string());

                    metadata_args.push("-map".to_string());
                    metadata_args.push("0".to_string());

                    metadata_args.push("-map".to_string());
                    metadata_args.push("1".to_string());

                    metadata_args.push("-disposition:v:1".to_string());
                    metadata_args.push("attached_pic".to_string());
                }
            }
        }

        for arg in metadata_args.iter().map(|s| s.as_str()) {
            cmd.push(arg);
        }

        cmd.push(output_path);
        cmd.push("-y");

        log::info!("[ffmpeg] final command{:?}", cmd);

        let command = self
            .ffmpeg
            .args(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match SharedChild::spawn(command) {
            Ok(child) => {
                let cp = Arc::new(child);
                let cp_clone1 = cp.clone();
                let cp_clone2 = cp.clone();
                let cp_clone3 = cp.clone();
                let cp_clone4 = cp.clone();

                let window = match self.app.get_webview_window("main") {
                    Some(window) => window,
                    None => return Err(String::from("Could not attach to main window")),
                };
                let destroy_event_id =
                    window.listen(TauriEvents::Destroyed.get_str("key").unwrap(), move |_| {
                        log::info!("[tauri] window destroyed");
                        match cp.kill() {
                            Ok(_) => {
                                log::info!("child process killed.");
                            }
                            Err(err) => {
                                log::error!(
                                    "child process could not be killed {}",
                                    err.to_string()
                                );
                            }
                        }
                    });

                let should_cancel = Arc::new(Mutex::new(false));
                let should_cancel_clone = Arc::clone(&should_cancel);

                let cancel_event_id = window.listen(
                    CustomEvents::CancelInProgressCompression.as_ref(),
                    move |evt| {
                        let payload_str = evt.payload();
                        let payload_opt: Option<CancelInProgressCompressionPayload> =
                            serde_json::from_str(payload_str).ok();
                        if let Some(payload) = payload_opt {
                            let batch_id = batch_id_clone1.as_str();
                            if payload.video_id == video_id_clone1
                                || (payload.batch_id.is_some()
                                    && payload.batch_id.unwrap() == batch_id)
                            {
                                log::info!("compression requested to cancel.");
                                match cp_clone4.kill() {
                                    Ok(_) => {
                                        log::info!("child process killed.");
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "child process could not be killed {}",
                                            err.to_string()
                                        );
                                    }
                                };
                                let mut _should_cancel = should_cancel_clone.lock().unwrap();
                                *_should_cancel = true;
                            }
                        }
                    },
                );

                #[cfg(debug_assertions)]
                tokio::spawn(async move {
                    if let Some(stderr) = cp_clone1.take_stderr() {
                        let mut reader = BufReader::new(stderr);

                        loop {
                            let mut buf: Vec<u8> = Vec::new();
                            match tauri::utils::io::read_line(&mut reader, &mut buf) {
                                Ok(n) => {
                                    if n == 0 {
                                        break;
                                    }
                                    if let Ok(val) = std::str::from_utf8(&buf) {
                                        log::debug!("stderr: {:?}", val);
                                    }
                                }
                                Err(_) => {
                                    break;
                                }
                            }
                        }
                    }
                });

                let (tx, rx): (Sender<String>, Receiver<String>) = crossbeam_channel::unbounded();

                let thread: tokio::task::JoinHandle<u8> = tokio::spawn(async move {
                    if let Some(stdout) = cp_clone2.take_stdout() {
                        let mut reader = BufReader::new(stdout);
                        loop {
                            let mut buf: Vec<u8> = Vec::new();
                            match tauri::utils::io::read_line(&mut reader, &mut buf) {
                                Ok(n) => {
                                    if n == 0 {
                                        break;
                                    }
                                    if let Ok(output) = std::str::from_utf8(&buf) {
                                        log::debug!("stdout: {:?}", output);
                                        let re =
                                            Regex::new("out_time=(?<out_time>.*?)\\n").unwrap();
                                        if let Some(cap) = re.captures(output) {
                                            let out_time = &cap["out_time"];
                                            if !out_time.is_empty() {
                                                tx.try_send(String::from(out_time)).ok();
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    break;
                                }
                            }
                        }
                    }

                    if cp_clone2.wait().is_ok() {
                        return 0;
                    }
                    1
                });

                let app_clone = self.app.clone();
                tokio::spawn(async move {
                    let file_name_clone_str = file_name_clone.as_str();

                    while let Ok(current_duration) = rx.recv() {
                        let video_progress = VideoCompressionProgress {
                            video_id: String::from(video_id_clone2.clone()),
                            batch_id: batch_id_clone2.clone(),
                            file_name: String::from(file_name_clone_str),
                            current_duration,
                        };
                        if let Some(window) = app_clone.get_webview_window("main") {
                            window
                                .emit(
                                    CustomEvents::VideoCompressionProgress.as_ref(),
                                    video_progress,
                                )
                                .ok();
                        }
                    }
                });

                let message: String = match thread.await {
                    Ok(exit_status) => {
                        if exit_status == 1 {
                            String::from("Video is corrupted.")
                        } else {
                            String::from("")
                        }
                    }
                    Err(err) => err.to_string(),
                };

                // Cleanup
                window.unlisten(destroy_event_id);
                window.unlisten(cancel_event_id);
                match cp_clone3.kill() {
                    Ok(_) => {
                        log::info!("child process killed.");
                    }
                    Err(err) => {
                        log::error!("child process could not be killed {}", err.to_string());
                    }
                }

                let is_cancelled = should_cancel.lock().unwrap();
                if *is_cancelled {
                    return Err(String::from("CANCELLED"));
                }

                if !message.is_empty() {
                    return Err(message);
                }
            }
            Err(err) => {
                return Err(err.to_string());
            }
        };

        let file_metadata = get_file_metadata(&output_file.to_string_lossy().to_string());
        Ok(CompressionResult {
            video_id: video_id.to_owned(),
            file_name,
            file_path: output_file.display().to_string(),
            file_metadata: file_metadata.ok(),
        })
    }

    /// Compressed videos in batch
    pub async fn compress_videos_batch(
        &mut self,
        batch_id: &str,
        videos: Vec<VideoCompressionConfig>,
    ) -> Result<BatchCompressionResult, String> {
        let mut results: std::collections::HashMap<String, CompressionResult> =
            std::collections::HashMap::new();
        let total_count = videos.len();

        for (index, video_options) in videos.iter().enumerate() {
            let video_path = &video_options.video_path;
            let video_id = &video_options.video_id;

            let app_clone = self.app.clone();
            let batch_id_clone = batch_id.to_string();
            let video_id_clone = video_id.clone();

            tokio::spawn(async move {
                if let Some(window) = app_clone.get_webview_window("main") {
                    let _ = window.clone().listen(
                        CustomEvents::VideoCompressionProgress.as_ref(),
                        move |evt| {
                            if let Ok(progress) =
                                serde_json::from_str::<VideoCompressionProgress>(evt.payload())
                            {
                                if progress.video_id == video_id_clone {
                                    let batch_progress = BatchCompressionProgress {
                                        batch_id: batch_id_clone.to_owned(),
                                        current_index: index,
                                        total_count,
                                        video_progress: progress,
                                    };
                                    let _ = window.emit(
                                        CustomEvents::BatchCompressionProgress.as_ref(),
                                        batch_progress,
                                    );
                                }
                            }
                        },
                    );
                }
            });

            let mut ffmpeg_instance = match FFMPEG::new(&self.app) {
                Ok(f) => f,
                Err(e) => return Err(format!("Failed to create ffmpeg instance: {}", e)),
            };

            let app_clone2 = self.app.clone();
            let batch_id_clone2 = batch_id.to_string();

            let convert_to_extension = &video_options.convert_to_extension;
            let preset_name = video_options.preset_name.as_deref();
            let batch_id_for_compression = batch_id.clone();
            let should_mute_video = video_options.should_mute_video;
            let quality = video_options.quality;
            let dimensions = video_options.dimensions;
            let fps = video_options.fps.as_deref();
            let transforms_history = video_options
                .transforms_history
                .as_ref()
                .map(|v| v.as_ref());
            let metadata_config = video_options.metadata_config.as_ref();
            let thumbnail_path = video_options.custom_thumbnail_path.as_deref();

            match ffmpeg_instance
                .compress_video(
                    video_path,
                    convert_to_extension,
                    preset_name,
                    video_id,
                    Some(batch_id_for_compression),
                    should_mute_video,
                    quality,
                    dimensions,
                    fps,
                    transforms_history,
                    metadata_config,
                    thumbnail_path,
                )
                .await
            {
                Ok(result) => {
                    let video_id = result.video_id.clone();
                    results.insert(video_id, result.clone());

                    tokio::spawn(async move {
                        if let Some(window) = app_clone2.get_webview_window("main") {
                            let individual_compression_result: BatchCompressionIndividualCompressionResult =
                                BatchCompressionIndividualCompressionResult {
                                    batch_id: batch_id_clone2,
                                    result: result,
                                };
                            let _ = window.emit(
                                CustomEvents::BatchCompressionIndividualCompressionCompletion
                                    .as_ref(),
                                individual_compression_result,
                            );
                        }
                    });
                }
                Err(e) => {
                    if e == "CANCELLED" {
                        return Err(String::from("CANCELLED"));
                    }
                    log::error!("Failed to compress video at index {}: {}", index, e);
                }
            }
        }

        Ok(BatchCompressionResult { results })
    }

    /// Generates a .jpeg thumbnail image from a video path
    pub async fn generate_video_thumbnail(
        &mut self,
        video_path: &str,
    ) -> Result<VideoThumbnail, String> {
        if !Path::exists(Path::new(video_path)) {
            return Err(String::from("File does not exist in given path."));
        }
        let id = nanoid!();
        let file_name = format!("{}.jpg", id);
        let output_path: PathBuf = [self.assets_dir.clone(), PathBuf::from(&file_name)]
            .iter()
            .collect();

        let command = self.ffmpeg.args([
            "-i",
            video_path,
            "-ss",
            "00:00:01.00",
            "-vframes",
            "1",
            &output_path.display().to_string(),
            "-y",
        ]);

        match SharedChild::spawn(command) {
            Ok(child) => {
                let cp = Arc::new(child);
                let cp_clone1 = cp.clone();
                let cp_clone2 = cp.clone();

                let window = match self.app.get_webview_window("main") {
                    Some(window) => window,
                    None => return Err(String::from("Could not attach to main window")),
                };
                let destroy_event_id = window.listen(
                    TauriEvents::Destroyed.get_str("key").unwrap(),
                    move |_| match cp.kill() {
                        Ok(_) => {
                            log::info!("child process killed.");
                        }
                        Err(err) => {
                            log::error!("child process could not be killed {}", err.to_string());
                        }
                    },
                );

                let thread: tokio::task::JoinHandle<u8> = tokio::spawn(async move {
                    if cp_clone1.wait().is_ok() {
                        return 0;
                    }
                    1
                });

                let message: String = match thread.await {
                    Ok(exit_status) => {
                        if exit_status == 1 {
                            String::from("Video is corrupted.")
                        } else {
                            String::from("")
                        }
                    }
                    Err(err) => err.to_string(),
                };

                // Cleanup
                window.unlisten(destroy_event_id);
                match cp_clone2.kill() {
                    Ok(_) => {
                        log::info!("child process killed.");
                    }
                    Err(err) => {
                        log::error!("child process could not be killed {}", err.to_string());
                    }
                }
                if !message.is_empty() {
                    return Err(message);
                }
            }
            Err(err) => return Err(err.to_string()),
        };
        Ok(VideoThumbnail {
            id,
            file_name,
            file_path: output_path.display().to_string(),
        })
    }

    pub fn get_asset_dir(&self) -> String {
        self.assets_dir.display().to_string()
    }

    pub async fn get_video_info(&mut self, video_path: &str) -> Result<VideoInfo, String> {
        if !Path::exists(Path::new(video_path)) {
            return Err(String::from("File does not exist in given path."));
        }

        let command = self
            .ffmpeg
            .args(["-i", video_path, "-hide_banner"])
            .stderr(Stdio::piped()); // Capture stderr for metadata parsing

        match SharedChild::spawn(command) {
            Ok(child) => {
                let cp = Arc::new(child);
                let cp_clone1 = cp.clone();
                let cp_clone2 = cp.clone();

                let window = match self.app.get_webview_window("main") {
                    Some(window) => window,
                    None => return Err(String::from("Could not attach to main window")),
                };

                let destroy_event_id = window.listen(
                    TauriEvents::Destroyed.get_str("key").unwrap(),
                    move |_| match cp.kill() {
                        Ok(_) => log::info!("child process killed."),
                        Err(err) => log::error!("child process could not be killed {}", err),
                    },
                );

                let thread: tokio::task::JoinHandle<(
                    u8,
                    Option<String>,
                    Option<(u32, u32)>,
                    Option<f32>,
                )> = tokio::task::spawn(async move {
                    let mut duration: Option<String> = None;
                    let mut dimensions: Option<(u32, u32)> = None;
                    let mut fps: Option<f32> = None;

                    if let Some(stderr) = cp_clone1.take_stderr() {
                        let reader = BufReader::new(stderr);
                        let duration_re = Regex::new(r"Duration: (?P<duration>.*?),").unwrap();
                        let dimension_re =
                            Regex::new(r"Video:.*?,.*? (?P<width>\d{2,5})x(?P<height>\d{2,5})")
                                .unwrap();
                        let fps_re = Regex::new(r"(?P<fps>\d+(\.\d+)?) fps").unwrap();

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                if duration.is_none() {
                                    if let Some(cap) = duration_re.captures(&line) {
                                        duration = Some(cap["duration"].to_string());
                                    }
                                }
                                if dimensions.is_none() {
                                    if let Some(cap) = dimension_re.captures(&line) {
                                        if let (Ok(w), Ok(h)) = (
                                            cap["width"].parse::<u32>(),
                                            cap["height"].parse::<u32>(),
                                        ) {
                                            dimensions = Some((w, h));
                                        }
                                    }
                                }
                                if fps.is_none() {
                                    if let Some(cap) = fps_re.captures(&line) {
                                        if let Ok(parsed_fps) = cap["fps"].parse::<f32>() {
                                            fps = Some(parsed_fps);
                                        }
                                    }
                                }
                                if duration.is_some() && dimensions.is_some() && fps.is_some() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }

                    if cp_clone1.wait().is_ok() {
                        (0, duration, dimensions, fps)
                    } else {
                        (1, duration, dimensions, fps)
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, duration, dimensions, fps)) => {
                        if exit_status == 1 {
                            Err("Video file is corrupted".to_string())
                        } else {
                            Ok(VideoInfo {
                                duration,
                                dimensions,
                                fps,
                            })
                        }
                    }
                    Err(err) => Err(err.to_string()),
                };

                // Cleanup
                window.unlisten(destroy_event_id);
                if let Err(err) = cp_clone2.kill() {
                    log::error!("child process could not be killed {}", err);
                }

                result
            }
            Err(err) => Err(err.to_string()),
        }
    }

    fn build_ffmpeg_filters(&self, actions: &Vec<Value>) -> String {
        let mut filters: Vec<String> = Vec::new();
        let mut latest_crop: Option<&Value> = None;

        for action in actions {
            let action_type = action["type"].as_str().unwrap_or("");

            match action_type {
                "rotate" => {
                    let angle = action["value"].as_i64().unwrap_or(0);
                    match angle % 360 {
                        -90 | 270 => filters.push("transpose=2".to_string()),
                        90 | -270 => filters.push("transpose=1".to_string()),
                        180 | -180 => filters.push("hflip,vflip".to_string()),
                        _ => {}
                    }
                }
                "flip" => {
                    if let Some(flip_obj) = action["value"].as_object() {
                        if flip_obj.get("horizontal").and_then(|v| v.as_bool()) == Some(true) {
                            filters.push("hflip".to_string());
                        }
                        if flip_obj.get("vertical").and_then(|v| v.as_bool()) == Some(true) {
                            filters.push("vflip".to_string());
                        }
                    }
                }
                "crop" => {
                    latest_crop = Some(&action["value"]);
                }
                _ => {}
            }
        }

        // Apply only the last crop
        if let Some(c) = latest_crop {
            let w = c["width"].as_f64().unwrap_or(0.0).round() as i64;
            let h = c["height"].as_f64().unwrap_or(0.0).round() as i64;
            let x = c["left"].as_f64().unwrap_or(0.0).round() as i64;
            let y = c["top"].as_f64().unwrap_or(0.0).round() as i64;

            filters.push(format!("crop={}:{}:{}:{}", w, h, x, y));
        }

        filters.join(",")
    }
}
