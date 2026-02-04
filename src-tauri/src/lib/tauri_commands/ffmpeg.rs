use crate::{
    domain::{
        BatchCompressionResult, CompressionResult, VideoCompressionConfig, VideoInfo,
        VideoMetadataConfig, VideoThumbnail,
    },
    ffmpeg::{self},
    fs::delete_stale_files,
};
use serde_json::Value;

#[tauri::command]
pub async fn compress_video(
    app: tauri::AppHandle,
    video_path: &str,
    convert_to_extension: &str,
    preset_name: Option<&str>,
    video_id: &str,
    should_mute_video: bool,
    quality: u16,
    dimensions: Option<(u32, u32)>,
    fps: Option<&str>,
    transforms_history: Option<Vec<Value>>,
    metadata_config: Option<VideoMetadataConfig>,
    custom_thumbnail_path: Option<&str>,
) -> Result<CompressionResult, String> {
    let mut ffmpeg = ffmpeg::FFMPEG::new(&app)?;
    if let Ok(files) =
        delete_stale_files(ffmpeg.get_asset_dir().as_str(), 24 * 60 * 60 * 1000).await
    {
        log::debug!(
            "[main] Stale files deleted. Number of deleted files = {}",
            files.len()
        )
    };
    match ffmpeg
        .compress_video(
            video_path,
            convert_to_extension,
            preset_name,
            video_id,
            None,
            should_mute_video,
            quality,
            dimensions,
            fps,
            transforms_history.as_ref(),
            metadata_config.as_ref(),
            custom_thumbnail_path,
        )
        .await
    {
        Ok(result) => Ok(result),
        Err(err) => Err(err),
    }
}

#[tauri::command]
pub async fn generate_video_thumbnail(
    app: tauri::AppHandle,
    video_path: &str,
) -> Result<VideoThumbnail, String> {
    let mut ffmpeg = ffmpeg::FFMPEG::new(&app)?;
    ffmpeg.generate_video_thumbnail(video_path).await
}

#[tauri::command]
pub async fn get_video_info(app: tauri::AppHandle, video_path: &str) -> Result<VideoInfo, String> {
    let mut ffmpeg = ffmpeg::FFMPEG::new(&app)?;
    ffmpeg.get_video_info(video_path).await
}

#[tauri::command]
pub async fn compress_videos_batch(
    app: tauri::AppHandle,
    batch_id: &str,
    videos: Vec<VideoCompressionConfig>,
) -> Result<BatchCompressionResult, String> {
    let mut ffmpeg = ffmpeg::FFMPEG::new(&app)?;
    if let Ok(files) =
        delete_stale_files(ffmpeg.get_asset_dir().as_str(), 24 * 60 * 60 * 1000).await
    {
        log::debug!(
            "[main] Stale files deleted. Number of deleted files = {}",
            files.len()
        )
    };
    ffmpeg
        .compress_videos_batch(batch_id, videos)
        .await
        .map(|result| Ok(result))
        .unwrap_or_else(|err| Err(err))
}
