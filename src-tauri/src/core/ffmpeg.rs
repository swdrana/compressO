use crate::core::domain::{
    AudioConfig, BatchCompressionResult, BatchVideoCompressionProgress,
    BatchVideoIndividualCompressionResult, CustomEvents, MediaMetadataConfig, MediaTransform,
    MediaTransformCrop, MediaTransformHistory, SubtitleStream, SubtitlesConfig,
    VideoCompressionConfig, VideoCompressionProgress, VideoCompressionResult, VideoThumbnail,
    VideoTrimSegment,
};
use crate::core::ffprobe::FFPROBE;
use crate::core::image::ImageCompressor;
use crate::core::media_process::{CancelCallback, MediaProcessExecutorBuilder};
use crate::sys::fs::{ensure_assets_dir, get_file_metadata};
use crate::utils;
use nanoid::nanoid;
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::Command;

pub struct FFMPEG {
    app: AppHandle,
    assets_dir: PathBuf,
}

const EXTENSIONS: [&str; 6] = ["mp4", "mov", "webm", "avi", "mkv", "gif"];

impl FFMPEG {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let assets_dir = ensure_assets_dir(app)?;

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

    fn preferred_gpu_h264_encoders() -> &'static [&'static str] {
        if cfg!(target_os = "macos") {
            &["h264_videotoolbox"]
        } else if cfg!(any(target_os = "windows", target_os = "linux")) {
            &["h264_nvenc", "h264_amf", "h264_qsv"]
        } else {
            &[]
        }
    }

    fn preferred_gpu_hevc_encoders() -> &'static [&'static str] {
        if cfg!(target_os = "macos") {
            &["hevc_videotoolbox"]
        } else if cfg!(any(target_os = "windows", target_os = "linux")) {
            &["hevc_nvenc", "hevc_amf", "hevc_qsv"]
        } else {
            &[]
        }
    }

    fn is_supported_h264_encoder(codec: &str) -> bool {
        matches!(
            codec,
            "libx264"
                | "h264_videotoolbox"
                | "h264_nvenc"
                | "h264_amf"
                | "h264_qsv"
        )
    }

    fn is_supported_hevc_encoder(codec: &str) -> bool {
        matches!(
            codec,
            "libx265"
                | "hevc_videotoolbox"
                | "hevc_nvenc"
                | "hevc_amf"
                | "hevc_qsv"
        )
    }

    fn map_quality_inverse(quality: u16, min: u16, max: u16, default_value: u16) -> u16 {
        if (0..=100).contains(&quality) {
            let range = max.saturating_sub(min);
            min + (range - (range * quality) / 100)
        } else {
            default_value
        }
    }

    fn map_quality_linear(quality: u16, min: u16, max: u16, default_value: u16) -> u16 {
        if (0..=100).contains(&quality) {
            let range = max.saturating_sub(min);
            min + (range * quality) / 100
        } else {
            default_value
        }
    }

    async fn probe_encoder(&self, encoder: &str) -> bool {
        let ffmpeg_cmd = match self.get_ffmpeg_command() {
            Ok(cmd) => cmd,
            Err(err) => {
                log::warn!("[ffmpeg] failed to create ffmpeg command: {}", err);
                return false;
            }
        };

        match ffmpeg_cmd
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=128x128:d=0.5",
                "-c:v",
                encoder,
                "-f",
                "null",
                "-",
            ])
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(err) => {
                log::warn!("[ffmpeg] encoder probe failed for {}: {}", encoder, err);
                false
            }
        }
    }

    async fn select_h264_encoder(&self, requested_codec: Option<&str>) -> String {
        if let Some(codec) = requested_codec {
            if Self::is_supported_h264_encoder(codec) {
                if codec == "libx264" || self.probe_encoder(codec).await {
                    return codec.to_string();
                }
                log::warn!(
                    "[ffmpeg] requested encoder {} is unavailable, falling back",
                    codec
                );
                return "libx264".to_string();
            }
            log::warn!(
                "[ffmpeg] unsupported requested video codec {}. forcing H.264 path",
                codec
            );
        }

        for encoder in Self::preferred_gpu_h264_encoders() {
            if self.probe_encoder(encoder).await {
                return (*encoder).to_string();
            }
        }

        "libx264".to_string()
    }

    async fn select_hevc_encoder(&self, requested_codec: Option<&str>) -> String {
        if let Some(codec) = requested_codec {
            if Self::is_supported_hevc_encoder(codec) {
                if codec == "libx265" || self.probe_encoder(codec).await {
                    return codec.to_string();
                }
                log::warn!(
                    "[ffmpeg] requested encoder {} is unavailable, falling back",
                    codec
                );
                return "libx265".to_string();
            }
        }

        for encoder in Self::preferred_gpu_hevc_encoders() {
            if self.probe_encoder(encoder).await {
                return (*encoder).to_string();
            }
        }

        "libx265".to_string()
    }

    fn build_video_encoder_args(
        &self,
        encoder: &str,
        quality: u16,
        preset_name: Option<&str>,
        is_gif_target: bool,
    ) -> Vec<String> {
        let mut args = vec!["-c:v:0".to_string(), encoder.to_string()];

        match encoder {
            "libx264" => {
                let crf = Self::map_quality_inverse(quality, 24, 36, 28);
                args.extend([
                    "-preset".to_string(),
                    "slow".to_string(),
                    "-crf".to_string(),
                    crf.to_string(),
                ]);
            }
            "h264_videotoolbox" => {
                let qv = Self::map_quality_linear(quality, 40, 85, 60);
                args.extend([
                    "-q:v".to_string(),
                    qv.to_string(),
                    "-profile:v".to_string(),
                    "high".to_string(),
                ]);
            }
            "h264_nvenc" => {
                let cq = Self::map_quality_inverse(quality, 18, 34, 24);
                args.extend([
                    "-preset".to_string(),
                    "p6".to_string(),
                    "-tune".to_string(),
                    "hq".to_string(),
                    "-rc".to_string(),
                    "vbr".to_string(),
                    "-cq".to_string(),
                    cq.to_string(),
                ]);
            }
            "h264_qsv" => {
                let global_quality = Self::map_quality_inverse(quality, 18, 34, 24);
                args.extend([
                    "-preset".to_string(),
                    "slow".to_string(),
                    "-global_quality".to_string(),
                    global_quality.to_string(),
                ]);
            }
            "h264_amf" | "hevc_amf" => {
                args.extend([
                    "-quality".to_string(),
                    "quality".to_string(),
                    "-usage".to_string(),
                    "transcoding".to_string(),
                    "-rc".to_string(),
                    "cqp".to_string(),
                ]);
            }
            "hevc_videotoolbox" => {
                let qv = Self::map_quality_linear(quality, 40, 85, 60);
                args.extend([
                    "-q:v".to_string(),
                    qv.to_string(),
                    "-profile:v".to_string(),
                    "main".to_string(),
                ]);
            }
            "hevc_nvenc" => {
                let cq = Self::map_quality_inverse(quality, 18, 34, 24);
                args.extend([
                    "-preset".to_string(),
                    "p6".to_string(),
                    "-tune".to_string(),
                    "hq".to_string(),
                    "-rc".to_string(),
                    "vbr".to_string(),
                    "-cq".to_string(),
                    cq.to_string(),
                ]);
            }
            "hevc_qsv" => {
                let global_quality = Self::map_quality_inverse(quality, 18, 34, 24);
                args.extend([
                    "-preset".to_string(),
                    "slow".to_string(),
                    "-global_quality".to_string(),
                    global_quality.to_string(),
                ]);
            }
            "libvpx-vp9" => {
                let crf = if preset_name.is_some() && !is_gif_target {
                    Self::map_quality_inverse(quality, 24, 36, 28)
                } else {
                    18
                };
                args.extend(["-crf".to_string(), crf.to_string()]);
            }
            _ => {
                let crf = Self::map_quality_inverse(quality, 24, 36, 28);
                args.extend(["-crf".to_string(), crf.to_string()]);
            }
        }

        args
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
        dimensions: Option<(f64, f64)>,
        fps: Option<&str>,
        video_codec: Option<&str>,
        transform_history: Option<&MediaTransformHistory>,
        strip_metadata: Option<bool>,
        metadata_config: Option<&MediaMetadataConfig>,
        custom_thumbnail_path: Option<&str>,
        trim_segments: Option<&Vec<VideoTrimSegment>>,
        subtitles_config: Option<&SubtitlesConfig>,
        speed: Option<f32>,
    ) -> Result<VideoCompressionResult, String> {
        if !EXTENSIONS.contains(&convert_to_extension) {
            return Err(String::from("Invalid convert to extension."));
        }

        let (audio_streams, video_duration_opt) = {
            let mut ffprobe = FFPROBE::new(&self.app)?;
            let audio = ffprobe.get_audio_streams(video_path).await?;
            let basic_info = ffprobe.get_video_basic_info(video_path).await?;
            (audio, basic_info.duration)
        };
        let video_duration = video_duration_opt; // Keep it as Option<f64>
        let has_audio_stream = !audio_streams.is_empty();

        let (existing_subtitle_count, existing_subtitle_streams): (usize, Vec<SubtitleStream>) = {
            if let Some(subs_config) = subtitles_config {
                let preserve = subs_config.preserve_existing_subtitles.unwrap_or(false);
                if preserve {
                    let mut ffprobe = FFPROBE::new(&self.app)?;
                    let streams = ffprobe.get_subtitle_streams(video_path).await?;
                    let count = streams.len();
                    (count, streams)
                } else {
                    (0, Vec::new())
                }
            } else {
                (0, Vec::new())
            }
        };
        let has_existing_subtitles = existing_subtitle_count > 0;

        let batch_id = match batch_id {
            Some(id) => String::from(id),
            None => nanoid!(),
        };

        let is_gif_target = convert_to_extension == "gif";

        let file_name = if is_gif_target {
            format!("{}.mp4", video_id)
        } else {
            format!("{}.{}", video_id, convert_to_extension)
        };

        let output_file: PathBuf = [self.assets_dir.clone(), PathBuf::from(&file_name)]
            .iter()
            .collect();

        let mut cmd_args: Vec<&str> = Vec::new();

        // Hardware Acceleration for decoding
        if cfg!(target_os = "macos") {
            cmd_args.extend_from_slice(&["-hwaccel", "videotoolbox"]);
        } else if cfg!(target_os = "windows") {
            cmd_args.extend_from_slice(&["-hwaccel", "auto"]);
        }

        cmd_args.push("-i");
        cmd_args.push(video_path);

        // Track input indices for mapping
        let mut input_index: usize = 1; // 0 is video, 1+ are thumbnails/subtitles

        if !is_gif_target && convert_to_extension != "webm" {
            if let Some(thumb_path) = custom_thumbnail_path {
                if thumb_path.len() > 0 {
                    cmd_args.extend_from_slice(&["-i", thumb_path]);
                    input_index += 1;
                }
            }
        }

        let subtitle_input_indices: Vec<(usize, String, Option<String>)> = if !is_gif_target {
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
                                let title = sub.title.as_ref().map(|t| t.trim().to_string());
                                indices.push((input_index, lang, title));
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

        let should_strip_metadata = strip_metadata.unwrap_or(false);
        if should_strip_metadata {
            cmd_args.extend_from_slice(&["-map_metadata", "-1"]);
        } else {
            cmd_args.extend_from_slice(&["-map_metadata", "0"]);
        }

        cmd_args.extend_from_slice(&[
            "-hide_banner",
            "-progress",
            "-",
            "-nostats",
            "-loglevel",
            "error",
        ]);

        let mut cmd_args = if !is_gif_target {
            match preset_name {
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
                        ]);
                        cmd_args
                    }
                },
                None => cmd_args,
            }
        } else {
            cmd_args
        };

        let selected_video_encoder = if convert_to_extension == "webm" {
            "libvpx-vp9".to_string()
        } else if let Some(codec) = video_codec {
            if codec == "libx265" || codec.contains("hevc") {
                self.select_hevc_encoder(video_codec).await
            } else {
                self.select_h264_encoder(video_codec).await
            }
        } else {
            self.select_h264_encoder(video_codec).await
        };

        // Build the post-processing chain for video (transforms + scale + pad)
        let video_post_process = build_ffmpeg_filters(transform_history, dimensions);

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

        let clamped_speed = speed.map(|s| s.clamp(0.25, 4.0));
        let video_speed_filter = if let Some(speed_value) = clamped_speed {
            if speed_value != 1.0 {
                let pts_multiplier = 1.0 / speed_value;
                Some(format!("setpts={}*PTS", pts_multiplier))
            } else {
                None
            }
        } else {
            None
        };

        fn build_audio_speed_filter(speed: f32) -> String {
            if speed == 1.0 {
                return String::new();
            }
            if speed >= 0.5 && speed <= 2.0 {
                return format!("atempo={}", speed);
            }
            if speed < 0.5 {
                // For speeds < 0.5, chain multiple atempo=0.5 filters
                let mut filters = Vec::new();
                let mut remaining = speed;
                while remaining < 0.5 {
                    filters.push("atempo=0.5".to_string());
                    remaining /= 0.5;
                }
                if remaining > 0.0 && remaining != 1.0 {
                    filters.push(format!("atempo={}", remaining));
                }
                filters.join(",")
            } else {
                // For speeds > 2.0, chain multiple atempo=2.0 filters
                let mut filters = Vec::new();
                let mut remaining = speed;
                while remaining > 2.0 {
                    filters.push("atempo=2.0".to_string());
                    remaining /= 2.0;
                }
                if remaining > 0.0 && remaining != 1.0 {
                    filters.push(format!("atempo={}", remaining));
                }
                filters.join(",")
            }
        }

        let audio_speed_filter = if let Some(speed_value) = clamped_speed {
            build_audio_speed_filter(speed_value)
        } else {
            String::new()
        };

        if let Some(segments) = trim_segments {
            if !segments.is_empty() {
                map_video = true;
                if segments.len() == 1 {
                    let seg = &segments[0];
                    // Single trim: trim -> speed -> post_process -> [outv]
                    let speed_part = video_speed_filter
                        .as_ref()
                        .map(|f| format!("{},", f))
                        .unwrap_or_default();
                    filter_complex_parts.push(format!(
                        "[0:v]trim={}:{},setpts=PTS-STARTPTS,{}{}[outv]",
                        seg.start, seg.end, speed_part, video_post_process
                    ));
                } else {
                    // Multi trim: trim segments -> concat -> speed -> post process -> [outv]
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

                    let speed_part = video_speed_filter
                        .as_ref()
                        .map(|f| format!("{},", f))
                        .unwrap_or_default();
                    filter_complex_parts.push(format!(
                        "{} concat=n={}:v=1:a=0,{}{}[outv]",
                        video_labels.join(""),
                        segments.len(),
                        speed_part,
                        video_post_process
                    ));
                }
            }
        }

        // If no trimming, just apply post-processing to input
        if !map_video {
            let speed_part = video_speed_filter
                .as_ref()
                .map(|f| format!("{},", f))
                .unwrap_or_default();
            filter_complex_parts.push(format!("[0:v]{}{}[outv]", speed_part, video_post_process));
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

                        // Combine audio filters: volume/channel + speed
                        let audio_filters_with_speed = if !combined_audio_filter.is_empty()
                            && !audio_speed_filter.is_empty()
                        {
                            format!("{},{}", combined_audio_filter, audio_speed_filter)
                        } else if !combined_audio_filter.is_empty() {
                            combined_audio_filter.clone()
                        } else if !audio_speed_filter.is_empty() {
                            audio_speed_filter.clone()
                        } else {
                            String::new()
                        };

                        let audio_filters_with_speed_comma = if !audio_filters_with_speed.is_empty()
                        {
                            format!(",{}", audio_filters_with_speed)
                        } else {
                            "".to_string()
                        };

                        if segments.len() == 1 {
                            let seg = &segments[0];
                            filter_complex_parts.push(format!(
                                "[0:a:{}]atrim={}:{},asetpts=PTS-STARTPTS{}[{}]",
                                track_index,
                                seg.start,
                                seg.end,
                                audio_filters_with_speed_comma,
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
                                audio_filters_with_speed_comma,
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
                    || !audio_speed_filter.is_empty()
                    || (audio_config.volume > 0 && audio_config.volume != 100))
            {
                let mut args = vec![];
                let audio_filters_with_speed =
                    if !combined_audio_filter.is_empty() && !audio_speed_filter.is_empty() {
                        format!("{},{}", combined_audio_filter, audio_speed_filter)
                    } else if !combined_audio_filter.is_empty() {
                        combined_audio_filter.clone()
                    } else if !audio_speed_filter.is_empty() {
                        audio_speed_filter.clone()
                    } else {
                        String::new()
                    };

                if let Some(ref selected_tracks) = audio_config.selected_audio_tracks {
                    for &track_index in selected_tracks {
                        args.push(format!("-filter:a:{}", track_index));
                        args.push(audio_filters_with_speed.clone());
                    }
                } else {
                    for track_index in 0..audio_streams.len() {
                        args.push(format!("-filter:a:{}", track_index));
                        args.push(audio_filters_with_speed.clone());
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

        if audio_config.volume == 0 || is_gif_target {
            cmd_args.push("-an");
        }

        let mut metadata_args: Vec<String> = Vec::new();

        if !is_gif_target && !should_strip_metadata {
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

        if !is_gif_target {
            if has_existing_subtitles {
                let bitmap_codecs = ["hdmv_pgs_subtitle", "dvd_subtitle", "xsub"];

                for (idx, stream) in existing_subtitle_streams.iter().enumerate() {
                    let is_bitmap = bitmap_codecs.contains(&stream.codec.as_str());
                    let output_container = convert_to_extension;

                    if is_bitmap && output_container != "mkv" {
                        log::warn!(
                            "[ffmpeg] Skipping bitmap subtitle stream {} (codec: {}) - not compatible with {} container",
                            idx,
                            stream.codec,
                            output_container
                        );
                        continue;
                    }

                    if output_container == "avi" {
                        log::warn!("[ffmpeg] Skipping subtitle stream {} (codec: {}) - AVI container has limited subtitle support", idx, stream.codec);
                        continue;
                    }

                    subtitle_args_owned.push("-map".to_string());
                    subtitle_args_owned.push(format!("0:s:{}", idx));

                    let subtitle_codec = match output_container {
                        "mkv" if is_bitmap => "copy",
                        "mkv" => "srt",
                        "webm" => "webvtt",
                        _ => "mov_text",
                    };

                    subtitle_args_owned.push(format!("-c:s:{}", subtitle_index));
                    subtitle_args_owned.push(subtitle_codec.to_string());

                    if let Some(ref lang) = stream.language {
                        subtitle_args_owned.push(format!("-metadata:s:s:{}", subtitle_index));
                        subtitle_args_owned.push(format!("language={}", lang));
                    }

                    if let Some(ref title) = stream.title {
                        subtitle_args_owned.push(format!("-metadata:s:s:{}", subtitle_index));
                        subtitle_args_owned.push(format!("title={}", title.trim()));
                    }

                    subtitle_index += 1;
                }
            }

            for (sub_input_idx, language, title) in subtitle_input_indices.iter() {
                if convert_to_extension == "avi" {
                    log::warn!("[ffmpeg] Skipping external subtitle file - AVI container has limited subtitle support");
                    continue;
                }

                subtitle_args_owned.push("-map".to_string());
                subtitle_args_owned.push(format!("{}:s", sub_input_idx));

                let subtitle_codec = match convert_to_extension {
                    "mkv" => "copy",
                    "webm" => "webvtt",
                    _ => "mov_text",
                };
                subtitle_args_owned.push(format!("-c:s:{}", subtitle_index));
                subtitle_args_owned.push(subtitle_codec.to_string());

                if !language.is_empty() {
                    subtitle_args_owned.push(format!("-metadata:s:s:{}", subtitle_index));
                    subtitle_args_owned.push(format!("language={}", language));
                }

                if let Some(ref t) = title {
                    subtitle_args_owned.push(format!("-metadata:s:s:{}", subtitle_index));
                    subtitle_args_owned.push(format!("title={}", t));
                }

                subtitle_index += 1;
            }
        }
        cmd_args.extend(subtitle_args_owned.iter().map(|s| s.as_str()));

        if !is_gif_target && custom_thumbnail_path.is_some() && convert_to_extension != "webm" {
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

        let output_path = output_file.display().to_string();
        let common_args_owned: Vec<String> =
            cmd_args.iter().map(|arg| arg.to_string()).collect();

        let output_file_clone = output_file.clone();
        let cancel_callback: CancelCallback = Arc::new(move || {
            std::fs::remove_file(&output_file_clone).ok();
            log::info!("Cleaned up partial output file: {:?}", output_file_clone);
        });


        let mut encoders_to_try = vec![selected_video_encoder.clone()];
        if convert_to_extension != "webm" {
            if selected_video_encoder.contains("hevc") && selected_video_encoder != "libx265" {
                encoders_to_try.push("libx265".to_string());
            } else if selected_video_encoder != "libx264" && !selected_video_encoder.contains("hevc")
            {
                encoders_to_try.push("libx264".to_string());
            }
        }

        let mut compressed = false;
        let start_time = std::time::Instant::now();

        for (attempt_index, encoder_name) in encoders_to_try.iter().enumerate() {
            let encoder_name_for_cb = encoder_name.clone();
            let app_clone = self.app.clone();
            let video_id_for_progress = video_id.to_string();
            let batch_id_for_progress = batch_id.clone();
            
            let re_time = Regex::new(r"out_time=([^\n\s\r]+)").unwrap();
            let re_fps = Regex::new(r"fps=([^\n\s\r]+)").unwrap();
            let re_speed = Regex::new(r"speed=([^\n\s\r]+)").unwrap();

            let total_duration_secs = video_duration.unwrap_or(0.0);
            println!("\n----------------------");
            println!("Processing attempt: {}", attempt_index + 1);
            println!("Encoder: {}", encoder_name);
            if encoder_name.contains("videotoolbox") {
                println!("GPU ACCELERATION: ACTIVE (VideoToolbox)");
            } else {
                println!("GPU ACCELERATION: INACTIVE (Software Fallback)");
            }
            println!("----------------------\n");
            
            let stdout_callback = Arc::new(move |_process_index: usize, stdout_line: String| {
                let mut current_time_str = String::new();
                let mut current_fps = 0.0f32;
                let mut current_speed_str = String::from("0x");

                if let Some(cap) = re_time.captures(&stdout_line) {
                    current_time_str = cap.get(1).map_or("", |m| m.as_str()).to_string();
                }
                if let Some(cap) = re_fps.captures(&stdout_line) {
                    current_fps = cap.get(1).map_or("0.0", |m| m.as_str()).trim().parse().unwrap_or(0.0);
                }
                if let Some(cap) = re_speed.captures(&stdout_line) {
                    current_speed_str = cap.get(1).map_or("0x", |m| m.as_str()).to_string();
                }

                if !current_time_str.is_empty() && (current_time_str.contains(':') || current_time_str.parse::<f64>().is_ok()) {
                    let elapsed_playback_secs = utils::duration::convert_duration_to_seconds(&current_time_str);
                    let percentage = if total_duration_secs > 0.0 {
                        (elapsed_playback_secs / total_duration_secs * 100.0).min(100.0)
                    } else {
                        0.0
                    };

                    let speed_val: f32 = current_speed_str.trim_end_matches('x').trim().parse().unwrap_or(0.0);
                    let time_remaining = if speed_val > 0.01 && total_duration_secs > elapsed_playback_secs {
                        let remaining_secs = (total_duration_secs - elapsed_playback_secs) / (speed_val as f64);
                        utils::duration::format_seconds_to_duration(remaining_secs as u64)
                    } else {
                        "Calculating...".to_string()
                    };

                    let wall_clock_elapsed = start_time.elapsed().as_secs();
                    let time_elapsed = utils::duration::format_seconds_to_duration(wall_clock_elapsed);

                    let video_progress = VideoCompressionProgress {
                        video_id: video_id_for_progress.clone(),
                        batch_id: batch_id_for_progress.clone(),
                        current_duration: current_time_str,
                        encoder: encoder_name_for_cb.clone(),
                        speed: current_speed_str,
                        fps: current_fps,
                        percentage: percentage as f32,
                        time_remaining,
                        time_elapsed,
                    };

                    if let Some(window) = app_clone.get_webview_window("main") {
                        window.emit(CustomEvents::VideoCompressionProgress.as_ref(), video_progress).ok();
                    }
                }
            });

            let mut final_args_owned = common_args_owned.clone();
            final_args_owned.extend(self.build_video_encoder_args(
                &encoder_name,
                quality,
                preset_name,
                is_gif_target,
            ));
            final_args_owned.extend(["-y".to_string(), output_path.clone()]);

            log::info!(
                "[ffmpeg] compress attempt {} using encoder {}",
                attempt_index + 1,
                encoder_name
            );
            let ffmpeg_cmd = self.get_ffmpeg_command()?;
            let ffmpeg_cmd = ffmpeg_cmd.args(final_args_owned.iter().map(|arg| arg.as_str()));

            println!("--- [FFMPEG START] ---");
            println!("Encoder: {}", encoder_name);
            println!("Hardware Acceleration: VideoToolbox enabled");
            println!("Command: {:?}", final_args_owned);
            println!("----------------------");

            let executor = MediaProcessExecutorBuilder::new(self.app.clone())
                .command(ffmpeg_cmd.into())
                .with_cancel_support(
                    vec![video_id.to_string(), batch_id.clone()],
                    Some(cancel_callback.clone()),
                )
                .with_stdout_callback(stdout_callback)
                .build()?;

            match executor.spawn_and_wait().await {
                Ok(result) if result.success() => {
                    compressed = true;
                    break;
                }
                Ok(_) => {
                    let _ = std::fs::remove_file(&output_path);
                    if attempt_index + 1 < encoders_to_try.len() {
                        log::warn!(
                            "[ffmpeg] encoder {} failed, retrying with fallback",
                            encoder_name
                        );
                    }
                }
                Err(err) => {
                    if err == "CANCELLED" {
                        return Err(err);
                    }
                    let _ = std::fs::remove_file(&output_path);
                    if attempt_index + 1 < encoders_to_try.len() {
                        log::warn!(
                            "[ffmpeg] encoder {} returned error: {}. retrying with fallback",
                            encoder_name,
                            err
                        );
                    }
                }
            }
        }

        if !compressed {
            return Err("Video compression failed".to_string());
        }

        if convert_to_extension == "gif" {
            log::info!("[ffmpeg] Converting processed video to GIF");

            let temp_video_path = output_file.to_string_lossy().to_string();

            let gif_output_path = self
                .convert_video_to_gif(&temp_video_path, quality as u8, video_id, dimensions, fps)
                .await?;

            std::fs::remove_file(&temp_video_path).ok();
            log::info!(
                "[ffmpeg] Removed temporary video file: {:?}",
                temp_video_path
            );

            let gif_path = gif_output_path.to_string_lossy().to_string();
            let file_metadata = get_file_metadata(&gif_path);
            let gif_file_name = format!("{}.gif", video_id);

            return Ok(VideoCompressionResult {
                video_id: video_id.to_owned(),
                file_name: gif_file_name,
                file_path: gif_path,
                file_metadata: file_metadata.ok(),
            });
        }

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
            let transform_history = video_options.transform_history.as_ref().map(|v| v.as_ref());
            let metadata_config = video_options.metadata_config.as_ref();
            let thumbnail_path = video_options.custom_thumbnail_path.as_deref();
            let trim_segments = video_options.trim_segments.as_ref();
            let subtitles_config = video_options.subtitles_config.as_ref();
            let strip_metadata = video_options.strip_metadata;
            let speed = video_options.speed;

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
                    transform_history,
                    strip_metadata,
                    metadata_config,
                    thumbnail_path,
                    trim_segments,
                    subtitles_config,
                    speed,
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

        let timestamp_str = timestamp.unwrap_or("00:00:01.00");
        let output_path_str = output_path.display().to_string();

        let ffmpeg_cmd = self.get_ffmpeg_command()?;
        let ffmpeg_cmd = ffmpeg_cmd.args([
            "-ss",
            &timestamp_str,
            "-i",
            video_path,
            "-vframes",
            "1",
            "-q:v",
            "2",
            &output_path_str,
            "-y",
        ]);

        let executor = MediaProcessExecutorBuilder::new(self.app.clone())
            .command(ffmpeg_cmd.into())
            .build()?;

        let result = executor.spawn_and_wait().await?;

        if !result.success() {
            return Err("Video is corrupted or thumbnail generation failed".to_string());
        }

        Ok(VideoThumbnail {
            id,
            file_name,
            file_path: output_path_str,
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

        let ffmpeg_cmd = self.get_ffmpeg_command()?;
        let ffmpeg_cmd = ffmpeg_cmd
            .arg("-i")
            .arg(video_path)
            .args(["-map", &format!("0:s:{}", subtitle_specific_index)])
            .args(["-c:s", ffmpeg_codec])
            .arg(&output_path_buf)
            .arg("-y");

        let executor = MediaProcessExecutorBuilder::new(self.app.clone())
            .command(ffmpeg_cmd.into())
            .build()?;

        let result = executor.spawn_and_wait().await?;

        if !result.success() {
            if Path::exists(&output_path_buf) {
                return Err(format!(
                    "Failed to extract subtitle (exit code {:?}). The subtitle may be in an unsupported format.",
                    result.code()
                ));
            } else {
                return Err(String::from(
                    "Failed to extract subtitle: Output file was not created.",
                ));
            }
        }

        Ok(output_path.to_string())
    }

    pub async fn convert_video_to_gif(
        &mut self,
        video_path: &str,
        quality: u8,
        video_id: &str,
        dimensions: Option<(f64, f64)>,
        fps: Option<&str>,
    ) -> Result<PathBuf, String> {
        let output_filename = format!("{}.gif", video_id);
        let output_path: PathBuf = [self.assets_dir.clone(), PathBuf::from(&output_filename)]
            .iter()
            .collect();

        let output_path_str = output_path
            .to_str()
            .ok_or("Invalid output path")?
            .to_string();

        let video_duration_seconds = {
            let mut ffprobe = FFPROBE::new(&self.app)?;
            let video_info = ffprobe.get_video_basic_info(video_path).await?;
            video_info.duration.unwrap_or(0.0)
        };

        let video_duration_offset = if video_duration_seconds > 0.0 {
            let hours = (video_duration_seconds / 3600.0) as u32;
            let minutes = ((video_duration_seconds % 3600.0) / 60.0) as u32;
            let seconds = video_duration_seconds % 60.0;
            format!("{:02}:{:02}:{:05.2}", hours, minutes, seconds)
        } else {
            String::from("00:00:00.00")
        };

        let ffmpeg_cmd = self.get_ffmpeg_command()?;
        let ffmpeg_cmd = ffmpeg_cmd.args([
            "-i",
            video_path,
            "-pix_fmt",
            "yuv420p", // Convert to 8-bit for yuv4mpegpipe compatibility
            "-f",
            "yuv4mpegpipe",
            "-",
        ]);

        let gifski_quality = quality.clamp(1, 100);
        let mut gifski_args: Vec<String> = vec!["-Q".to_string(), gifski_quality.to_string()];

        if let Some(fps_val) = fps {
            gifski_args.extend(["-r".to_string(), fps_val.to_string()]);
        }

        if let Some((width, height)) = dimensions {
            gifski_args.extend([
                "-W".to_string(),
                width.to_string(),
                "-H".to_string(),
                height.to_string(),
            ]);
        }

        gifski_args.extend(["-o".to_string(), output_path_str, "-".to_string()]);

        let gifski_args_refs: Vec<&str> = gifski_args.iter().map(|s| s.as_str()).collect();

        let image_compressor = ImageCompressor::new(&self.app)?;
        let mut gifski_cmd = image_compressor
            .get_gifski_command()
            .map_err(|e| format!("Gifski command error: {}", e))?;
        gifski_cmd.args(&gifski_args_refs);

        log::info!(
            "[ffmpeg] final ffmpeg and gifski commands prepared"
        );

        let cancel_callback = Arc::new(|| {
            log::info!("CANCELLED Video to GIF conversion");
        });

        let app_clone = self.app.clone();
        let video_id_for_progress = video_id.to_string();
        let video_duration_offset_clone = video_duration_offset.clone();
        let time_regex = Regex::new(r"time=(?P<time>[\d:.]+)").unwrap();

        let stderr_callback = Arc::new(move |process_index: usize, stderr_line: String| {
            if process_index == 0 {
                if let Some(cap) = time_regex.captures(&stderr_line) {
                    if let Some(time) = cap.name("time") {
                        let current_duration = time.as_str();
                        if !current_duration.is_empty() {
                            let combined_duration = if let Some(sum) =
                                utils::duration::add_durations(&[
                                    &video_duration_offset_clone,
                                    current_duration,
                                ]) {
                                sum
                            } else {
                                current_duration.to_string()
                            };

                            let video_progress = VideoCompressionProgress {
                                video_id: video_id_for_progress.clone(),
                                batch_id: String::new(),
                                current_duration: combined_duration,
                                encoder: "gifski".to_string(),
                                speed: "1x".to_string(),
                                fps: 0.0,
                                percentage: 0.0,
                                time_remaining: String::from("..."),
                                time_elapsed: String::from("..."),
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
                    }
                }
            }
        });

        let executor = MediaProcessExecutorBuilder::new(self.app.clone())
            .commands(vec![ffmpeg_cmd.into(), gifski_cmd.into()])
            .with_piped()
            .with_cancel_support(vec![video_id.to_string()], Some(cancel_callback))
            .with_stderr_callback(stderr_callback)
            .build()?;

        let result = executor.spawn_and_wait().await?;

        if !result.success() {
            return Err("Video to GIF conversion failed".to_string());
        }

        Ok(output_path)
    }
}

/// Builds complete FFmpeg filter complex for video/image post-processing
/// Combines transforms (crop, rotate, flip) + scale (dimensions) + padding
pub fn build_ffmpeg_filters(
    transform_history: Option<&MediaTransformHistory>,
    dimensions: Option<(f64, f64)>,
) -> String {
    let mut filters: Vec<String> = Vec::new();
    let mut latest_crop: Option<&MediaTransformCrop> = None;

    if let Some(actions) = transform_history {
        for action in actions {
            match action {
                MediaTransform::Rotate { value } => {
                    let angle = *value;
                    match angle % 360 {
                        -90 | 270 => filters.push("transpose=2".to_string()),
                        90 | -270 => filters.push("transpose=1".to_string()),
                        180 | -180 => filters.push("hflip,vflip".to_string()),
                        _ => {}
                    }
                }
                MediaTransform::Flip { value } => {
                    if value.horizontal {
                        filters.push("hflip".to_string());
                    }
                    if value.vertical {
                        filters.push("vflip".to_string());
                    }
                }
                MediaTransform::Crop { value } => {
                    latest_crop = Some(value);
                }
            }
        }
    }

    // Apply only the last crop
    if let Some(c) = latest_crop {
        let w = c.width.round() as i64;
        let h = c.height.round() as i64;
        let x = c.left.round() as i64;
        let y = c.top.round() as i64;

        filters.push(format!("crop={}:{}:{}:{}", w, h, x, y));
    }

    if let Some((width, height)) = dimensions {
        let w = width.round() as i64;
        let h = height.round() as i64;
        filters.push(format!("scale={}:{}:flags=lanczos", w, h));
    }

    filters.push("pad=ceil(iw/2)*2:ceil(ih/2)*2".to_string());

    filters.join(",")
}
