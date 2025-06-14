# MelodySyncer.rs API

A REST API built with Rust and deployed on Vercel that repeats a given string N times.

## API Endpoint

### POST /repeat

Accepts JSON input and returns an array of the repeated string.

**Request Body:**
```json
{
  "text": "Hello World",
  "count": 3
}
```

**Response:**
```json
{
  "result": ["Hello World", "Hello World", "Hello World"],
  "original_text": "Hello World",
  "count": 3
}
```

### GET /repeat

Accepts query parameters and returns an array of the repeated string.

**Query Parameters:**
- `text`: The string to repeat (URL encoded)
- `count`: The number of times to repeat (max 1000)

**Example:**
```
GET /repeat?text=Hello%20World&count=3
```

**Response:**
```json
{
  "result": ["Hello World", "Hello World", "Hello World"],
  "original_text": "Hello World",
  "count": 3
}
```

## Local Development

1. Install Rust and Vercel CLI
2. Run `vercel dev` to start local development server
3. Test the API at `http://localhost:3000/repeat`

## Deployment

1. Run `vercel` to deploy to Vercel
2. The API will be available at your Vercel domain

## Limits

- Maximum count: 1000 (to prevent abuse)
- Supports both GET and POST methods
- Proper error handling and validation 