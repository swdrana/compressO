use crate::domain::{
    BatchCompressionIndividualCompressionResult, BatchCompressionProgress, BatchCompressionResult,
    CancelInProgressCompressionPayload, CompressionResult, CustomEvents, TauriEvents, TrimSegment,
    VideoCompressionConfig, VideoCompressionProgress, VideoMetadataConfig, VideoThumbnail,
};
use crate::ffprobe::FFPROBE;
use crate::fs::get_file_metadata;
use crossbeam_channel::{Receiver, Sender};
use nanoid::nanoid;
use regex::Regex;
use serde_json::Value;
use shared_child::SharedChild;
use std::{
    io::BufReader,
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
        video_codec: Option<&str>,
        transforms_history: Option<&Vec<Value>>,
        metadata_config: Option<&VideoMetadataConfig>,
        custom_thumbnail_path: Option<&str>,
        trim_segments: Option<&Vec<TrimSegment>>,
    ) -> Result<CompressionResult, String> {
        if !EXTENSIONS.contains(&convert_to_extension) {
            return Err(String::from("Invalid convert to extension."));
        }

        let has_audio_stream = {
            let mut ffprobe = FFPROBE::new(&self.app)?;
            ffprobe.has_audio_stream(video_path).await?
        };

        println!(">> has_audio_stream {}", has_audio_stream);

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

        let mut cmd_args: Vec<&str> = Vec::new();

        cmd_args.push("-i");
        cmd_args.push(video_path);

        if convert_to_extension != "webm" {
            if let Some(thumb_path) = custom_thumbnail_path {
                if thumb_path.len() > 0 {
                    cmd_args.extend_from_slice(&["-i", thumb_path]);
                }
            }
        }

        // Preserve existing metadata
        cmd_args.extend_from_slice(&["-map_metadata", "0"]);

        cmd_args.extend_from_slice(&[
            "-hide_banner",
            "-progress",
            "-",
            "-nostats",
            "-loglevel",
            "error",
        ]);

        let mut cmd_args = match preset_name {
            Some(preset) => match preset {
                "thunderbolt" => cmd_args,
                _ => {
                    cmd_args.extend_from_slice(&[
                        "-pix_fmt:v:0",
                        "yuv420p",
                        "-b:v:0",
                        "0",
                        "-movflags",
                        "+faststart",
                        "-preset",
                        "slow",
                    ]);
                    cmd_args
                }
            },
            None => cmd_args,
        };

        // Codec
        let output_codec: String = {
            fn default_codec(convert_to_extension: &str) -> String {
                match convert_to_extension {
                    "webm" => "libvpx-vp9".to_string(),
                    _ => "libx264".to_string(),
                }
            }
            if let Some(codec) = video_codec {
                codec.to_string()
            } else {
                if preset_name.is_none() {
                    let source_codec = {
                        let mut ffprobe = FFPROBE::new(&self.app)?;
                        ffprobe.get_video_codec(video_path).await?
                    };

                    match source_codec {
                        Some(codec_name) => codec_name.to_string(),
                        None => default_codec(convert_to_extension),
                    };
                }
                default_codec(convert_to_extension)
            }
        };
        println!(">> {:?}", output_codec);
        cmd_args.extend_from_slice(&["-c:v:0", output_codec.as_str()]);

        // Quality
        let max_crf: u16 = 36;
        let min_crf: u16 = 24;
        let default_crf: u16 = 28;
        let compression_quality = if (0..=100).contains(&quality) {
            let diff = (max_crf - min_crf) - ((max_crf - min_crf) * quality) / 100;
            format!("{}", min_crf + diff)
        } else {
            format!("{default_crf}")
        };
        if preset_name.is_some() || (0..=100).contains(&quality) {
            cmd_args.extend_from_slice(&["-crf", compression_quality.as_str()]);
        }

        // Transforms
        let transform_filters = if let Some(transforms) = transforms_history {
            self.build_ffmpeg_filters(transforms)
        } else {
            String::from("")
        };

        // Dimensions
        let padding = "pad=ceil(iw/2)*2:ceil(ih/2)*2";

        // Build the post-processing chain for video (transforms + scale + pad)
        let mut video_post_chain: Vec<String> = Vec::new();
        if !transform_filters.is_empty() {
            video_post_chain.push(transform_filters.clone());
        }
        if let Some((width, height)) = dimensions {
            video_post_chain.push(format!("scale={}:{}", width, height));
        }
        video_post_chain.push(String::from(padding));
        let video_post_process = video_post_chain.join(",");

        // Build unified filter_complex string
        let mut filter_complex_parts: Vec<String> = Vec::new();
        let mut map_video = false;
        let mut map_audio = false;

        if let Some(segments) = trim_segments {
            if !segments.is_empty() {
                map_video = true;
                if segments.len() == 1 {
                    let seg = &segments[0];
                    // Single trim: trim -> post_process -> [outv]
                    filter_complex_parts.push(format!(
                        "[0:v]trim={}:{},setpts=PTS-STARTPTS,{}[outv]",
                        seg.start, seg.end, video_post_process
                    ));
                } else {
                    // Multi trim: trim segments -> concat -> post_process -> [outv]
                    let mut video_parts = Vec::new();
                    let mut video_labels = Vec::new();
                    for (i, seg) in segments.iter().enumerate() {
                        let label = format!("v{}", i);
                        video_labels.push(format!("[{}]", label));
                        video_parts.push(format!(
                            "[0:v]trim={}:{},setpts=PTS-STARTPTS[{}]",
                            seg.start, seg.end, label
                        ));
                    }
                    filter_complex_parts.push(video_parts.join("; "));

                    // Concat segments, apply post-processing, output to [outv]
                    filter_complex_parts.push(format!(
                        "{} concat=n={}:v=1:a=0,{}[outv]",
                        video_labels.join(""),
                        segments.len(),
                        video_post_process
                    ));
                }
            }
        }

        // If no trimming, just apply post-processing to input
        if !map_video {
            filter_complex_parts.push(format!("[0:v]{}[outv]", video_post_process));
            map_video = true;
        }

        // Only process audio if not muted AND trimming is present
        if !should_mute_video && has_audio_stream {
            if let Some(segments) = trim_segments {
                if !segments.is_empty() {
                    map_audio = true;
                    if segments.len() == 1 {
                        let seg = &segments[0];
                        filter_complex_parts.push(format!(
                            "[0:a]atrim={}:{},asetpts=PTS-STARTPTS[outa]",
                            seg.start, seg.end
                        ));
                    } else {
                        let mut audio_parts = Vec::new();
                        let mut audio_labels = Vec::new();
                        for (i, seg) in segments.iter().enumerate() {
                            let label = format!("a{}", i);
                            audio_labels.push(format!("[{}]", label));
                            audio_parts.push(format!(
                                "[0:a]atrim={}:{},asetpts=PTS-STARTPTS[{}]",
                                seg.start, seg.end, label
                            ));
                        }
                        filter_complex_parts.push(format!(
                            "{}; {} concat=n={}:v=0:a=1[outa]",
                            audio_parts.join("; "),
                            audio_labels.join(""),
                            segments.len()
                        ));
                    }
                }
            }
        }

        let fc = match filter_complex_parts.len() {
            0 => "".to_string(),
            _ => filter_complex_parts.join(";").to_string(),
        };

        if !fc.is_empty() {
            cmd_args.extend_from_slice(&["-filter_complex", &fc]);
        }

        // FPS
        if let Some(fps_val) = fps {
            cmd_args.push("-r");
            cmd_args.push(fps_val);
        }

        // Map output video
        if map_video {
            cmd_args.extend_from_slice(&["-map", "[outv]"]);
        }

        // Map output audio
        if map_audio {
            cmd_args.extend_from_slice(&["-map", "[outa]"]);
        } else if !should_mute_video && has_audio_stream {
            cmd_args.extend_from_slice(&["-map", "0:a?"]);
        }

        // Mute Audio (overrides everything if set)
        if should_mute_video {
            cmd_args.push("-an");
        }

        let mut metadata_args: Vec<String> = Vec::new();
        if let Some(metadata) = metadata_config {
            if let Some(ref title) = metadata.title {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("title={}", title.trim()));
            }
            if let Some(ref artist) = metadata.artist {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("artist={}", artist.trim()));
            }
            if let Some(ref album) = metadata.album {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("album={}", album.trim()));
            }
            if let Some(ref year) = metadata.year {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("date={}", year.trim()));
            }
            if let Some(ref comment) = metadata.comment {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("comment={}", comment.trim()));
            }
            if let Some(ref description) = metadata.description {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("description={}", description.trim()));
            }
            if let Some(ref synopsis) = metadata.synopsis {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("synopsis={}", synopsis.trim()));
            }
            if let Some(ref genre) = metadata.genre {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("genre={}", genre.trim()));
            }
            if let Some(ref creation_time) = metadata.creation_time {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("creation_time={}", creation_time.trim()));
            }
        }

        // Remove the `Chapters` metadata forcefully if video has been trimmed
        if let Some(segments) = trim_segments {
            if !segments.is_empty() {
                metadata_args.extend_from_slice(&["-map_chapters".to_string(), "-1".to_string()]);
            }
        }

        for arg in metadata_args.iter().map(|s| s.as_str()) {
            cmd_args.push(arg);
        }

        if custom_thumbnail_path.is_some() && convert_to_extension != "webm" {
            if let Some(thumb_path) = custom_thumbnail_path {
                if thumb_path.len() > 0 {
                    cmd_args.push("-c:v:1");
                    if thumb_path.to_lowercase().ends_with(".webp") {
                        cmd_args.push("png");
                    } else {
                        cmd_args.push("copy");
                    }
                    cmd_args.extend_from_slice(&["-map", "1"]);
                    cmd_args.extend_from_slice(&["-disposition:v:1", "attached_pic"]);
                }
            }
        }

        // Output path
        let output_path = output_file.display().to_string();
        cmd_args.extend_from_slice(&["-y", &output_path]);

        log::info!("[ffmpeg] final command{:?}", cmd_args);

        let command = self
            .ffmpeg
            .args(cmd_args)
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
                                log::info!("[ffmpeg-sidecar] child process killed.");
                            }
                            Err(err) => {
                                log::error!(
                                    "[ffmpeg-sidecar] child process could not be killed {}",
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
                                        log::info!("[ffmpeg-sidecar] child process killed.");
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "[ffmpeg-sidecar] child process could not be killed {}",
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

                    match cp_clone2.wait() {
                        Ok(status) if status.success() => 0,
                        _ => 1,
                    }
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
                        log::info!("[ffmpeg-sidecar] child process killed.");
                    }
                    Err(err) => {
                        log::error!(
                            "[ffmpeg-sidecar] child process could not be killed {}",
                            err.to_string()
                        );
                    }
                }

                let is_cancelled = should_cancel.lock().unwrap();
                if *is_cancelled {
                    // Delete the partial output file
                    std::fs::remove_file(&output_file).ok();
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
            let batch_id_for_compression = batch_id;
            let should_mute_video = video_options.should_mute_video;
            let quality = video_options.quality;
            let dimensions = video_options.dimensions;
            let fps = video_options.fps.as_deref();
            let video_codec = video_options.video_codec.as_deref();
            let transforms_history = video_options
                .transforms_history
                .as_ref()
                .map(|v| v.as_ref());
            let metadata_config = video_options.metadata_config.as_ref();
            let thumbnail_path = video_options.custom_thumbnail_path.as_deref();
            let trim_segments = video_options.trim_segments.as_ref();

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
                    video_codec,
                    transforms_history,
                    metadata_config,
                    thumbnail_path,
                    trim_segments,
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
        timestamp: Option<&str>,
    ) -> Result<VideoThumbnail, String> {
        if !Path::exists(Path::new(video_path)) {
            return Err(String::from("File does not exist in given path."));
        }
        let id = nanoid!();
        let file_name = format!("{}.jpg", id);
        let output_path: PathBuf = [self.assets_dir.clone(), PathBuf::from(&file_name)]
            .iter()
            .collect();

        let timestamp_value = timestamp.unwrap_or("00:00:01.00");

        let command = self.ffmpeg.args([
            "-ss",
            timestamp_value,
            "-i",
            video_path,
            "-frames:v",
            "1",
            "-an",
            "-sn",
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
                            log::info!("[ffmpeg-sidecar] child process killed.");
                        }
                        Err(err) => {
                            log::error!(
                                "[ffmpeg-sidecar] child process could not be killed {}",
                                err.to_string()
                            );
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
                        log::info!("[ffmpeg-sidecar] child process killed.");
                    }
                    Err(err) => {
                        log::error!(
                            "[ffmpeg-sidecar] child process could not be killed {}",
                            err.to_string()
                        );
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
