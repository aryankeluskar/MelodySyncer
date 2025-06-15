use anyhow::{anyhow, Result};
use base64::Engine;
use futures::future::join_all;
use mongodb::{bson::{doc, Document}, Client as MongoClient, Collection};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, env, sync::Arc, time::Duration};
use tokio::sync::RwLock;

// Global HTTP client with connection pooling for MAXIMUM SPEED
pub static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .pool_max_idle_per_host(20)
        .pool_idle_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .tcp_keepalive(Duration::from_secs(60))
        .tcp_nodelay(true)
        .use_rustls_tls()
        .build()
        .expect("Failed to create HTTP client")
});

// Global MongoDB client with connection pooling  
pub static MONGO_CLIENT: Lazy<Option<MongoClient>> = Lazy::new(|| {
    if let Ok(uri) = env::var("MONGO_URI") {
        tokio::runtime::Handle::try_current()
            .ok()
            .and_then(|_| {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        MongoClient::with_uri_str(&uri).await.ok()
                    })
                })
            })
    } else {
        None
    }
});

// Spotify API client with token caching for SPEED
pub static SPOTIFY_CLIENT: Lazy<Arc<RwLock<SpotifyClient>>> =
    Lazy::new(|| Arc::new(RwLock::new(SpotifyClient::new())));

#[derive(Debug, Clone)]
pub struct SpotifyClient {
    pub token: Option<String>,
    pub token_expires_at: Option<std::time::Instant>,
}

impl SpotifyClient {
    pub fn new() -> Self {
        Self {
            token: None,
            token_expires_at: None,
        }
    }

    pub async fn get_token(&mut self) -> Result<String> {
        // Check if we have a valid cached token
        if let (Some(token), Some(expires_at)) = (&self.token, &self.token_expires_at) {
            if expires_at > &std::time::Instant::now() {
                return Ok(token.clone());
            }
        }

        // Get new token
        let client_id =
            env::var("SPOTIPY_CLIENT_ID").map_err(|_| anyhow!("SPOTIPY_CLIENT_ID not found"))?;
        let client_secret = env::var("SPOTIPY_CLIENT_SECRET")
            .map_err(|_| anyhow!("SPOTIPY_CLIENT_SECRET not found"))?;

        let auth_string = format!("{}:{}", client_id, client_secret);
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(auth_string.as_bytes())
        );

        let params = [("grant_type", "client_credentials")];

        let response = HTTP_CLIENT
            .post("https://accounts.spotify.com/api/token")
            .header("Authorization", auth_header)
            .form(&params)
            .send()
            .await?;

        let token_response: Value = response.json().await?;
        let access_token = token_response["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access token in response"))?
            .to_string();

        let expires_in = token_response["expires_in"].as_u64().unwrap_or(3600);

        // Cache the token
        self.token = Some(access_token.clone());
        self.token_expires_at = Some(
            std::time::Instant::now() + Duration::from_secs(expires_in - 60), // 1 minute buffer
        );

        Ok(access_token)
    }
}

#[derive(Debug, Deserialize)]
pub struct SpotifySong {
    pub name: String,
    pub artists: Vec<SpotifyArtist>,
    pub album: SpotifyAlbum,
    pub duration_ms: u32,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyArtist {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyAlbum {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct YouTubeSearchResponse {
    pub items: Vec<YouTubeVideo>,
    #[serde(rename = "pageInfo")]
    pub page_info: YouTubePageInfo,
}

#[derive(Debug, Deserialize)]
pub struct YouTubePageInfo {
    #[serde(rename = "totalResults")]
    pub total_results: u32,
}

#[derive(Debug, Deserialize)]
pub struct YouTubeVideo {
    pub id: YouTubeVideoId,
    pub snippet: YouTubeSnippet,
}

#[derive(Debug, Deserialize)]
pub struct YouTubeVideoId {
    #[serde(rename = "videoId")]
    pub video_id: String,
}

#[derive(Debug, Deserialize)]
pub struct YouTubeSnippet {
    pub title: String,
    #[serde(rename = "channelTitle")]
    pub channel_title: String,
}

#[derive(Debug, Deserialize)]
pub struct YouTubeVideoDetails {
    pub items: Vec<YouTubeVideoDetailsItem>,
}

#[derive(Debug, Deserialize)]
pub struct YouTubeVideoDetailsItem {
    #[serde(rename = "contentDetails")]
    pub content_details: YouTubeContentDetails,
}

#[derive(Debug, Deserialize)]
pub struct YouTubeContentDetails {
    pub duration: String,
}

// LIGHTNING FAST song info fetcher
pub async fn get_song_info(song_id: &str) -> Result<SpotifySong> {
    let mut spotify_client = SPOTIFY_CLIENT.write().await;
    let token = spotify_client.get_token().await?;
    drop(spotify_client); // Release lock ASAP for speed

    let url = format!("https://api.spotify.com/v1/tracks/{}", song_id);
    let response = HTTP_CLIENT
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("Failed to fetch song info: {}", response.status()));
    }

    let song: SpotifySong = response.json().await?;
    Ok(song)
}

// ULTRA FAST YouTube duration parsing - converts ISO 8601 to milliseconds
pub fn parse_iso_duration(duration: &str) -> u32 {
    static DURATION_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?").unwrap());

    let caps = match DURATION_REGEX.captures(duration) {
        Some(caps) => caps,
        None => return 0,
    };

    let hours = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .unwrap_or(0);
    let minutes = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .unwrap_or(0);
    let seconds = caps
        .get(3)
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .unwrap_or(0);

    (hours * 3600 + minutes * 60 + seconds) * 1000
}

// BLAZING FAST YouTube video duration fetcher with API key rotation
pub async fn get_track_duration_yt(video_id: &str, api_keys: &[String]) -> Result<u32> {
    for api_key in api_keys {
        let url = format!(
            "https://youtube.googleapis.com/youtube/v3/videos?part=contentDetails&key={}&id={}",
            api_key, video_id
        );

        match HTTP_CLIENT.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                if let Ok(details) = response.json::<YouTubeVideoDetails>().await {
                    if let Some(item) = details.items.first() {
                        return Ok(parse_iso_duration(&item.content_details.duration));
                    }
                }
            }
            _ => continue, // Try next API key
        }
    }

    Err(anyhow!("Failed to get video duration with all API keys"))
}

