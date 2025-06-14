# MelodySyncer - The Fastest Spotify to Youtube API Ever

MelodySyncer is an **API** to convert Spotify songs or playlists to their Youtube equivalent. It is also the **most accurate** since it uses a unique scoring system that minimizes searching credits and maximises accuracy by factoring in song metadata. Takes less than **100 milliseconds** per song! Built using 🦀 **Rust**, hosted on Vercel.
 
**You get:** Skip manual searching, Directly get a List, Peace of Mind <br> **I get (hopefully):** Star, Heart, Follow :)

## 💡 Usage
Visit [dub.sh/melodysyncer](https://dub.sh/melodysyncer) to test this API on our playground. Check out detailed documentation [here](https://melodysyncer.aryankeluskar.com/docs) generated with OpenAPI. The BASE_URL is `https://melodysyncer.aryankeluskar.com/`

**Example**: `https://melodysyncer.aryankeluskar.com/playlist?query=7fITt66rmO4QIeNs2LPRDj` responds with a list of YouTube Links in JSON format.

### GET /
    Parameters: None 
    Response: (html) API Home Page for testing.
    
<hr>

### GET /song
    Parameters: 
    - query (string): ID of the song in Spotify
    - X-YouTube-API-Key (header, optional): Google Cloud API Key with YouTube Data v3 enabled
    Response: (string) Accurate Youtube ID of the song, neglecting any remix, cover, and music videos
    
<hr>

### GET /playlist
    Parameters: 
    - query (string): ID of the playlist in Spotify
    - X-YouTube-API-Key (header, optional): Google Cloud API Key with YouTube Data v3 enabled
    Response: (list of str) List / Array of Strings, each element contains the Youtube URL for the song. The indices remain same from Spotify Playlist

## 🔑 API Key Usage
You can provide the YouTube API key in two ways:
1. As a header (Preferred): `X-YouTube-API-Key: YOUR_API_KEY`
2. As a query parameter: `?youtubeAPIKEY=YOUR_API_KEY`

If no API key is provided, the server will use its default API key which has a limited trial.

## ⬇️ Install & Run Locally
Requirements: gh, pip, python <= 3.8
```bash
    gh repo clone aryankeluskar/melodysyncer
    cd melodysyncer
    cargo install --path .
```
Add the required details in .env, then
```bash
    cargo build --release
    vercel dev
```

## 💬 Support this project!
This is my first API written in Rust! Please consider leaving a 🌟 if this added value to your wonderful project. Made with pure love and [freshman fascination](## "it's a real term i swear") by [aryankeluskar.com](https://aryankeluskar.com)