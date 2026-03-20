use crate::domain::{
    AudioStream, Chapter, ContainerInfo, Disposition, SubtitleStream, TauriEvents, VideoInfo,
    VideoStream,
};
use serde_json::Value;
use shared_child::SharedChild;
use std::{
    io::BufRead,
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
};
use strum::EnumProperty;
use tauri::AppHandle;
use tauri::{Listener, Manager};
use tauri_plugin_shell::ShellExt;

pub struct FFPROBE {
    app: AppHandle,
}

impl FFPROBE {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        Ok(Self {
            app: app.to_owned(),
        })
    }

    pub fn get_ffprobe_command(&self) -> Result<Command, String> {
        self.app
            .shell()
            .sidecar("compresso_ffprobe")
            .map(Command::from)
            .map_err(|e| format!("Failed to create ffprobe command: {}", e))
    }

    /// Gets video basic information (duration, dimensions, fps) using ffprobe JSON output
    pub async fn get_video_basic_info(&mut self, video_path: &str) -> Result<VideoInfo, String> {
        if !Path::exists(Path::new(video_path)) {
            return Err(String::from("File does not exist in given path."));
        }

        let mut ffprobe_cmd = self.get_ffprobe_command()?;

        let command = ffprobe_cmd
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,r_frame_rate,avg_frame_rate",
                "-show_entries",
                "format=duration",
                "-of",
                "json",
                video_path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                        Ok(_) => log::info!("[ffprobe-sidecar] child process killed."),
                        Err(err) => log::error!(
                            "[ffprobe-sidecar] child process could not be killed {}",
                            err
                        ),
                    },
                );

                let thread = tokio::task::spawn(async move {
                    let mut duration: Option<f64> = None;
                    let mut dimensions: Option<(u32, u32)> = None;
                    let mut fps: Option<f32> = None;

                    if let Some(stdout) = cp_clone1.take_stdout() {
                        let reader = std::io::BufReader::new(stdout);
                        let mut json_str = String::new();

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                json_str.push_str(&line);
                            } else {
                                break;
                            }
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                            // Parse duration from format (in seconds)
                            if let Some(format) = json.get("format") {
                                if let Some(dur) = format.get("duration").and_then(|d| d.as_str()) {
                                    if let Ok(parsed) = dur.parse::<f64>() {
                                        duration = Some(parsed);
                                    }
                                }
                            }

                            // Parse video stream info
                            if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
                                if let Some(stream) = streams.first() {
                                    // Parse dimensions
                                    if let (Some(w), Some(h)) = (
                                        stream.get("width").and_then(|v| v.as_u64()),
                                        stream.get("height").and_then(|v| v.as_u64()),
                                    ) {
                                        dimensions = Some((w as u32, h as u32));
                                    }

                                    // Parse FPS from r_frame_rate (e.g., "30000/1001" or "30/1")
                                    if fps.is_none() {
                                        if let Some(r_frame_rate) =
                                            stream.get("r_frame_rate").and_then(|v| v.as_str())
                                        {
                                            if let Some((num, den)) = parse_fraction(r_frame_rate) {
                                                fps = Some(num as f32 / den as f32);
                                            }
                                        }

                                        // Fallback to avg_frame_rate
                                        if fps.is_none() {
                                            if let Some(avg_frame_rate) = stream
                                                .get("avg_frame_rate")
                                                .and_then(|v| v.as_str())
                                            {
                                                if let Some((num, den)) =
                                                    parse_fraction(avg_frame_rate)
                                                {
                                                    if den > 0 {
                                                        fps = Some(num as f32 / den as f32);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
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
                    log::error!(
                        "[ffprobe-sidecar] child process could not be killed {}",
                        err
                    );
                }

                result
            }
            Err(err) => Err(err.to_string()),
        }
    }

    // Get all video streams from the source video
    pub async fn get_video_streams(&mut self, path: &str) -> Result<Vec<VideoStream>, String> {
        let mut ffprobe_cmd = self.get_ffprobe_command()?;

        let command = ffprobe_cmd
            .args([
                "-v",
                "error",
                "-select_streams",
                "v",
                "-show_entries",
                "stream=codec_name,codec_long_name,profile,codec_type,width,height,coded_width,coded_height,r_frame_rate,avg_frame_rate,pix_fmt,color_space,color_range,color_primaries,color_transfer,chroma_location,bit_rate,duration,nb_frames,refs,field_order,time_base,tags,gop_size,level",
                "-of",
                "json",
                path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                        Ok(_) => log::info!("[ffprobe-sidecar] child process killed."),
                        Err(err) => log::error!(
                            "[ffprobe-sidecar] child process could not be killed {}",
                            err
                        ),
                    },
                );

                let thread = tokio::task::spawn(async move {
                    let streams = if let Some(stdout) = cp_clone1.take_stdout() {
                        let reader = std::io::BufReader::new(stdout);
                        let mut json_str = String::new();

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                json_str.push_str(&line);
                            } else {
                                break;
                            }
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(streams_array) =
                                json.get("streams").and_then(|s| s.as_array())
                            {
                                let mut result = Vec::new();
                                for stream in streams_array {
                                    // Basic codec info
                                    let codec = stream
                                        .get("codec_name")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let codec_long_name = stream
                                        .get("codec_long_name")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let profile = stream
                                        .get("profile")
                                        .and_then(|p| p.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let codec_type = stream
                                        .get("codec_type")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("video")
                                        .to_string();

                                    // Dimensions
                                    let width =
                                        stream.get("width").and_then(|w| w.as_u64()).unwrap_or(0)
                                            as u32;

                                    let height =
                                        stream.get("height").and_then(|h| h.as_u64()).unwrap_or(0)
                                            as u32;

                                    let coded_width = stream
                                        .get("coded_width")
                                        .and_then(|w| w.as_u64())
                                        .unwrap_or(0)
                                        as u32;

                                    let coded_height = stream
                                        .get("coded_height")
                                        .and_then(|h| h.as_u64())
                                        .unwrap_or(0)
                                        as u32;

                                    // Frame rate
                                    let r_frame_rate = stream
                                        .get("r_frame_rate")
                                        .and_then(|r| r.as_str())
                                        .unwrap_or("0/0")
                                        .to_string();

                                    let avg_frame_rate = stream
                                        .get("avg_frame_rate")
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("0/0")
                                        .to_string();

                                    // Pixel format and color
                                    let pix_fmt = stream
                                        .get("pix_fmt")
                                        .and_then(|p| p.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let color_space = stream
                                        .get("color_space")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());

                                    let color_range = stream
                                        .get("color_range")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());

                                    let color_primaries = stream
                                        .get("color_primaries")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());

                                    let color_transfer = stream
                                        .get("color_transfer")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());

                                    let chroma_location = stream
                                        .get("chroma_location")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());

                                    // Bitrate and duration
                                    let bit_rate = stream
                                        .get("bit_rate")
                                        .and_then(|b| b.as_str())
                                        .map(|s| s.to_string());

                                    let duration = stream
                                        .get("duration")
                                        .and_then(|d| d.as_str())
                                        .map(|s| s.to_string());

                                    // Frame info
                                    let nb_frames = stream
                                        .get("nb_frames")
                                        .and_then(|n| n.as_str())
                                        .map(|s| s.to_string());

                                    let refs = stream
                                        .get("refs")
                                        .and_then(|r| r.as_u64())
                                        .and_then(|r| u32::try_from(r).ok());

                                    // Bitstream/codec-specific info
                                    let gop_size = stream
                                        .get("gop_size")
                                        .and_then(|g| g.as_u64())
                                        .and_then(|g| u32::try_from(g).ok());

                                    let level = stream
                                        .get("level")
                                        .and_then(|l| l.as_i64())
                                        .and_then(|l| u32::try_from(l).ok());

                                    // Other
                                    let field_order = stream
                                        .get("field_order")
                                        .and_then(|f| f.as_str())
                                        .unwrap_or("progressive")
                                        .to_string();

                                    let time_base = stream
                                        .get("time_base")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("0/0")
                                        .to_string();

                                    // Rotation metadata (from tags)
                                    let rotation = if let Some(tags_obj) = stream.get("tags") {
                                        tags_obj
                                            .get("rotate")
                                            .and_then(|r| r.as_str())
                                            .and_then(|r| r.parse::<f64>().ok())
                                    } else {
                                        None
                                    };

                                    result.push(VideoStream {
                                        codec,
                                        codec_long_name,
                                        profile,
                                        codec_type,
                                        width,
                                        height,
                                        coded_width,
                                        coded_height,
                                        r_frame_rate,
                                        avg_frame_rate,
                                        pix_fmt,
                                        color_space,
                                        color_range,
                                        color_primaries,
                                        color_transfer,
                                        chroma_location,
                                        bit_rate,
                                        duration,
                                        nb_frames,
                                        refs,
                                        gop_size,
                                        level,
                                        field_order,
                                        time_base,
                                        rotation,
                                    });
                                }
                                result
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    if cp_clone1.wait().is_ok() {
                        (0, streams)
                    } else {
                        (1, Vec::new())
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, streams)) => {
                        if exit_status == 1 {
                            Err("Failed to get video streams".to_string())
                        } else {
                            Ok(streams)
                        }
                    }
                    Err(err) => Err(err.to_string()),
                };

                // Cleanup
                window.unlisten(destroy_event_id);
                if let Err(err) = cp_clone2.kill() {
                    log::error!(
                        "[ffprobe-sidecar] child process could not be killed {}",
                        err
                    );
                }

                result
            }
            Err(err) => Err(err.to_string()),
        }
    }

    /// Get container-level format information
    pub async fn get_container_info(&mut self, path: &str) -> Result<ContainerInfo, String> {
        if !Path::exists(Path::new(path)) {
            return Err(String::from("File does not exist in given path."));
        }

        let mut ffprobe_cmd = self.get_ffprobe_command()?;

        let command = ffprobe_cmd
            .args([
                "-v", "error",
                "-show_entries",
                "format=filename,format_name,format_long_name,duration,size,bit_rate,nb_streams:format_tags",
                "-of", "json",
                path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                        Ok(_) => log::info!("[ffprobe-sidecar] child process killed."),
                        Err(err) => log::error!(
                            "[ffprobe-sidecar] child process could not be killed {}",
                            err
                        ),
                    },
                );

                let thread = tokio::task::spawn(async move {
                    if let Some(stdout) = cp_clone1.take_stdout() {
                        let reader = std::io::BufReader::new(stdout);
                        let mut json_str = String::new();

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                json_str.push_str(&line);
                            } else {
                                break;
                            }
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(format) = json.get("format") {
                                let filename = format
                                    .get("filename")
                                    .and_then(|f| f.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let format_name = format
                                    .get("format_name")
                                    .and_then(|f| f.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let format_long_name = format
                                    .get("format_long_name")
                                    .and_then(|f| f.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let duration = format
                                    .get("duration")
                                    .and_then(|d| d.as_str())
                                    .and_then(|s| s.parse::<f64>().ok());

                                let size = format
                                    .get("size")
                                    .and_then(|s| s.as_str())
                                    .and_then(|s| s.parse::<u64>().ok())
                                    .unwrap_or(0);

                                let bit_rate = format
                                    .get("bit_rate")
                                    .and_then(|b| b.as_str())
                                    .and_then(|b| b.parse::<u64>().ok());

                                let nb_streams = format
                                    .get("nb_streams")
                                    .and_then(|n| n.as_str())
                                    .and_then(|n| n.parse::<u32>().ok())
                                    .unwrap_or(0);

                                let tags = if let Some(tags_obj) = format.get("tags") {
                                    if let Some(tags_map) = tags_obj.as_object() {
                                        let mut tags_vec = Vec::new();
                                        for (key, value) in tags_map.iter() {
                                            if let Some(val_str) = value.as_str() {
                                                tags_vec.push((key.clone(), val_str.to_string()));
                                            }
                                        }
                                        Some(tags_vec)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                if cp_clone1.wait().is_ok() {
                                    (
                                        0,
                                        Some(ContainerInfo {
                                            filename,
                                            format_name,
                                            format_long_name,
                                            duration,
                                            size,
                                            bit_rate,
                                            nb_streams,
                                            tags,
                                        }),
                                    )
                                } else {
                                    (1, None)
                                }
                            } else {
                                (1, None)
                            }
                        } else {
                            (1, None)
                        }
                    } else {
                        (1, None)
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, format_info)) => {
                        if exit_status == 1 {
                            Err("Failed to get format info".to_string())
                        } else if let Some(info) = format_info {
                            Ok(info)
                        } else {
                            Err("Failed to parse format info".to_string())
                        }
                    }
                    Err(err) => Err(err.to_string()),
                };

                window.unlisten(destroy_event_id);
                if let Err(err) = cp_clone2.kill() {
                    log::error!(
                        "[ffprobe-sidecar] child process could not be killed {}",
                        err
                    );
                }

                result
            }
            Err(err) => Err(err.to_string()),
        }
    }

    // Get all audio streams from the media
    pub async fn get_audio_streams(&mut self, path: &str) -> Result<Vec<AudioStream>, String> {
        let mut ffprobe_cmd = self.get_ffprobe_command()?;
        let command = ffprobe_cmd
            .args([
                "-v",
                "error",
                "-select_streams",
                "a",
                "-show_entries",
                "stream=codec_name,codec_long_name,codec_type,profile,channels,channel_layout,sample_rate,sample_fmt,bits_per_sample,bit_rate,duration:stream_tags=language,title",
                "-of",
                "json",
                path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                        Ok(_) => log::info!("[ffprobe-sidecar] child process killed."),
                        Err(err) => log::error!(
                            "[ffprobe-sidecar] child process could not be killed {}",
                            err
                        ),
                    },
                );

                let thread = tokio::task::spawn(async move {
                    let streams = if let Some(stdout) = cp_clone1.take_stdout() {
                        let reader = std::io::BufReader::new(stdout);
                        let mut json_str = String::new();

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                json_str.push_str(&line);
                            } else {
                                break;
                            }
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(streams_array) =
                                json.get("streams").and_then(|s| s.as_array())
                            {
                                let mut result = Vec::new();
                                for stream in streams_array {
                                    // Basic codec info
                                    let codec = stream
                                        .get("codec_name")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let codec_long_name = stream
                                        .get("codec_long_name")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let codec_type = stream
                                        .get("codec_type")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("audio")
                                        .to_string();

                                    let profile = stream
                                        .get("profile")
                                        .and_then(|p| p.as_str())
                                        .map(|s| s.to_string());

                                    // Channels and layout
                                    let channels = stream
                                        .get("channels")
                                        .and_then(|c| c.as_u64())
                                        .unwrap_or(0)
                                        .to_string();

                                    let channel_layout = stream
                                        .get("channel_layout")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    // Sample rate and format
                                    let sample_rate = stream
                                        .get("sample_rate")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("0")
                                        .to_string();

                                    let sample_fmt = stream
                                        .get("sample_fmt")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string());

                                    let bits_per_sample = stream
                                        .get("bits_per_sample")
                                        .and_then(|b| b.as_u64())
                                        .and_then(|b| u32::try_from(b).ok());

                                    // Bitrate and duration
                                    let bit_rate = stream
                                        .get("bit_rate")
                                        .and_then(|b| b.as_str())
                                        .map(|s| s.to_string());

                                    let duration = stream
                                        .get("duration")
                                        .and_then(|d| d.as_str())
                                        .map(|s| s.to_string());

                                    let language = stream
                                        .get("tags")
                                        .and_then(|t| t.get("language"))
                                        .and_then(|l| l.as_str())
                                        .map(|s| s.to_string());

                                    // Tags (language, title, etc.)
                                    let tags = if let Some(tags_obj) = stream.get("tags") {
                                        if let Some(tags_map) = tags_obj.as_object() {
                                            let mut tags_vec = Vec::new();
                                            for (key, value) in tags_map.iter() {
                                                if let Some(val_str) = value.as_str() {
                                                    tags_vec
                                                        .push((key.clone(), val_str.to_string()));
                                                }
                                            }
                                            Some(tags_vec)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    };

                                    result.push(AudioStream {
                                        codec,
                                        codec_long_name,
                                        codec_type,
                                        profile,
                                        channels,
                                        channel_layout,
                                        sample_rate,
                                        sample_fmt,
                                        bits_per_sample,
                                        bit_rate,
                                        duration,
                                        language,
                                        tags,
                                    });
                                }
                                result
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    if cp_clone1.wait().is_ok() {
                        (0, streams)
                    } else {
                        (1, Vec::new())
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, streams)) => {
                        if exit_status == 1 {
                            Err("Failed to get audio streams".to_string())
                        } else {
                            Ok(streams)
                        }
                    }
                    Err(err) => Err(err.to_string()),
                };

                // Cleanup
                window.unlisten(destroy_event_id);
                if let Err(err) = cp_clone2.kill() {
                    log::error!(
                        "[ffprobe-sidecar] child process could not be killed {}",
                        err
                    );
                }

                result
            }
            Err(err) => Err(err.to_string()),
        }
    }

    /// Get all subtitle streams from the media
    pub async fn get_subtitle_streams(
        &mut self,
        path: &str,
    ) -> Result<Vec<SubtitleStream>, String> {
        let mut ffprobe_cmd = self.get_ffprobe_command()?;
        let command = ffprobe_cmd
            .args([
                "-v",
                "error",
                "-select_streams",
                "s",
                "-show_entries",
                "stream=index,codec_name,codec_long_name,codec_type:stream_tags:stream_disposition",
                "-of",
                "json",
                path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                        Ok(_) => log::info!("[ffprobe-sidecar] child process killed."),
                        Err(err) => log::error!(
                            "[ffprobe-sidecar] child process could not be killed {}",
                            err
                        ),
                    },
                );

                let thread = tokio::task::spawn(async move {
                    let streams = if let Some(stdout) = cp_clone1.take_stdout() {
                        let reader = std::io::BufReader::new(stdout);
                        let mut json_str = String::new();

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                json_str.push_str(&line);
                            } else {
                                break;
                            }
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(streams_array) =
                                json.get("streams").and_then(|s| s.as_array())
                            {
                                let mut result = Vec::new();
                                for stream in streams_array {
                                    let index =
                                        stream.get("index").and_then(|i| i.as_u64()).unwrap_or(0)
                                            as u32;

                                    let codec = stream
                                        .get("codec_name")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let codec_long_name = stream
                                        .get("codec_long_name")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let codec_type = stream
                                        .get("codec_type")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("subtitle")
                                        .to_string();

                                    // Extract tags (language, title, etc.)
                                    let (language, title) =
                                        if let Some(tags_obj) = stream.get("tags") {
                                            let lang = tags_obj
                                                .get("language")
                                                .and_then(|l| l.as_str())
                                                .map(|s| s.to_string());
                                            let title_tag = tags_obj
                                                .get("title")
                                                .and_then(|t| t.as_str())
                                                .map(|s| s.to_string());
                                            (lang, title_tag)
                                        } else {
                                            (None, None)
                                        };

                                    // Extract disposition
                                    let disposition =
                                        if let Some(disposition_obj) = stream.get("disposition") {
                                            Disposition {
                                                default: disposition_obj
                                                    .get("default")
                                                    .and_then(|d| d.as_i64())
                                                    .unwrap_or(0)
                                                    != 0,
                                                forced: disposition_obj
                                                    .get("forced")
                                                    .and_then(|f| f.as_i64())
                                                    .unwrap_or(0)
                                                    != 0,
                                                attached_pic: disposition_obj
                                                    .get("attached_pic")
                                                    .and_then(|a| a.as_i64())
                                                    .unwrap_or(0)
                                                    != 0,
                                                comment: disposition_obj
                                                    .get("comment")
                                                    .and_then(|c| c.as_i64())
                                                    .unwrap_or(0)
                                                    != 0,
                                                karaoke: disposition_obj
                                                    .get("karaoke")
                                                    .and_then(|k| k.as_i64())
                                                    .unwrap_or(0)
                                                    != 0,
                                                lyrics: disposition_obj
                                                    .get("lyrics")
                                                    .and_then(|l| l.as_i64())
                                                    .unwrap_or(0)
                                                    != 0,
                                            }
                                        } else {
                                            Disposition::default()
                                        };

                                    result.push(SubtitleStream {
                                        index,
                                        codec,
                                        codec_long_name,
                                        codec_type,
                                        language,
                                        title,
                                        disposition,
                                    });
                                }
                                result
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    if cp_clone1.wait().is_ok() {
                        (0, streams)
                    } else {
                        (1, Vec::new())
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, streams)) => {
                        if exit_status == 1 {
                            Err("Failed to get subtitle streams".to_string())
                        } else {
                            Ok(streams)
                        }
                    }
                    Err(err) => Err(err.to_string()),
                };

                window.unlisten(destroy_event_id);
                if let Err(err) = cp_clone2.kill() {
                    log::error!(
                        "[ffprobe-sidecar] child process could not be killed {}",
                        err
                    );
                }

                result
            }
            Err(err) => Err(err.to_string()),
        }
    }

    /// Get all chapters from the media
    pub async fn get_chapters(&mut self, path: &str) -> Result<Vec<Chapter>, String> {
        let mut ffprobe_cmd = self.get_ffprobe_command()?;
        let command = ffprobe_cmd
            .args([
                "-v",
                "error",
                "-show_entries",
                "chapter=id,time_base,start,end,title",
                "-of",
                "json",
                path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                        Ok(_) => log::info!("[ffprobe-sidecar] child process killed."),
                        Err(err) => log::error!(
                            "[ffprobe-sidecar] child process could not be killed {}",
                            err
                        ),
                    },
                );

                let thread = tokio::task::spawn(async move {
                    let chapters = if let Some(stdout) = cp_clone1.take_stdout() {
                        let reader = std::io::BufReader::new(stdout);
                        let mut json_str = String::new();

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                json_str.push_str(&line);
                            } else {
                                break;
                            }
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(chapters_array) =
                                json.get("chapters").and_then(|c| c.as_array())
                            {
                                let mut result = Vec::new();
                                for chapter in chapters_array {
                                    println!(">> {:?}", chapter);
                                    let id =
                                        chapter.get("id").and_then(|i| i.as_u64()).unwrap_or(0);

                                    let time_base = chapter
                                        .get("time_base")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let start = chapter
                                        .get("start")
                                        .and_then(|s| s.as_f64())
                                        .unwrap_or(0.0)
                                        / 1000.0;

                                    let end =
                                        chapter.get("end").and_then(|e| e.as_f64()).unwrap_or(0.0)
                                            / 1000.0;

                                    let title = chapter
                                        .get("title")
                                        .and_then(|t| t.as_str())
                                        .and_then(|s| Some(s.to_string()));

                                    result.push(Chapter {
                                        id,
                                        time_base,
                                        start,
                                        end,
                                        title,
                                    });
                                }
                                result
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    if cp_clone1.wait().is_ok() {
                        (0, chapters)
                    } else {
                        (1, Vec::new())
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, chapters)) => {
                        if exit_status == 1 {
                            Err("Failed to get chapters".to_string())
                        } else {
                            Ok(chapters)
                        }
                    }
                    Err(err) => Err(err.to_string()),
                };

                window.unlisten(destroy_event_id);
                if let Err(err) = cp_clone2.kill() {
                    log::error!(
                        "[ffprobe-sidecar] child process could not be killed {}",
                        err
                    );
                }

                result
            }
            Err(err) => Err(err.to_string()),
        }
    }
}

/// Helper function to parse fraction strings like "30000/1001" or "30/1"
fn parse_fraction(fraction: &str) -> Option<(u64, u64)> {
    let parts: Vec<&str> = fraction.split('/').collect();
    if parts.len() == 2 {
        let num = parts[0].parse::<u64>().ok()?;
        let den = parts[1].parse::<u64>().ok()?;
        Some((num, den))
    } else {
        None
    }
}
