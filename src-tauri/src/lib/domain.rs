use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum::{AsRefStr, EnumProperty};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompressionResult {
    pub video_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_metadata: Option<FileMetadata>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    pub path: String,
    pub file_name: String,
    pub mime_type: String,
    pub extension: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoCompressionProgress {
    pub video_id: String,
    pub batch_id: String,
    pub file_name: String,
    pub current_duration: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoThumbnail {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
}

#[derive(Clone, AsRefStr)]
pub enum CustomEvents {
    VideoCompressionProgress,
    CancelInProgressCompression,
    BatchCompressionProgress,
    BatchCompressionIndividualCompressionCompletion,
}

#[derive(EnumProperty)]
pub enum TauriEvents {
    #[strum(props(key = "tauri://destroyed"))]
    Destroyed,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelInProgressCompressionPayload {
    pub video_id: String,
    pub batch_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub duration: Option<String>,
    pub dimensions: Option<(u32, u32)>,
    pub fps: Option<f32>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCoordinates {
    pub top: u32,
    pub left: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFlip {
    pub horizontal: bool,
    pub vertical: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoTransforms {
    pub crop: VideoCoordinates,
    pub rotate: i32,
    pub flip: VideoFlip,
}

use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCompressionResult {
    pub results: HashMap<String, CompressionResult>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchCompressionProgress {
    pub batch_id: String,
    pub current_index: usize,
    pub total_count: usize,
    pub video_progress: VideoCompressionProgress,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchCompressionIndividualCompressionResult {
    pub batch_id: String,
    pub result: CompressionResult,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoFileMetadata {
    pub id: String,
    pub file_name: String,
    pub path: String,
    pub size: u64,
    pub thumbnail_path: Option<String>,
    pub duration: Option<String>,
    pub dimensions: Option<(u32, u32)>,
    pub fps: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoWithPath {
    pub video_path: String,
    pub video_id: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoCompressionConfig {
    pub video_id: String,
    pub video_path: String,
    pub convert_to_extension: String,
    pub preset_name: Option<String>,
    pub should_mute_video: bool,
    pub quality: u16,
    pub dimensions: Option<(u32, u32)>,
    pub fps: Option<String>,
    pub transforms_history: Option<Vec<Value>>,
    pub metadata_config: Option<VideoMetadataConfig>,
    pub custom_thumbnail_path: Option<String>,
    pub should_enable_custom_thumbnail: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadataConfig {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub comment: Option<String>,
    pub genre: Option<String>,
    pub creation_time: Option<String>,
}
