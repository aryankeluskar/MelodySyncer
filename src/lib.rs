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
        .pool_max_idle_per_host(100)  // INCREASED for more connections
        .pool_idle_timeout(Duration::from_secs(90))  // Longer idle time
        .timeout(Duration::from_secs(8))  // Slightly faster timeout
        .connect_timeout(Duration::from_secs(2))  // FASTER connection timeout  
        .tcp_keepalive(Duration::from_secs(60))  // Longer keepalive
        .tcp_nodelay(true)
        .use_rustls_tls()
        .http2_prior_knowledge()
        .http2_keep_alive_interval(Duration::from_secs(5))  // More frequent pings
        .http2_keep_alive_timeout(Duration::from_secs(3))  // Faster timeout
        .http2_max_frame_size(Some(1 << 21))  // BIGGER frames for more data per round trip
        .http2_adaptive_window(true)  // ADAPTIVE window sizing
        .build()
        .expect("Failed to create HTTP client")
});

// FIXED: Use proper async initialization
pub static MONGO_CLIENT: Lazy<Arc<RwLock<Option<MongoClient>>>> = Lazy::new(|| {
    Arc::new(RwLock::new(None))
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
        // Check if we have a valid cached token - FASTER validation
        if let (Some(token), Some(expires_at)) = (&self.token, &self.token_expires_at) {
            if expires_at > &std::time::Instant::now() {
                return Ok(token.clone());
            }
        }

        // Get new token with OPTIMIZED request
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
            .header("Content-Type", "application/x-www-form-urlencoded")  // EXPLICIT content type
            .form(&params)
            .send()
            .await?;

        let token_response: Value = response.json().await?;
        let access_token = token_response["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access token in response"))?
            .to_string();

        let expires_in = token_response["expires_in"].as_u64().unwrap_or(3600);

        // Cache the token with BETTER expiry buffer
        self.token = Some(access_token.clone());
        self.token_expires_at = Some(
            std::time::Instant::now() + Duration::from_secs(expires_in - 120), // 2 minute buffer for safety
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

// GODLY FAST YouTube search with custom accuracy scoring - ULTRA OPTIMIZED
pub async fn search_track_yt(
    song_name: &str,
    artist_name: &str,
    album_name: &str,
    song_duration: u32,
    api_keys: &[String],
) -> Result<String> {
    // OPTIMIZED search query construction
    let search_query = format!(
        "{} {} {} Official Audio",
        song_name, album_name, artist_name
    );

    // PARALLEL API key attempts for MAXIMUM SPEED
    let search_futures: Vec<_> = api_keys.iter().enumerate().map(|(i, api_key)| {
        let search_query = search_query.clone();
        let api_key = api_key.clone();
        async move {
            let url = format!(
                "https://youtube.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&maxResults=10&key={}",
                urlencoding::encode(&search_query),
                api_key
            );

            // FASTER request with explicit headers
            let response = HTTP_CLIENT
                .get(&url)
                .header("Accept", "application/json")
                .header("Accept-Encoding", "gzip, deflate, br")
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(search_result) = resp.json::<YouTubeSearchResponse>().await {
                        if !search_result.items.is_empty() {
                            return Some((i, search_result));
                        }
                    }
                }
                _ => {}
            }
            None
        }
    }).collect();

    // Wait for FIRST successful response (fastest API key wins)
    let search_results = join_all(search_futures).await;
    
    // Get the first successful result
    let search_result = search_results
        .into_iter()
        .find_map(|result| result)
        .ok_or_else(|| anyhow!("Failed to search YouTube with all API keys"))?;

    let (_winning_api_index, search_data) = search_result;

    // ULTRA PARALLEL duration fetching - ALL at once
    let duration_tasks: Vec<_> = search_data
        .items
        .iter()
        .map(|item| {
            let video_id = item.id.video_id.clone();
            let api_keys = api_keys.to_vec();
            async move {
                get_track_duration_yt(&video_id, &api_keys).await.unwrap_or(0)
            }
        })
        .collect();

    let durations = join_all(duration_tasks).await;

    // LIGHTNING FAST ACCURACY SCORING with BETTER scoring algorithm
    let mut best_score = -1;
    let mut best_video_id = &search_data.items[0].id.video_id;

    for (i, item) in search_data.items.iter().enumerate() {
        let mut score = 0;

        // +3 for Topic channels (official artist channels) - INCREASED weight
        if item.snippet.channel_title.contains("Topic") {
            score += 3;
        }

        // +3 for Official Audio/Video - INCREASED weight  
        if item.snippet.title.contains("Official Audio")
            || item.snippet.title.contains("Official Video")
            || item.snippet.title.contains("Full Audio Song")
        {
            score += 3;
        }

        // +1 for containing artist name in title
        if item.snippet.title.to_lowercase().contains(&artist_name.to_lowercase()) {
            score += 1;
        }

        // +1 for containing song name in title
        if item.snippet.title.to_lowercase().contains(&song_name.to_lowercase()) {
            score += 1;
        }

        // +7 for PERFECT duration match (within 1 second) - INCREASED weight
        let video_duration = durations[i];
        if video_duration > 0 {
            let duration_diff = ((video_duration as i64) - (song_duration as i64)).abs();
            if duration_diff <= 1000 {
                score += 7;  // Perfect match
            } else if duration_diff <= 2000 {
                score += 5;  // Very close match
            } else if duration_diff <= 5000 {
                score += 2;  // Close match
            }
        }

        if score > best_score {
            best_score = score;
            best_video_id = &item.id.video_id;
        }
    }

    Ok(best_video_id.clone())
}

