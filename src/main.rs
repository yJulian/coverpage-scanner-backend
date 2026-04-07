mod models;
mod ocr;
mod pipeline;

use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use crate::models::{ScanResponse, StudentInfo};
use crate::pipeline::{Context, ScannerPipeline};
use crate::pipeline::steps::{ImagePreProcessor, OcrScanner, QrCodeScanner};
use crate::ocr::{LocalOcrProvider, OcrProvider};
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;

struct AppState {
    ocr_provider: Arc<dyn OcrProvider>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize provider once at startup
    let ocr_provider = match LocalOcrProvider::new("models") {
        Ok(p) => Arc::new(p) as Arc<dyn OcrProvider>,
        Err(e) => {
            tracing::error!("CRITICAL: Failed to initialize OCR provider: {}. Make sure 'models/text-detection.rten' and 'models/text-recognition.rten' are valid .rten files.", e);
            std::process::exit(1);
        }
    };

    let state = Arc::new(AppState { ocr_provider });

    let app = Router::new()
        .route("/scan", post(scan_handler))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)); // 10MB limit

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn scan_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart
) -> impl IntoResponse {
    let mut image_data = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "image" {
            if let Ok(data) = field.bytes().await {
                image_data = Some(data);
            }
            break;
        }
    }

    let Some(image_data) = image_data else {
        return (StatusCode::BAD_REQUEST, Json(ScanResponse::Error("No image provided".into()))).into_response();
    };

    let image = match image::load_from_memory(&image_data) {
        Ok(img) => img,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ScanResponse::Error(format!("Invalid image: {}", e)))).into_response(),
    };

    let pipeline = ScannerPipeline::new()
        .add_step(Box::new(ImagePreProcessor))
        .add_step(Box::new(QrCodeScanner))
        .add_step(Box::new(OcrScanner { provider: state.ocr_provider.clone() }));

    let context = Context::new(image);
    match pipeline.run(context) {
        Ok(ctx) => {
            if ctx.is_complete() {
                let info = StudentInfo {
                    first_name: ctx.partial_info.first_name.unwrap(),
                    last_name: ctx.partial_info.last_name.unwrap(),
                    matriculation_number: ctx.partial_info.matriculation_number.unwrap(),
                };
                (StatusCode::OK, Json(ScanResponse::Success(info))).into_response()
            } else {
                let missing = ctx.get_missing_fields();
                (StatusCode::OK, Json(ScanResponse::Partial {
                    info: ctx.partial_info,
                    missing,
                })).into_response()
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ScanResponse::Error(format!("Pipeline error: {}", e)))).into_response(),
    }
}
