use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(_req: Request) -> Result<Response<Body>, Error> {
    let help_content = r#"
🎵 MelodySyncer - The Fastest Spotify to YouTube API Ever (Rust Edition)

📚 API Documentation:

🚀 Endpoints:
- GET /song?query={spotify_song_id} - Convert a single Spotify song to YouTube
- GET /playlist?query={spotify_playlist_id} - Convert entire playlist to YouTube URLs
- GET /analytics - Get usage statistics
- GET /help - This help page
- GET/POST /repeat - Utility endpoint to repeat text multiple times

⚡ Features:
- Lightning fast Rust implementation
- Connection pooling for maximum speed
- Token caching for Spotify API
- Multiple YouTube API key support with failover
- Parallel processing for playlists
- Custom accuracy scoring algorithm

🔑 Authentication:
You can provide YouTube API key in two ways:
1. Header: X-YouTube-API-Key: YOUR_API_KEY
2. Query param: ?youtubeAPIKEY=YOUR_API_KEY

If no API key provided, server uses default (limited quota).

📖 Usage Examples:
- Single song: /song?query=58ge6dfP91o9oXMzq3XkIS
- Playlist: /playlist?query=7fITt66rmO4QIeNs2LPRDj
- With API key: /song?query=SONG_ID&youtubeAPIKEY=YOUR_KEY

🎯 Accuracy Algorithm:
- +2 points: Channel has "Topic" (official artist channels)
- +2 points: Title contains "Official Audio"
- +5 points: Duration matches within 2 seconds

💻 GitHub: https://github.com/aryankeluskar/MelodySyncer
📚 Full docs: /docs

Built with ❤️ and 🦀 Rust for maximum performance!
"#;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("Cache-Control", "public, max-age=3600")
        .body(help_content.into())?)
}
