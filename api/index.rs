use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

const HTML_CONTENT: &str = include_str!("../public/index.html");

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(_req: Request) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "public, max-age=3600")
        .body(HTML_CONTENT.into())?)
}
