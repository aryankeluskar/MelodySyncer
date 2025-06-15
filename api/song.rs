use melody_syncer_rust::{
    get_song_info, get_youtube_api_keys, search_track_yt, update_analytics, ApiResponse,
};
use serde::Serialize;
use serde_json::json;
use std::env;
use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

#[derive(Serialize)]
struct SongResponse {
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(req: Request) -> Result<Response<Body>, Error> {
    // parameter parsing with optimized string operations
    let uri = req.uri();
    let query_params = uri.query().unwrap_or("");

    let mut song_id = None;
    let mut youtube_api_key = None;

    // parameter parsing - fewer allocations
    for param in query_params.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "query" => {
                    let decoded = urlencoding::decode(value).unwrap();
                    if decoded != "null" && !decoded.is_empty() {
                        song_id = Some(decoded.into_owned());
                    }
                },
                "youtubeAPIKEY" => {
                    let decoded = urlencoding::decode(value).unwrap();
                    if decoded != "default" && !decoded.is_empty() {
                        youtube_api_key = Some(decoded.into_owned());
                    }
                }
                _ => {}
            }
        }
    }

    // Check for API key in headers - header access
    if youtube_api_key.is_none() {
        youtube_api_key = req
            .headers()
            .get("X-YouTube-API-Key")
            .and_then(|h| h.to_str().ok())
            .filter(|s| !s.is_empty() && *s != "default")
            .map(|s| s.to_string());
    }

    // FAST validation
    let song_id = match song_id {
        Some(id) => id,
        None => {
            let error_response =
                ApiResponse::<()>::error("Please enter a valid Spotify song ID".to_string());
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "no-cache")
                .body(serde_json::to_string(&error_response)?.into())?);
        }
    };

    // Get YouTube API keys with failover
    let mut api_keys = get_youtube_api_keys();
    if let Some(key) = youtube_api_key {
        api_keys.insert(0, key); // Prioritize user-provided key
    }

    // song processing
    match process_song(&song_id, &api_keys).await {
        Ok(youtube_url) => {
            let response_data = ApiResponse::success(SongResponse { url: youtube_url });
            let response_body = serde_json::to_string(&response_data)?;
            
            // Send response IMMEDIATELY - NO BLOCKING
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "public, max-age=600") // 10 minute cache
                .header("Access-Control-Allow-Origin", "*")  // CORS for faster browser access
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type, X-YouTube-API-Key")
                .header("Vary", "Accept-Encoding")  // Support compression
                .body(response_body.into())?;
                
            // Analytics AFTER response - no await
            tokio::spawn(async move {
                let _ = update_analytics(1, 0).await;
            });
            
            Ok(response)
        }
        Err(e) => {
            // error handling with specific error types
            let (error_msg, status_code) = match e.to_string().as_str() {
                msg if msg.contains("Failed to fetch song info") || msg.contains("404") => 
                    ("Could not fetch song information from Spotify. Please check if the song ID is valid.".to_string(), StatusCode::NOT_FOUND),
                msg if msg.contains("Failed to search YouTube") || msg.contains("API Limit Exceeded") => 
                    ("API Limit Exceeded for all YouTube API Keys. Please try again later or provide your own YouTube API Key.".to_string(), StatusCode::TOO_MANY_REQUESTS),
                msg if msg.contains("timeout") =>
                    ("Request timeout. Please try again.".to_string(), StatusCode::REQUEST_TIMEOUT),
                _ => ("An unexpected error occurred. Please try again later.".to_string(), StatusCode::INTERNAL_SERVER_ERROR),
            };

            let error_response = ApiResponse::<()>::error(error_msg);
            Ok(Response::builder()
                .status(status_code)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "no-cache")
                .header("Access-Control-Allow-Origin", "*")
                .body(serde_json::to_string(&error_response)?.into())?)
        }
    }
}

async fn process_song(
    song_id: &str,
    api_keys: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch song info from Spotify
    let song = get_song_info(song_id).await?;

    // Search YouTube for the best match
    let video_id = search_track_yt(
        &song.name,
        &song.artists[0].name,
        &song.album.name,
        song.duration_ms,
        api_keys,
    )
    .await?;

    if video_id == "dQw4w9WgXcQ" {
        return Err("No matching song found on YouTube".into());
    }

    Ok(format!("https://www.youtube.com/watch?v={}", video_id))
}
