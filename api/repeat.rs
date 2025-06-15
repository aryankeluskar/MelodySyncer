use serde::{Deserialize, Serialize};
use serde_json::json;
use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

#[derive(Deserialize)]
struct RepeatRequest {
    text: String,
    count: u32,
}

#[derive(Serialize)]
struct RepeatResponse {
    result: Vec<String>,
    original_text: String,
    count: u32,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(req: Request) -> Result<Response<Body>, Error> {
    match req.method().as_str() {
        "POST" => handle_post(req).await,
        "GET" => handle_get(req).await,
        _ => Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "error": "Method not allowed. Use POST with JSON body or GET with query parameters."
                })
                .to_string()
                .into(),
            )?),
    }
}

async fn handle_post(req: Request) -> Result<Response<Body>, Error> {
    let body = req.body();
    let body_str = std::str::from_utf8(body).map_err(|_| {
        Error::from("Invalid UTF-8 in request body")
    })?;

    let request_data: RepeatRequest = serde_json::from_str(body_str).map_err(|_| {
        Error::from("Invalid JSON in request body. Expected: {\"text\": \"string\", \"count\": number}")
    })?;

    // Validate count is reasonable (prevent abuse)
    if request_data.count > 100000 {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "error": "Count must be 100000 or less"
                })
                .to_string()
                .into(),
            )?);
    }

    let repeated_strings: Vec<String> = (0..request_data.count)
        .map(|_| request_data.text.clone())
        .collect();

    let response = RepeatResponse {
        result: repeated_strings,
        original_text: request_data.text,
        count: request_data.count,
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&response)?.into())?)
}

async fn handle_get(req: Request) -> Result<Response<Body>, Error> {
    let uri = req.uri();
    let query = uri.query().unwrap_or("");
    
    // Parse query parameters manually
    let mut text: Option<String> = None;
    let mut count: Option<u32> = None;
    
    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "text" => {
                    // URL decode the text parameter
                    text = Some(urlencoding::decode(value).map_err(|_| Error::from("Failed to decode text parameter"))?.into_owned());
                },
                "count" => {
                    count = value.parse().ok();
                },
                _ => {}
            }
        }
    }

    let text = text.ok_or_else(|| Error::from("Missing 'text' query parameter"))?;
    let count = count.ok_or_else(|| Error::from("Missing or invalid 'count' query parameter"))?;

    // Validate count is reasonable (prevent abuse)
    if count > 100000 {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "error": "Count must be 100000 or less"
                })
                .to_string()
                .into(),
            )?);
    }

    let repeated_strings: Vec<String> = (0..count)
        .map(|_| text.clone())
        .collect();

    let response = RepeatResponse {
        result: repeated_strings,
        original_text: text,
        count,
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&response)?.into())?)
} 