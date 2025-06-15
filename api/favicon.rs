use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

const FAVICON_DATA: &[u8] = include_bytes!("../public/favicon.ico");

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(_req: Request) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/x-icon")
        .header("Cache-Control", "public, max-age=86400") // 24 hour cache
        .body(FAVICON_DATA.to_vec().into())?)
}
