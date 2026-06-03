mod play;
mod session;
mod template;

use crate::play::handle_play;
use crate::session::{HandObject, Session};
use crate::template::{IndexTemplate, SessionTemplate};
use askama::Template;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use rust_embed::Embed;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Embed)]
#[folder = "assets/"]
struct EmbedAsset;

struct AppState {
    sessions: Mutex<HashMap<String, Session>>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/dist/{*path}", get(serve_static))
        .route("/session/{id}", get(visit_session).put(create_session))
        .route("/session/{id}/hands", get(serve_hands).put(update_hands))
        .route("/session/{id}/play", any(upgrade_play))
        .with_state(Arc::new(AppState::new()));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn serve_template(template: impl Template) -> Response {
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            eprintln!("An error occurred while rendering a template: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "Render error").into_response()
        }
    }
}

async fn serve_index(State(state): State<Arc<AppState>>) -> Response {
    let sessions = state.sessions.lock().unwrap();
    serve_template(IndexTemplate {
        sessions: &sessions,
    })
}

async fn serve_static(Path(path): Path<String>) -> Response {
    match EmbedAsset::get(path.as_str()) {
        Some(content) => {
            let mime = match path.split('.').next_back() {
                Some("js") => "application/javascript",
                _ => "application/octet-stream",
            };
            ([(header::CONTENT_TYPE, mime)], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

async fn visit_session(
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let passcode = query.get("passcode");

    let sessions = state.sessions.lock().unwrap();

    match sessions.get(&id) {
        Some(session) => match passcode {
            Some(passcode) if passcode.as_str() == session.passcode => {
                serve_template(SessionTemplate { id: &id, session })
            }
            _ => (StatusCode::FORBIDDEN, "Incorrect session passcode").into_response(),
        },
        None => (StatusCode::NOT_FOUND, "Session does not exist").into_response(),
    }
}

async fn create_session(
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let name = query.get("name").cloned().unwrap_or("Unknown".to_string());
    let passcode = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(675603000)
        .to_string();

    let mut sessions = state.sessions.lock().unwrap();

    let session = Session::new(name, passcode.clone());
    sessions.insert(id, session);

    (StatusCode::CREATED, passcode).into_response()
}

async fn serve_hands(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    let sessions = state.sessions.lock().unwrap();

    match sessions.get(&id) {
        Some(session) => Json(session.hands.keys().collect::<Vec<_>>()).into_response(),
        None => (StatusCode::NOT_FOUND, "Session does not exist").into_response(),
    }
}

async fn update_hands(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HashMap<String, Vec<HandObject>>>,
) -> Response {
    let mut sessions = state.sessions.lock().unwrap();

    match sessions.get_mut(&id) {
        Some(session) => {
            session.hands = payload;
            (StatusCode::NO_CONTENT, ()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Session does not exist").into_response(),
    }
}

async fn upgrade_play(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_play(socket, state))
}
