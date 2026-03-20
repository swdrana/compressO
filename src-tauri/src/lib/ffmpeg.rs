use crate::domain::{
    AudioConfig, BatchCompressionResult, BatchVideoCompressionProgress,
    BatchVideoIndividualCompressionResult, CancelInProgressCompressionPayload, CustomEvents,
    SubtitlesConfig, TauriEvents, VideoCompressionConfig, VideoCompressionProgress,
    VideoCompressionResult, VideoMetadataConfig, VideoThumbnail, VideoTrimSegment,
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
    assets_dir: PathBuf,
}

const EXTENSIONS: [&str; 5] = ["mp4", "mov", "webm", "avi", "mkv"];

impl FFMPEG {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
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
            assets_dir,
        })
    }

    pub fn get_asset_dir(&self) -> String {
        self.assets_dir.display().to_string()
    }

    pub fn get_ffmpeg_command(&self) -> Result<Command, String> {
        self.app
            .shell()
            .sidecar("compresso_ffmpeg")
            .map(Command::from)
            .map_err(|e| format!("Failed to create ffmpeg command: {}", e))
    }

    /// Compresses a video from a path
    pub async fn compress_video(
        &mut self,
        video_path: &str,
        convert_to_extension: &str,
        preset_name: Option<&str>,
        video_id: &str,
        batch_id: Option<&str>,
        audio_config: &AudioConfig,
        quality: u16,
        dimensions: Option<(u32, u32)>,
        fps: Option<&str>,
        video_codec: Option<&str>,
        transforms_history: Option<&Vec<Value>>,
        metadata_config: Option<&VideoMetadataConfig>,
        custom_thumbnail_path: Option<&str>,
        trim_segments: Option<&Vec<VideoTrimSegment>>,
        subtitles_config: Option<&SubtitlesConfig>,
    ) -> Result<VideoCompressionResult, String> {
        if !EXTENSIONS.contains(&convert_to_extension) {
            return Err(String::from("Invalid convert to extension."));
        }

        let audio_streams = {
            let mut ffprobe = FFPROBE::new(&self.app)?;
            ffprobe.get_audio_streams(video_path).await?
        };
        let has_audio_stream = !audio_streams.is_empty();

        // Detect existing subtitle streams in source video
        let (existing_subtitle_count, _) = {
            if let Some(subs_config) = subtitles_config {
                let preserve = subs_config.preserve_existing_subtitles.unwrap_or(false);
                if preserve {
                    let mut ffprobe = FFPROBE::new(&self.app)?;
                    let count = ffprobe.get_subtitle_streams(video_path).await?.len();
                    (count, true)
                } else {
                    (0, false)
                }
            } else {
                (0, false)
            }
        };
        let has_existing_subtitles = existing_subtitle_count > 0;

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

        // Track input indices for mapping
        let mut input_index: usize = 1; // 0 is video, 1+ are thumbnails/subtitles

        if convert_to_extension != "webm" {
            if let Some(thumb_path) = custom_thumbnail_path {
                if thumb_path.len() > 0 {
                    cmd_args.extend_from_slice(&["-i", thumb_path]);
                    input_index += 1;
                }
            }
        }

        let subtitle_input_indices: Vec<(usize, String)> = if convert_to_extension != "webm" {
            if let Some(subs_config) = subtitles_config {
                if subs_config.should_enable_subtitles.unwrap_or(false) {
                    let mut indices = Vec::new();
                    for sub in &subs_config.subtitles {
                        if let Some(ref sub_path) = sub.subtitle_path {
                            if sub_path.len() > 0 {
                                cmd_args.extend_from_slice(&["-i", sub_path]);
                                let lang = if sub.language == "und" {
                                    String::new()
                                } else {
                                    sub.language.clone()
                                };
                                indices.push((input_index, lang));
                                input_index += 1;
                            }
                        }
                    }
                    indices
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

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
                    let source_streams = {
                        let mut ffprobe = FFPROBE::new(&self.app)?;
                        ffprobe.get_video_streams(video_path).await?
                    };

                    match source_streams.first() {
                        Some(stream) => stream.codec.clone(),
                        None => default_codec(convert_to_extension),
                    }
                } else {
                    default_codec(convert_to_extension)
                }
            }
        };
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

        let mut filter_complex_parts: Vec<String> = Vec::new();
        let mut map_video = false;
        let mut map_audio = false;

        let volume_filter_str = if audio_config.volume > 0 && audio_config.volume != 100 {
            let volume_value = audio_config.volume as f32 / 100.0;
            format!("volume={}", volume_value)
        } else {
            "".to_string()
        };

        let channel_filter_str =
            if let Some(channel_config) = audio_config.audio_channel_config.as_ref() {
                if let Some(ref layout) = channel_config.channel_layout {
                    match layout.as_str() {
                        "mono" => {
                            if let Some(ref mono_source) = channel_config.mono_source {
                                match (mono_source.left, mono_source.right) {
                                    (true, true) => "aformat=channel_layouts=mono".to_string(),
                                    (true, false) => "pan=mono|c0=c0".to_string(),
                                    (false, true) => "pan=mono|c0=c1".to_string(),
                                    (false, false) => "aformat=channel_layouts=mono".to_string(),
                                }
                            } else {
                                "aformat=channel_layouts=mono".to_string()
                            }
                        }
                        "stereo" => {
                            if channel_config.stereo_swap_channels == Some(true) {
                                "pan=stereo|c0=c1|c1=c0".to_string()
                            } else {
                                "".to_string()
                            }
                        }
                        _ => "".to_string(),
                    }
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            };

        let combined_audio_filter =
            if !channel_filter_str.is_empty() && !volume_filter_str.is_empty() {
                format!("{},{}", channel_filter_str, volume_filter_str)
            } else if !channel_filter_str.is_empty() {
                channel_filter_str
            } else if !volume_filter_str.is_empty() {
                volume_filter_str
            } else {
                "".to_string()
            };

        let combined_audio_filter_with_comma = if !combined_audio_filter.is_empty() {
            format!(",{}", combined_audio_filter)
        } else {
            "".to_string()
        };

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
                    // Multi trim: trim segments -> concat -> post process -> [outv]
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

        if audio_config.volume > 0 && has_audio_stream {
            if let Some(segments) = trim_segments {
                if !segments.is_empty() {
                    map_audio = true;

                    let audio_tracks_to_process: Vec<usize> =
                        if let Some(ref selected_tracks) = audio_config.selected_audio_tracks {
                            selected_tracks.clone()
                        } else {
                            (0..audio_streams.len()).collect()
                        };

                    for (track_idx, track_index) in audio_tracks_to_process.iter().enumerate() {
                        let out_label = if audio_tracks_to_process.len() == 1 {
                            "outa".to_string()
                        } else {
                            format!("outa{}", track_idx)
                        };

                        if segments.len() == 1 {
                            let seg = &segments[0];
                            filter_complex_parts.push(format!(
                                "[0:a:{}]atrim={}:{},asetpts=PTS-STARTPTS{}[{}]",
                                track_index,
                                seg.start,
                                seg.end,
                                combined_audio_filter_with_comma,
                                out_label
                            ));
                        } else {
                            let mut audio_parts = Vec::new();
                            let mut audio_labels = Vec::new();
                            for (i, seg) in segments.iter().enumerate() {
                                let label = format!("a{}t{}", track_idx, i);
                                audio_labels.push(format!("[{}]", label));
                                audio_parts.push(format!(
                                    "[0:a:{}]atrim={}:{},asetpts=PTS-STARTPTS[{}]",
                                    track_index, seg.start, seg.end, label
                                ));
                            }
                            filter_complex_parts.push(format!(
                                "{}; {} concat=n={}:v=0:a=1{}[{}]",
                                audio_parts.join("; "),
                                audio_labels.join(""),
                                segments.len(),
                                combined_audio_filter_with_comma,
                                out_label
                            ));
                        }
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

        let mut audio_args_owned: Vec<String> = Vec::new();

        // Map output audio
        if map_audio {
            let processed_tracks: Vec<usize> =
                if let Some(ref selected_tracks) = audio_config.selected_audio_tracks {
                    selected_tracks.clone()
                } else {
                    (0..audio_streams.len()).collect()
                };

            for (track_idx, _) in processed_tracks.iter().enumerate() {
                audio_args_owned.push("-map".to_string());

                let out_label = if processed_tracks.len() == 1 {
                    "[outa]".to_string()
                } else {
                    format!("[outa{}]", track_idx)
                };

                audio_args_owned.push(out_label);
            }
        } else if audio_config.volume > 0 && has_audio_stream {
            if let Some(ref selected_tracks) = audio_config.selected_audio_tracks {
                for &track_index in selected_tracks {
                    audio_args_owned.push("-map".to_string());
                    audio_args_owned.push(format!("0:a:{}", track_index));
                }
            } else {
                cmd_args.extend_from_slice(&["-map", "0:a?"]);
            }
        }

        // Audio filter
        let audio_filter_args: Vec<String> = {
            if has_audio_stream
                && !map_audio
                && (!combined_audio_filter.is_empty()
                    || (audio_config.volume > 0 && audio_config.volume != 100))
            {
                let mut args = vec![];
                if let Some(ref selected_tracks) = audio_config.selected_audio_tracks {
                    for &track_index in selected_tracks {
                        args.push(format!("-filter:a:{}", track_index));
                        args.push(combined_audio_filter.clone());
                    }
                } else {
                    for track_index in 0..audio_streams.len() {
                        args.push(format!("-filter:a:{}", track_index));
                        args.push(combined_audio_filter.clone());
                    }
                }
                args
            } else {
                vec![]
            }
        };
        audio_args_owned.extend(audio_filter_args);

        // Audio bitrate
        if audio_config.volume > 0 && has_audio_stream {
            if let Some(bitrate) = audio_config.bitrate {
                audio_args_owned.push("-b:a".to_string());
                audio_args_owned.push(format!("{}k", bitrate));
            }
        }

        // Audio codec
        if audio_config.volume > 0 && has_audio_stream {
            if let Some(codec) = &audio_config.audio_codec {
                audio_args_owned.push("-c:a".to_string());
                audio_args_owned.push(codec.clone());
            }
        }

        cmd_args.extend(audio_args_owned.iter().map(|s| s.as_str()));

        if audio_config.volume == 0 {
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
            if let Some(ref copyright) = metadata.copyright {
                metadata_args.push("-metadata".to_string());
                metadata_args.push(format!("copyright={}", copyright.trim()));
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

        let mut subtitle_args_owned: Vec<String> = Vec::new();
        let mut subtitle_index = 0usize;

        if has_existing_subtitles {
            for idx in 0..existing_subtitle_count {
                subtitle_args_owned.push("-map".to_string());
                subtitle_args_owned.push(format!("0:s:{}", idx));

                let subtitle_codec = match convert_to_extension {
                    "mkv" => "srt",
                    _ => "mov_text",
                };
                subtitle_args_owned.push(format!("-c:s:{}", subtitle_index));
                subtitle_args_owned.push(subtitle_codec.to_string());
                subtitle_index += 1;
            }
        }

        for (sub_input_idx, language) in subtitle_input_indices.iter() {
            subtitle_args_owned.push("-map".to_string());
            subtitle_args_owned.push(format!("{}:s", sub_input_idx));

            let subtitle_codec = match convert_to_extension {
                "mkv" => "srt",
                _ => "mov_text",
            };
            subtitle_args_owned.push(format!("-c:s:{}", subtitle_index));
            subtitle_args_owned.push(subtitle_codec.to_string());

            if !language.is_empty() {
                subtitle_args_owned.push(format!("-metadata:s:s:{}", subtitle_index));
                subtitle_args_owned.push(format!("language={}", language));
            }
            subtitle_index += 1;
        }
        cmd_args.extend(subtitle_args_owned.iter().map(|s| s.as_str()));

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

        let mut ffmpeg_cmd: Command = self.get_ffmpeg_command()?;

        let command = ffmpeg_cmd
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
                            if payload.media_id == video_id_clone1
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
        Ok(VideoCompressionResult {
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
        let mut results: std::collections::HashMap<String, VideoCompressionResult> =
            std::collections::HashMap::new();
        let total_count = videos.len();

        for (index, video_options) in videos.iter().enumerate() {
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
                                    let batch_progress = BatchVideoCompressionProgress {
                                        batch_id: batch_id_clone.to_owned(),
                                        current_index: index,
                                        total_count,
                                        video_progress: progress,
                                    };
                                    let _ = window.emit(
                                        CustomEvents::BatchVideoCompressionProgress.as_ref(),
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

            let video_path = &video_options.video_path;
            let convert_to_extension = &video_options.convert_to_extension;
            let preset_name = video_options.preset_name.as_deref();
            let batch_id_for_compression = batch_id;
            let audio_config = &video_options.audio_config;
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
            let subtitles_config = video_options.subtitles_config.as_ref();

            match ffmpeg_instance
                .compress_video(
                    video_path,
                    convert_to_extension,
                    preset_name,
                    video_id,
                    Some(batch_id_for_compression),
                    audio_config,
                    quality,
                    dimensions,
                    fps,
                    video_codec,
                    transforms_history,
                    metadata_config,
                    thumbnail_path,
                    trim_segments,
                    subtitles_config,
                )
                .await
            {
                Ok(result) => {
                    let video_id = result.video_id.clone();
                    results.insert(video_id, result.clone());

                    tokio::spawn(async move {
                        if let Some(window) = app_clone2.get_webview_window("main") {
                            let individual_compression_result: BatchVideoIndividualCompressionResult =
                                BatchVideoIndividualCompressionResult {
                                    batch_id: batch_id_clone2,
                                    result: result,
                                };
                            let _ = window.emit(
                                CustomEvents::BatchVideoIndividualCompressionCompletion.as_ref(),
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

        let mut ffmpeg_cmd: Command = self.get_ffmpeg_command()?;

        let command = ffmpeg_cmd.args([
            "-ss",
            timestamp_value,
            "-i",
            video_path,
            "-vf",
            "scale=trunc(iw*sar/2)*2:ih,setsar=1",
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

    /// Extracts a subtitle stream from a video file to a separate subtitle file
    pub async fn extract_subtitle(
        &mut self,
        video_path: &str,
        stream_index: u32,
        output_path: &str,
        output_format: &str,
    ) -> Result<String, String> {
        if !Path::exists(Path::new(video_path)) {
            return Err(String::from("File does not exist in given path."));
        }

        let output_path_buf = PathBuf::from(output_path);

        if let Some(parent_dir) = output_path_buf.parent() {
            if !Path::exists(parent_dir) {
                return Err(String::from("Target directory does not exist."));
            }
        }

        let mut ffprobe = FFPROBE::new(&self.app)?;
        let subtitle_streams = ffprobe.get_subtitle_streams(video_path).await?;

        let target_stream = match subtitle_streams.iter().find(|s| s.index == stream_index) {
            Some(stream) => stream,
            None => {
                let available_indices: Vec<u32> =
                    subtitle_streams.iter().map(|s| s.index).collect();
                return Err(format!(
                    "Subtitle stream with global index {} not found. Available subtitle stream indices: {:?}",
                    stream_index, available_indices
                ));
            }
        };

        let codec = &target_stream.codec;

        let subtitle_specific_index = subtitle_streams
            .iter()
            .position(|s| s.index == stream_index)
            .unwrap_or(0);

        let ffmpeg_codec = match output_format {
            "vtt" => "webvtt",
            _ => output_format,
        };

        if matches!(
            codec.as_str(),
            "hdmv_pgs_subtitle" | "dvd_subtitle" | "xsub"
        ) {
            return Err(format!(
                "Cannot extract subtitle: Codec '{}' cannot be converted to {}. This is an image-based subtitle format (e.g., Blu-ray PGS or DVD VobSub).",
                codec, output_format.to_uppercase()
            ));
        }

        let mut ffmpeg_cmd: Command = self.get_ffmpeg_command()?;

        let command = ffmpeg_cmd
            .args(["-i", video_path])
            .args(["-map", &format!("0:s:{}", subtitle_specific_index)])
            .args(["-c:s", ffmpeg_codec])
            .arg(&output_path_buf)
            .arg("-y")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match SharedChild::spawn(command) {
            Ok(child) => {
                let cp = Arc::new(child);

                match cp.wait() {
                    Ok(exit_status) => {
                        if exit_status.success() {
                            if Path::exists(&output_path_buf) {
                                Ok(output_path.to_string())
                            } else {
                                Err(String::from(
                                    "Failed to extract subtitle: Output file was not created.",
                                ))
                            }
                        } else {
                            Err(format!("Failed to extract subtitle (exit code {}). The subtitle may be in an unsupported format.", exit_status))
                        }
                    }
                    Err(err) => Err(format!("Failed to extract subtitle: {}", err)),
                }
            }
            Err(err) => Err(format!("Failed to extract subtitle: {}", err)),
        }
    }

    pub fn convert_video_to_gif(
        &mut self,
        video_path: &str,
        quality: u8,
        video_id: &str,
        dimensions: Option<(u32, u32)>,
        fps: Option<&str>,
    ) -> Result<PathBuf, String> {
        // TODO: Implement CANCEL event and progress support.
        let output_filename = format!("{}.gif", video_id);
        let output_path: PathBuf = [self.assets_dir.clone(), PathBuf::from(&output_filename)]
            .iter()
            .collect();

        let output_path_str = output_path
            .to_str()
            .ok_or("Invalid output path")?
            .to_string();

        let ffmpeg_args = vec!["-i", video_path, "-f", "yuv4mpegpipe", "-"];

        let mut ffmpeg_cmd: Command = self.get_ffmpeg_command()?;

        let mut ffmpeg_child = ffmpeg_cmd
            .args(&ffmpeg_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

        let ffmpeg_stdout = ffmpeg_child
            .stdout
            .take()
            .ok_or("Failed to get ffmpeg stdout")?;

        let gifski_quality = quality.clamp(1, 100);
        let gifski_quality_str = gifski_quality.to_string();
        let mut gifski_args: Vec<String> = vec!["-Q".to_string(), gifski_quality_str.clone()];

        if let Some(fps_val) = fps {
            gifski_args.push("-r".to_string());
            gifski_args.push(fps_val.to_string());
        }

        if let Some((width, height)) = dimensions {
            gifski_args.push("-W".to_string());
            gifski_args.push(width.to_string());
            gifski_args.push("-H".to_string());
            gifski_args.push(height.to_string());
        }

        gifski_args.push("-o".to_string());
        gifski_args.push(output_path_str.clone());
        gifski_args.push("-".to_string());

        let gifski_args_refs: Vec<&str> = gifski_args.iter().map(|s| s.as_str()).collect();

        log::info!(
            "[image] video to gif final command: {:?} | {:?}",
            ffmpeg_args,
            gifski_args_refs
        );

        let mut gifski_cmd = match self.app.shell().sidecar("compresso_gifski") {
            Ok(command) => Command::from(command),
            Err(err) => return Err(format!("[gifski-sidecar]: {:?}", err.to_string())),
        };

        let mut gifski_child = gifski_cmd
            .args(&gifski_args_refs)
            .stdin(ffmpeg_stdout)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to run gifski: {}", e))?;

        let ffmpeg_result = ffmpeg_child.wait();
        let gifski_result = gifski_child.wait();

        match (ffmpeg_result, gifski_result) {
            (Ok(ffmpeg_status), Ok(gifski_status)) => {
                if ffmpeg_status.success() && gifski_status.success() {
                    Ok(output_path)
                } else {
                    Err(format!(
                        "Video to GIF conversion failed - ffmpeg: {:?}, gifski: {:?}",
                        ffmpeg_status, gifski_status
                    ))
                }
            }
            (Err(ffmpeg_err), _) => Err(format!("ffmpeg error: {}", ffmpeg_err)),
            (_, Err(gifski_err)) => Err(format!("gifski error: {}", gifski_err)),
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
