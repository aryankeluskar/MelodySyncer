use melody_syncer_rust::{ApiResponse, get_mongo_client};
use mongodb::bson::Document;
use serde_json::Value;
use std::env;
use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(_req: Request) -> Result<Response<Body>, Error> {
    match get_analytics().await {
        Ok(analytics_data) => {
            let response = ApiResponse::success(analytics_data);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "public, max-age=60") // 1 minute cache
                .header("Access-Control-Allow-Origin", "*")  // CORS support
                .body(serde_json::to_string(&response)?.into())?)
        }
        Err(e) => {
            let error_response = ApiResponse::<()>::error(e.to_string());
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(serde_json::to_string(&error_response)?.into())?)
        }
    }
}

async fn get_analytics() -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let client = get_mongo_client().await
        .ok_or("MongoDB client not available")?;

    let db_name = env::var("MONGO_DB").map_err(|_| "MONGO_DB environment variable not set")?;
    let collection_name = env::var("MONGO_COLLECTION")
        .map_err(|_| "MONGO_COLLECTION environment variable not set")?;

    let db = client.database(&db_name);
    let collection: mongodb::Collection<Document> = db.collection(&collection_name);
    
    let mut cursor = collection.find(None, None).await?;
    let mut results = Vec::new();

    use futures::stream::StreamExt;
    while let Some(doc) = cursor.next().await {
        match doc {
            Ok(mut doc) => {
                // Remove the _id field for cleaner output
                doc.remove("_id");
                // Convert BSON Document to serde_json::Value
                let json_doc: Value = mongodb::bson::to_bson(&doc)
                    .map_err(|e| format!("BSON conversion error: {}", e))?
                    .into();
                results.push(json_doc);
            }
            Err(e) => return Err(e.into()),
        }
    }

    if results.is_empty() {
        Ok(serde_json::json!({
            "message": "No analytics data found",
            "data": []
        }))
    } else {
        Ok(serde_json::json!({
            "data": results
        }))
    }
}
