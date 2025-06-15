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
    // LIGHTNING FAST parameter parsing
    let uri = req.uri();
    let query_params = uri.query().unwrap_or("");

    let mut playlist_id = None;
    let mut youtube_api_key = None;
    let mut give_length = false;

    for param in query_params.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "query" => playlist_id = Some(urlencoding::decode(value).unwrap().to_string()),
                "youtubeAPIKEY" => {
                    youtube_api_key = Some(urlencoding::decode(value).unwrap().to_string())
                }
                "give_length" => give_length = value == "yes",
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

    // Validate playlist ID
    let playlist_id = match playlist_id {
        Some(id) if id != "null" && !id.is_empty() => id,
        _ => {
            let error_response =
                ApiResponse::<()>::error("Please enter a valid Spotify playlist ID".to_string());
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
            api_keys.insert(0, key);
        }
    }

    // GODLY FAST playlist processing
    match process_playlist(&playlist_id, &api_keys).await {
        Ok(youtube_urls) => {
            if youtube_urls
                .iter()
                .any(|url| url.contains("API Limit Exceeded"))
            {
                let error_response = ApiResponse::<()>::error(
                    "API Limit Exceeded for all YouTube API Keys. Please try again later or enter your own YouTube API Key.".to_string()
                );
                return Ok(Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&error_response)?.into())?);
            }

            // Fire and forget analytics update
            let num_songs = youtube_urls.len() as i32;
            tokio::spawn(async move {
                let _ = update_analytics(num_songs, 1).await;
            });

            let mut response = PlaylistResponse {
                list: youtube_urls,
                length: None,
            };

            if give_length {
                response.length = Some(response.list.len());
            }

            let api_response = ApiResponse::success(response);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "public, max-age=300") // 5 minute cache
                .body(serde_json::to_string(&api_response)?.into())?)
        }
        Err(e) => {
            let error_msg = match e.to_string().as_str() {
                msg if msg.contains("404") => {
                    "Playlist not found. Please check if the playlist exists and is public."
                        .to_string()
                }
                msg if msg.contains("Failed to authenticate") => {
                    "Failed to authenticate with Spotify".to_string()
                }
                msg if msg.contains("empty") => "This playlist is empty".to_string(),
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
