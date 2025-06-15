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
    // Parse query parameters with MAXIMUM SPEED
    let uri = req.uri();
    let query_params = uri.query().unwrap_or("");

    let mut song_id = None;
    let mut youtube_api_key = None;

    // Ultra-fast parameter parsing
    for param in query_params.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "query" => song_id = Some(urlencoding::decode(value).unwrap().to_string()),
                "youtubeAPIKEY" => {
                    youtube_api_key = Some(urlencoding::decode(value).unwrap().to_string())
                }
                _ => {}
            }
        }
    }

    // Check for API key in headers
    if youtube_api_key.is_none() {
        youtube_api_key = req
            .headers()
            .get("X-YouTube-API-Key")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());
    }

    // Validate song ID
    let song_id = match song_id {
        Some(id) if id != "null" && !id.is_empty() => id,
        _ => {
            let error_response =
                ApiResponse::<()>::error("Please enter a valid Spotify song ID".to_string());
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&error_response)?.into())?);
        }
    };

    // Get YouTube API keys with failover
    let mut api_keys = get_youtube_api_keys();
    if let Some(key) = youtube_api_key {
        if key != "default" {
            api_keys.insert(0, key); // Prioritize user-provided key
        }
    }

    // LIGHTNING FAST song processing
    match process_song(&song_id, &api_keys).await {
        Ok(youtube_url) => {
            // Fire and forget analytics update for SPEED
            tokio::spawn(async move {
                let _ = update_analytics(1, 0).await;
            });

            let response = ApiResponse::success(SongResponse { url: youtube_url });
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "public, max-age=300") // 5 minute cache
                .body(serde_json::to_string(&response)?.into())?)
        }
        Err(e) => {
            let error_msg = match e.to_string().as_str() {
                msg if msg.contains("Failed to fetch song info") => 
                    "Could not fetch song information from Spotify. Please check if the song ID is valid.".to_string(),
                msg if msg.contains("Failed to search YouTube") || msg.contains("API Limit Exceeded") => 
                    "API Limit Exceeded for all YouTube API Keys. Please try again later or enter your own YouTube API Key.".to_string(),
                _ => "An unexpected error occurred. Please try again later.".to_string(),
            };

            let error_response = ApiResponse::<()>::error(error_msg);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
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
