mod session;
mod template;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::Router;
use axum::routing::{get};
use rust_embed::Embed;
use crate::session::{Session};
use crate::template::IndexTemplate;

#[derive(Embed)]
#[folder = "assets/"]
struct Asset;

struct AppState {
    sessions: Mutex<HashMap<String, Session>>
}

impl AppState {
    fn new() -> Self {
        AppState {
            sessions: Mutex::new(HashMap::new())
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/dist/{*path}", get(serve_static))
        .route("/session/{id}", get(visit_session).put(create_session))
        .with_state(Arc::new(AppState::new()));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn serve_index(State(state): State<Arc<AppState>>) -> Response {
    let sessions = state.sessions.lock().unwrap();
    let template = IndexTemplate {
        sessions: &sessions
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Render error").into_response()
    }
}

async fn serve_static(Path(path): Path<String>) -> Response {
    match Asset::get(path.as_str()) {
        Some(content) => {
            let mime = match path.split('.').last() {
                Some("html") => "text/html",
                _ => "application/octet-stream"
            };
            ([(header::CONTENT_TYPE, mime)], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response()
    }
}

async fn visit_session(Path(id): Path<String>, Query(query): Query<HashMap<String, String>>, State(state): State<Arc<AppState>>) -> Response {
    let passcode = query.get("passcode");

    let sessions = state.sessions.lock().unwrap();

    match sessions.get(&id) {
        Some(session) => {
            if passcode.map(|passcode| passcode.as_str() == session.passcode).unwrap_or(false) {
                (StatusCode::OK, "hi").into_response()
            } else {
                (StatusCode::FORBIDDEN, "Incorrect session passcode").into_response()
            }
        },
        None => (StatusCode::NOT_FOUND, "Session does not exist").into_response()
    }
}

async fn create_session(Path(id): Path<String>, Query(query): Query<HashMap<String, String>>, State(state): State<Arc<AppState>>) -> Response {
    let name = query.get("name").map(|name| name.clone()).unwrap_or("Unknown".to_string());
    let passcode = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.subsec_nanos()).unwrap_or(675603000).to_string();

    let mut sessions = state.sessions.lock().unwrap();

    let session = Session {
        steam_name: name,
        passcode: passcode.clone()
    };
    sessions.insert(id, session);

    (StatusCode::CREATED, passcode).into_response()
}