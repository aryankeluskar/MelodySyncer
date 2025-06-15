use futures::future::join_all;
use melody_syncer_rust::{
    get_youtube_api_keys, search_track_yt, update_analytics, ApiResponse, HTTP_CLIENT,
    SPOTIFY_CLIENT,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

#[derive(Deserialize)]
struct SpotifyPlaylistResponse {
    items: Vec<SpotifyPlaylistItem>,
}

#[derive(Deserialize)]
struct SpotifyPlaylistItem {
    track: Option<SpotifyTrack>,
}

#[derive(Deserialize)]
struct SpotifyTrack {
    id: Option<String>,
    name: String,
    artists: Vec<SpotifyArtist>,
    album: SpotifyAlbum,
    duration_ms: u32,
}

#[derive(Deserialize)]
struct SpotifyArtist {
    name: String,
}

#[derive(Deserialize)]
struct SpotifyAlbum {
    name: String,
}

#[derive(Serialize)]
struct PlaylistResponse {
    list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(req: Request) -> Result<Response<Body>, Error> {
    // parameter parsing with optimized allocations
    let uri = req.uri();
    let query_params = uri.query().unwrap_or("");

    let mut playlist_id = None;
    let mut youtube_api_key = None;
    let mut give_length = false;

    // parsing - fewer string allocations
    for param in query_params.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "query" => {
                    let decoded = urlencoding::decode(value).unwrap();
                    if decoded != "null" && !decoded.is_empty() {
                        playlist_id = Some(decoded.into_owned());
                    }
                },
                "youtubeAPIKEY" => {
                    let decoded = urlencoding::decode(value).unwrap();
                    if decoded != "default" && !decoded.is_empty() {
                        youtube_api_key = Some(decoded.into_owned());
                    }
                }
                "give_length" => give_length = value == "yes",
                _ => {}
            }
        }
    }

    // header access
    if youtube_api_key.is_none() {
        youtube_api_key = req
            .headers()
            .get("X-YouTube-API-Key")
            .and_then(|h| h.to_str().ok())
            .filter(|s| !s.is_empty() && *s != "default")
            .map(|s| s.to_string());
    }

    // validation
    let playlist_id = match playlist_id {
        Some(id) => id,
        None => {
            let error_response =
                ApiResponse::<()>::error("Please enter a valid Spotify playlist ID".to_string());
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
        api_keys.insert(0, key);
    }

    // playlist processing
    match process_playlist(&playlist_id, &api_keys).await {
        Ok(youtube_urls) => {
            // Check for API errors in results
            if youtube_urls
                .iter()
                .any(|url| url.contains("API Limit Exceeded"))
            {
                let error_response = ApiResponse::<()>::error(
                    "API Limit Exceeded for all YouTube API Keys. Please try again later or provide your own YouTube API Key.".to_string()
                );
                return Ok(Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .header("Content-Type", "application/json")
                    .header("Cache-Control", "no-cache")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(serde_json::to_string(&error_response)?.into())?);
            }

            // Prepare response data
            let mut response = PlaylistResponse {
                list: youtube_urls,
                length: None,
            };

            if give_length {
                response.length = Some(response.list.len());
            }

            let num_songs = response.list.len() as i32;
            let api_response = ApiResponse::success(response);
            let response_body = serde_json::to_string(&api_response)?;

            // CRITICAL: Send response IMMEDIATELY
            let http_response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "public, max-age=600") // 10 minute cache - INCREASED
                .header("Access-Control-Allow-Origin", "*")  // CORS support
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type, X-YouTube-API-Key")
                .header("Vary", "Accept-Encoding")  // Compression support
                .body(response_body.into())?;

            // Analytics AFTER response - COMPLETELY ASYNC
            tokio::spawn(async move {
                let _ = update_analytics(num_songs, 1).await;
            });

            Ok(http_response)
        }
        Err(e) => {
            // OPTIMIZED error handling with proper status codes
            let (error_msg, status_code) = match e.to_string().as_str() {
                msg if msg.contains("404") || msg.contains("not found") => {
                    ("Playlist not found. Please check if the playlist exists and is public.".to_string(), StatusCode::NOT_FOUND)
                }
                msg if msg.contains("Failed to authenticate") => {
                    ("Failed to authenticate with Spotify".to_string(), StatusCode::UNAUTHORIZED)
                }
                msg if msg.contains("empty") => ("This playlist is empty".to_string(), StatusCode::NOT_FOUND),
                msg if msg.contains("timeout") => ("Request timeout. Please try again.".to_string(), StatusCode::REQUEST_TIMEOUT),
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

async fn process_playlist(
    playlist_id: &str,
    api_keys: &[String],
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get Spotify token with caching
    let mut spotify_client = SPOTIFY_CLIENT.write().await;
    let token = spotify_client.get_token().await?;
    drop(spotify_client); // Release lock ASAP

    // Fetch playlist from Spotify
    let url = format!(
        "https://api.spotify.com/v1/playlists/{}/tracks",
        playlist_id
    );
    let response = HTTP_CLIENT
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if response.status() == 404 {
        return Err("Playlist not found".into());
    }

    if !response.status().is_success() {
        return Err(format!("Failed to fetch playlist: {}", response.status()).into());
    }

    let playlist: SpotifyPlaylistResponse = response.json().await?;

    if playlist.items.is_empty() {
        return Err("This playlist is empty".into());
    }

    // Filter valid tracks and collect info
    let tracks: Vec<_> = playlist
        .items
        .into_iter()
        .filter_map(|item| {
            item.track.and_then(|track| {
                track.id.and_then(|id| {
                    track.artists.first().map(|artist| (
                        id,
                        track.name,
                        artist.name.clone(),
                        track.album.name,
                        track.duration_ms,
                    ))
                })
            })
        })
        .collect();

    if tracks.is_empty() {
        return Err("No valid songs found in playlist".into());
    }

    // MASSIVE PARALLEL PROCESSING FOR GODLY SPEED
    // Process all songs simultaneously with Rust's zero-cost abstractions
    let search_tasks: Vec<_> = tracks
        .iter()
        .map(|(id, name, artist, album, duration)| {
            let api_keys = api_keys.to_vec();
            let name = name.clone();
            let artist = artist.clone();
            let album = album.clone();
            let duration = *duration;

            async move {
                match search_track_yt(&name, &artist, &album, duration, &api_keys).await {
                    Ok(video_id) => {
                        if video_id == "dQw4w9WgXcQ" {
                            "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string()
                        } else {
                            format!("https://www.youtube.com/watch?v={}", video_id)
                        }
                    }
                    Err(_) => "API Limit Exceeded for all YouTube API Keys".to_string(),
                }
            }
        })
        .collect();

    // Wait for all searches to complete in parallel - MAXIMUM THROUGHPUT
    let results = join_all(search_tasks).await;

    Ok(results)
}
