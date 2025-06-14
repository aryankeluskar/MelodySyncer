use serde_json::json;
use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(_req: Request) -> Result<Response<Body>, Error> {
    // Get the SECRET environment variable
    let secret = env::var("SECRET").unwrap_or_else(|_| "SECRET not found".to_string());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(
            json!({
                "secret": secret,
                "message": "Secret retrieved successfully"
            })
            .to_string()
            .into(),
        )?)
} 