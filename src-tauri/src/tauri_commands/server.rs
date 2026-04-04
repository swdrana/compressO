#![allow(unexpected_cfgs)]

use crate::core::server::ServerState;

/// Get the local video server URL
///
/// This command returns the base URL of the local HTTP server that serves video files.
/// The frontend should use this URL to construct video URLs instead of file:// paths.
///
/// # Returns
/// * `String` - The base server URL (e.g., "http://127.0.0.1:54321")
///
/// # Example
/// ```javascript
/// const serverUrl = await invoke('get_video_server_url');
/// const videoUrl = `${serverUrl}/video?path=${encodeURIComponent(filePath)}`;
/// ```
#[tauri::command]
pub fn get_video_server_url(state: tauri::State<ServerState>) -> Result<String, String> {
    Ok(format!("http://127.0.0.1:{}", state.port))
}

/// Construct a video URL for the local server
///
/// This is a convenience command that constructs the full video URL with the file path.
///
/// # Arguments
/// * `file_path` - The absolute path to the video file (can be asset:// URL)
///
/// # Returns
/// * `String` - The complete video URL
///
/// # Example
/// ```javascript
/// const videoUrl = await invoke('get_video_url', { filePath: '/path/to/video.mp4' });
/// // Returns: "http://127.0.0.1:54321/video?path=%2Fpath%2Fto%2Fvideo.mp4"
/// ```
#[tauri::command]
pub fn get_video_url(
    state: tauri::State<ServerState>,
    file_path: String,
) -> Result<String, String> {
    // Decode the URL first if it's already encoded (asset:// URLs are encoded by Tauri)
    let decoded_path: String = match urlencoding::decode(&file_path) {
        Ok(path) => path.into_owned(),
        Err(e) => {
            println!("Failed to decode file path: {}, using as-is", e);
            file_path.clone()
        }
    };

    println!(
        "Received request for video URL. Original: '{}', Decoded: '{}'",
        file_path, decoded_path
    );

    // If it's an asset:// URL, we want to extract the actual file path
    // asset://localhost/path/to/file -> /path/to/file
    let actual_path = if decoded_path.starts_with("asset://localhost/") {
        decoded_path
            .strip_prefix("asset://localhost/")
            .unwrap_or(&decoded_path)
    } else if decoded_path.starts_with("asset://") {
        decoded_path
            .strip_prefix("asset://")
            .unwrap_or(&decoded_path)
    } else {
        &decoded_path
    };

    // URL encode the file path for the query parameter
    let encoded_path = urlencoding::encode(actual_path);

    println!("Final encoded path: '{}'", encoded_path);

    Ok(format!(
        "http://127.0.0.1:{}/video?path={}",
        state.port, encoded_path
    ))
}