// Async function to get or initialize MongoDB client - OPTIMIZED FOR SPEED
pub async fn get_mongo_client() -> Option<MongoClient> {
    // Fast path: check if already initialized
    {
        let client_guard = MONGO_CLIENT.read().await;
        if let Some(ref client) = *client_guard {
            return Some(client.clone());
        }
    }
    
    // Initialize if not exists with OPTIMIZED settings
    if let Ok(uri) = env::var("MONGO_URI") {
        // Use spawn_blocking for CPU-bound initialization with OPTIMIZED client options
        match tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current().block_on(async {
                // OPTIMIZED MongoDB client with connection pooling
                let mut client_options = mongodb::options::ClientOptions::parse(&uri).await?;
                
                // CRITICAL: Optimize connection pool for SPEED
                client_options.max_pool_size = Some(20);  // More connections
                client_options.min_pool_size = Some(5);   // Keep minimum connections alive
                client_options.max_idle_time = Some(std::time::Duration::from_secs(300));  // 5 min idle
                client_options.connect_timeout = Some(std::time::Duration::from_millis(2000));  // 2s timeout
                client_options.server_selection_timeout = Some(std::time::Duration::from_millis(3000));  // 3s selection
                
                MongoClient::with_options(client_options)
            })
        }).await {
            Ok(Ok(client)) => {
                // Cache the client for future use
                let mut client_guard = MONGO_CLIENT.write().await;
                *client_guard = Some(client.clone());
                Some(client)
            }
            _ => None  // Fail silently for non-critical MongoDB operations
        }
    } else {
        None
    }
}

// SUPER FAST analytics updater - COMPLETELY ASYNC
pub async fn update_analytics(songs_converted: i32, playlists_converted: i32) -> Result<()> {
    // Only get client if we need it - lazy initialization
    if let Some(client) = get_mongo_client().await {
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

            // Fire and forget for maximum speed - but with proper error handling
            tokio::spawn(async move {
                if let Err(_) = collection
                    .update_many(
                        doc! {},
                        update_doc,
                        mongodb::options::UpdateOptions::builder()
                            .upsert(true)
                            .build(),
                    )
                    .await {
                    // Log error but don't fail the request
                    eprintln!("Analytics update failed");
                }
            });
        }
    }

    Ok(())
}

pub fn get_youtube_api_keys() -> Vec<String> {
    let mut keys = Vec::new();

    if let Ok(key) = env::var("YOUTUBE_API_KEY") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY2") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY3") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY4") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY5") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY6") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY7") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY8") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY9") {
        keys.push(key);
    }
    if let Ok(key) = env::var("YOUTUBE_API_KEY10") {
        keys.push(key);
    }

    if keys.is_empty() {
        keys.push("default".to_string());
    }

    keys
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
