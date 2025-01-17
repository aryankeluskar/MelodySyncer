# MelodySyncer - Most Accurate (and Fastest) Spotify to Youtube API Ever!
Built using Python, FastAPI, hosted on Render.com <br> MelodySyncer, or MeSo for short, is a Simple-to-use **Web API** to convert Spotify songs or playlists to their Youtube equivalent. Most Accurate since I have developed an unique scoring system that minimizes searching credits and maximises accuracy by factoring in Artist Name, Song Length, Album Name, Channel Source, etc. Takes only **0.2 seconds** per song! 
#### You get: _Skip manual searching, Directly get a List, Peace of Mind_ <br> I get _(hopefully): Star, Heart, Follow :)_

## Install (Only if local instance needed)
    Requirements: gh, pip, python <= 3.8
    Run the following commands:
    gh repo clone aryankeluskar/MelodySyncer
    pip install -r requirements.txt
  Also add the required details in .env


## Run the app (Only if local instance needed)
    uvicorn src.index:app --reload
Open your broswer at [http://localhost:8000](http://localhost:8000)

## Usage
Visit [dub.sh/melodysyncer](https://dub.sh/melodysyncer) to access this API. Check out detailed API documentation [here](https://melodysyncer.aryankeluskar.com/docs) generated with OpenAPI <br>

**Example**: `https://melodysyncer.aryankeluskar.com/playlist?query=7fITt66rmO4QIeNs2LPRDj&youtubeAPIKEY=default` responds with [a list of YouTube Links](## "can't reveal links in README for copyright purposes") which can be processed with HttpRequest in Java, or requests.get in Python. This data can be stored in an Array or List for further processing.

### GET /
    Parameters: None 
    Response: (string) Displays the valid paths available, and frontend in the future
    
<hr>

### GET /song
    Parameters: query (string) ID of the song in Spotify, youtubeAPIKEY (string) Google Cloud API Key with YouTube Data v3 enabled.
    Response: (string) Accurate Youtube ID of the song, neglecting any remix, cover, and music videos
    
<hr>

### GET /playlist
    Parameters: query (string) ID of the song in Spotify, youtubeAPIKEY (string) Google Cloud API Key with YouTube Data v3 enabled.
    Response: (list of str) List / Array of Strings, each element contains the Youtube URL for the song. The indices remain same from Spotify Playlist
    
## Support this project!
### This is my second ever API! Please consider leaving a 🌟 if this added value to your wonderful project
### Made with pure love and [freshman fascination](## "it's a real term i swear"). Visit my website at [aryankeluskar.github.io](https://aryankeluskar.github.io) <3
