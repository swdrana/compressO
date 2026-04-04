use axum::{
    body::Body,
    extract::{Query, Request},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_util::io::ReaderStream;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

/// Query parameters for the video route
#[derive(Debug, Deserialize)]
struct VideoQuery {
    path: String,
}

/// Server state to store the assigned port and shutdown signal
#[derive(Clone)]
pub struct ServerState {
    pub port: u16,
    pub shutdown_tx: broadcast::Sender<()>,
}

/// Start the local HTTP server on Linux for serving video files
/// This works around the WebKit bug where file:// URLs don't work for videos on Linux
///
/// Returns a Result with the ServerState containing the assigned port and shutdown sender
pub async fn start_server() -> Result<ServerState, Box<dyn std::error::Error>> {
    // Create a channel for shutdown signaling
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);

    // Bind to port 0 to let the OS assign a random available port
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    log::info!("Starting local video server on port {}", port);

    // Configure CORS based on debug/release mode
    let cors = configure_cors();

    let app = Router::new()
        .route("/video", get(serve_video))
        .route("/ping", get(ping))
        .layer(cors);

    // Create the server
    let server = axum::serve(listener, app);

    // Spawn the server in the background
    tokio::spawn(async move {
        log::info!("Video server is now accepting connections on port {}", port);

        // Wait for shutdown signal
        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    log::error!("Video server error: {:?}", e);
                } else {
                    log::info!("Video server shut down gracefully");
                }
            }
            _ = shutdown_rx.recv() => {
                log::info!("Video server shutdown signal received");
            }
        }
    });

    // Give the server a moment to start accepting connections
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    log::info!("Video server started successfully on port {}", port);

    Ok(ServerState { port, shutdown_tx })
}

/// Shutdown the video server gracefully
pub fn shutdown_server(state: &ServerState) {
    let _ = state.shutdown_tx.send(());
    log::info!("Video server shutdown signal sent");
}

/// Ping endpoint for health checking
async fn ping() -> &'static str {
    "pong"
}

/// Serve video file with Range header support for seeking
///
/// This endpoint:
/// - Accepts a file path via query parameter
/// - Validates the path exists and is a file
/// - Serves the file with proper MIME type
/// - Supports HTTP Range requests for video seeking
async fn serve_video(
    Query(params): Query<VideoQuery>,
    request: Request,
) -> Result<Response, StatusCode> {
    let path = PathBuf::from(&params.path);

    // Validate the path exists and is a file
    if !path.exists() {
        log::error!("Video file not found: {:?}", path);
        return Err(StatusCode::NOT_FOUND);
    }

    if !path.is_file() {
        log::error!("Path is not a file: {:?}", path);
        return Err(StatusCode::FORBIDDEN);
    }

    // Get file metadata
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(e) => {
            log::error!("Failed to get file metadata: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let file_len = metadata.len();

    // Detect MIME type based on file extension
    let mime_type = mime_type_from_path(&path);

    // Open the file
    let file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(e) => {
            log::error!("Failed to open file: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Handle Range header for seeking support
    let headers = request.headers();
    let range_header = headers.get(header::RANGE);

    if let Some(range) = range_header {
        // Parse Range header (format: "bytes=start-end")
        let range_str = match range.to_str() {
            Ok(s) => s,
            Err(_) => return Err(StatusCode::BAD_REQUEST),
        };

        if let Some((start, end)) = parse_range_header(range_str, file_len) {
            let content_length = end - start + 1;

            // Create a reader that starts from the requested position
            let mut file = match tokio::fs::File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Failed to reopen file for range request: {:?}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };

            match file.seek(std::io::SeekFrom::Start(start)).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to seek in file: {:?}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }

            // Create a limited stream that only reads the requested range
            let stream = ReaderStream::new(file.take(content_length));
            let body = Body::from_stream(stream);

            return Ok((
                StatusCode::PARTIAL_CONTENT,
                [(header::CONTENT_TYPE, mime_type)],
                [(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end, file_len),
                )],
                [(header::CONTENT_LENGTH, content_length.to_string())],
                [(header::ACCEPT_RANGES, "bytes")],
                body,
            )
                .into_response());
        }
    }

    // No Range header, serve the entire file
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, mime_type)],
        [(header::CONTENT_LENGTH, file_len.to_string())],
        [(header::ACCEPT_RANGES, "bytes")],
        body,
    )
        .into_response())
}

/// Parse HTTP Range header
/// Format: "bytes=start-end" where end is optional
fn parse_range_header(range: &str, file_len: u64) -> Option<(u64, u64)> {
    // Range header format: "bytes=start-end"
    let range = range.strip_prefix("bytes=")?;

    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    let start: u64 = parts[0].parse().ok()?;

    let end = if parts[1].is_empty() {
        // If end is not specified, use the end of the file
        file_len - 1
    } else {
        parts[1].parse().ok()?
    };

    // Validate range
    if start >= file_len || end >= file_len || start > end {
        return None;
    }

    Some((start, end))
}

/// Detect MIME type based on file extension
fn mime_type_from_path(path: &PathBuf) -> &'static str {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mkv" => "video/x-matroska",
            "avi" => "video/x-msvideo",
            "mov" => "video/quicktime",
            "flv" => "video/x-flv",
            "m4v" => "video/x-m4v",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "flac" => "audio/flac",
            "m4a" => "audio/mp4",
            "aac" => "audio/aac",
            _ => "application/octet-stream",
        })
        .unwrap_or("application/octet-stream")
}

/// Configure CORS based on build type
fn configure_cors() -> CorsLayer {
    #[cfg(debug_assertions)]
    {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([axum::http::Method::GET])
            .allow_headers(Any)
            .max_age(Duration::from_secs(3600))
    }

    #[cfg(not(debug_assertions))]
    {
        CorsLayer::new()
            .allow_origin("tauri://localhost".parse::<HeaderValue>().unwrap())
            .allow_methods([axum::http::Method::GET])
            .allow_headers(Any)
            .max_age(Duration::from_secs(3600))
    }
}
