use crate::domain::{TauriEvents, VideoInfo};
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
    ffprobe: Command,
}

impl FFPROBE {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        match app.shell().sidecar("compresso_ffprobe") {
            Ok(command) => Ok(Self {
                app: app.to_owned(),
                ffprobe: Command::from(command),
            }),
            Err(err) => Err(format!("[ffprobe-sidecar]: {:?}", err.to_string())),
        }
    }

    /// Gets video information (duration, dimensions, fps) using ffprobe JSON output
    pub async fn get_video_info(&mut self, video_path: &str) -> Result<VideoInfo, String> {
        if !Path::exists(Path::new(video_path)) {
            return Err(String::from("File does not exist in given path."));
        }

        let command = self
            .ffprobe
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

    // Get the video codec name from the source video
    pub async fn get_video_codec(&mut self, path: &str) -> Result<Option<String>, String> {
        let command = self
            .ffprobe
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=codec_name",
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
                    let codec = if let Some(stdout) = cp_clone1.take_stdout() {
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
                            if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
                                if let Some(stream) = streams.first() {
                                    stream
                                        .get("codec_name")
                                        .and_then(|c| c.as_str())
                                        .map(String::from)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if cp_clone1.wait().is_ok() {
                        (0, codec)
                    } else {
                        (1, None)
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, codec)) => {
                        if exit_status == 1 {
                            Err("Failed to get video codec".to_string())
                        } else {
                            Ok(codec)
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

    // Check if the media has an audio stream or not
    pub async fn has_audio_stream(&mut self, path: &str) -> Result<bool, String> {
        let command = self
            .ffprobe
            .args([
                "-i",
                path,
                "-show_streams",
                "-select_streams",
                "a",
                "-loglevel",
                "quiet",
                "-print_format",
                "json",
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
                    let has_audio = if let Some(stdout) = cp_clone1.take_stdout() {
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
                            if let Some(streams) = json.get("streams") {
                                if let Some(streams_array) = streams.as_array() {
                                    !streams_array.is_empty()
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if cp_clone1.wait().is_ok() {
                        (0, has_audio)
                    } else {
                        (1, has_audio)
                    }
                });

                let result = match thread.await {
                    Ok((exit_status, has_audio)) => {
                        if exit_status == 1 {
                            Err("Failed to check audio stream".to_string())
                        } else {
                            Ok(has_audio)
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