// GODLY FAST YouTube search with custom accuracy scoring
pub async fn search_track_yt(
    song_name: &str,
    artist_name: &str,
    album_name: &str,
    song_duration: u32,
    api_keys: &[String],
) -> Result<String> {
    let search_query = format!(
        "{} {} {} Official Audio",
        song_name, album_name, artist_name
    );

    for api_key in api_keys {
        let url = format!(
            "https://youtube.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&key={}",
            urlencoding::encode(&search_query),
            api_key
        );

        match HTTP_CLIENT.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                if let Ok(search_result) = response.json::<YouTubeSearchResponse>().await {
                    if search_result.items.is_empty() {
                        return Ok("dQw4w9WgXcQ".to_string()); // Fallback
                    }

                    // PARALLEL PROCESSING FOR MAXIMUM SPEED
                    let duration_tasks: Vec<_> = search_result
                        .items
                        .iter()
                        .map(|item| get_track_duration_yt(&item.id.video_id, api_keys))
                        .collect();

                    let durations = join_all(duration_tasks).await;

                    // LIGHTNING FAST ACCURACY SCORING
                    let mut best_score = 0;
                    let mut best_video_id = &search_result.items[0].id.video_id;

                    for (i, item) in search_result.items.iter().enumerate() {
                        let mut score = 0;

                        // +2 for Topic channels (official artist channels)
                        if item.snippet.channel_title.contains("Topic") {
                            score += 2;
                        }

                        // +2 for Official Audio
                        if item.snippet.title.contains("Official Audio")
                            || item.snippet.title.contains("Full Audio Song")
                        {
                            score += 2;
                        }

                        // +5 for duration match within 2 seconds
                        if let Ok(video_duration) = &durations[i] {
                            if ((*video_duration as i64) - (song_duration as i64)).abs() <= 2000 {
                                score += 5;
                            }
                        }

                        if score > best_score {
                            best_score = score;
                            best_video_id = &item.id.video_id;
                        }
                    }

                    return Ok(best_video_id.clone());
                }
            }
            _ => continue, // Try next API key
        }
    }

    Err(anyhow!("Failed to search YouTube with all API keys"))
}

// SUPER FAST analytics updater
pub async fn update_analytics(songs_converted: i32, playlists_converted: i32) -> Result<()> {
    if let Some(client) = MONGO_CLIENT.as_ref() {
        let db_name = env::var("MONGO_DB").unwrap_or_default();
        let collection_name = env::var("MONGO_COLLECTION").unwrap_or_default();

        if !db_name.is_empty() && !collection_name.is_empty() {
            let db = client.database(&db_name);
            let collection: Collection<Document> = db.collection(&collection_name);

            let update_doc = doc! {
                "$inc": {
                    "ISOtotalCalls": 5 * songs_converted,
                    "MESOtotalCalls": 1,
                    "MESOsongsConverted": songs_converted,
                    "MESOplaylistsConverted": playlists_converted,
                }
            };

            // Fire and forget for maximum speed
            tokio::spawn(async move {
                let _ = collection
                    .update_many(
                        doc! {},
                        update_doc,
                        mongodb::options::UpdateOptions::builder()
                            .upsert(true)
                            .build(),
                    )
                    .await;
            });
        }
    }

    Ok(())
}

// Cached YouTube API keys for MAXIMUM SPEED
static YOUTUBE_API_KEYS: Lazy<Vec<String>> = Lazy::new(|| {
    let mut keys = Vec::new();
    
    // Try to get up to 10 API keys efficiently
    for i in 1..=10 {
        let key_name = if i == 1 {
            "YOUTUBE_API_KEY".to_string()
        } else {
            format!("YOUTUBE_API_KEY{}", i)
        };
        
        if let Ok(key) = env::var(&key_name) {
            keys.push(key);
        }
    }
    
    if keys.is_empty() {
        keys.push("default".to_string());
    }
    
    keys
});

pub fn get_youtube_api_keys() -> Vec<String> {
    YOUTUBE_API_KEYS.clone()
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(flatten)]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status: "success".to_string(),
            message: None,
            data: Some(data),
        }
    }

    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            status: "error".to_string(),
            message: Some(message),
            data: None,
        }
    }
}
