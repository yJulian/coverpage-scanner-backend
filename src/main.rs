mod models;
mod ocr;
mod pipeline;
mod room_manager;

use axum::{
    extract::{DefaultBodyLimit, Multipart, State, ws::{WebSocket, WebSocketUpgrade, Message}},
    http::StatusCode,
    response::IntoResponse,
    routing::{post, get},
    Json, Router,
};
use crate::models::{ScanResponse, StudentInfo};
use crate::models::room::{UserInfo};
use crate::pipeline::{Context, ScannerPipeline};
use crate::pipeline::steps::{ImagePreProcessor, OcrScanner, QrCodeScanner};
use crate::ocr::{LocalOcrProvider, OcrProvider, MockOcrProvider};
use crate::room_manager::RoomManager;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;
use uuid::Uuid;
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;

struct AppState {
    ocr_provider: Arc<dyn OcrProvider>,
    room_manager: RoomManager,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize OCR provider once at startup
    let ocr_provider = match LocalOcrProvider::new("models") {
        Ok(p) => Arc::new(p) as Arc<dyn OcrProvider>,
        Err(e) => {
            tracing::error!("Failed to initialize OCR provider: {}. Using Mock for development.", e);
            Arc::new(MockOcrProvider { text: "Mock OCR Result".into() })
        }
    };

    let state = Arc::new(AppState {
        ocr_provider,
        room_manager: RoomManager::new(),
    });

    let app = Router::new()
        .route("/scan", post(scan_handler))
        .route("/rooms/create", post(create_room_handler))
        .route("/ws/join/{code}", get(ws_handler))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)); // 10MB limit

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Deserialize)]
struct CreateRoomRequest {
    user_name: String,
}

#[derive(serde::Serialize)]
struct CreateRoomResponse {
    code: String,
    user_id: Uuid,
}

async fn create_room_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateRoomRequest>,
) -> impl IntoResponse {
    // In a full implementation, the user would connect via WS right after this
    // We create a temporary dummy channel for the creation step
    let user_id = Uuid::new_v4();
    let (tx, _rx) = mpsc::unbounded_channel(); 
    let code = state.room_manager.create_room(UserInfo { id: user_id, name: payload.user_name }, tx);
    
    (StatusCode::CREATED, Json(CreateRoomResponse { code, user_id }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::Path(code): axum::extract::Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, code, state))
}

async fn handle_socket(socket: WebSocket, code: String, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let user_id = Uuid::new_v4();
    let user_info = UserInfo { id: user_id, name: format!("User-{}", &user_id.to_string()[..4]) };

    // Task to send messages from our channel to the WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if ws_sender.send(message).await.is_err() {
                break;
            }
        }
    });

    // Try to join the room
    if let Err(e) = state.room_manager.join_room(&code, user_info, tx) {
        tracing::error!("Join error: {}", e);
        return;
    }

    let code_clone = code.clone();
    
    // Task to receive messages from the WebSocket (can handle approval, chat, etc)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
            // Future: Handle PermissionResponse here
        }
    });

    // Wait for one of the tasks to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // Cleanup when disconnected
    state.room_manager.leave_room(&code_clone, user_id);
    tracing::info!("User {} left room {}", user_id, code_clone);
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
