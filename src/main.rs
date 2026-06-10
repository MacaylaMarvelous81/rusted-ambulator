mod play;
mod session;
mod template;

use crate::play::handle_play;
use crate::session::{HandObject, PlayUpdate, Seat, Session};
use crate::template::{IndexTemplate, SessionTemplate};
use askama::Template;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{any, get, put};
use axum::{Json, Router};
use rust_embed::Embed;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Embed)]
#[folder = "assets/"]
struct EmbedAsset;

struct AppState {
    sessions: RwLock<HashMap<String, Arc<Mutex<Session>>>>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/dist/{*path}", get(serve_static))
        .route("/find-session", get(find_session))
        .route("/session/{id}", get(visit_session).put(create_session))
        .route("/session/{id}/hands", put(update_hands))
        .route("/session/{id}/play", any(upgrade_play))
        .with_state(Arc::new(AppState::new()));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn serve_template(template: &impl Template) -> Result<Html<String>, &'static str> {
    template.render().map(Html).map_err(|err| {
        eprintln!("Template render error: {}", err);
        "Template render error"
    })
}

async fn serve_index() -> axum::response::Result<Html<String>> {
    let template = IndexTemplate;
    Ok(serve_template(&template)?)
}

async fn serve_static(Path(path): Path<String>) -> Response {
    match EmbedAsset::get(path.as_str()) {
        Some(content) => {
            let mime = match path.split('.').next_back() {
                Some("js") => "application/javascript",
                Some("css") => "text/css",
                _ => "application/octet-stream",
            };
            ([(header::CONTENT_TYPE, mime)], content.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn find_session(
    Query(query): Query<HashMap<String, String>>,
) -> axum::response::Result<Redirect> {
    let id = query.get("id").ok_or(StatusCode::NOT_FOUND)?;
    Ok(Redirect::to(format!("/session/{}", id).as_str()))
}

async fn visit_session(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> axum::response::Result<Html<String>> {
    let sessions = state.sessions.read().unwrap();
    let session = sessions
        .get(&id)
        .ok_or((StatusCode::NOT_FOUND, "Session does not exist"))?
        .lock()
        .unwrap();

    let template = SessionTemplate {
        id: &id,
        session: &session,
    };
    Ok(serve_template(&template)?)
}

async fn create_session(
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    let name = query.get("name").cloned().unwrap_or("Unknown".to_string());

    let mut sessions = state.sessions.write().unwrap();

    let session = Session::new(name);
    sessions.insert(id, Arc::new(Mutex::new(session)));

    StatusCode::CREATED
}

async fn update_hands(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HashMap<String, Vec<HandObject>>>,
) -> StatusCode {
    let mut sessions = state.sessions.write().unwrap();

    match sessions.get_mut(&id) {
        Some(session) => {
            let mut session = session.lock().unwrap();

            for (color, hand) in payload {
                let seat = session
                    .seats
                    .entry(color.to_owned())
                    .or_insert_with(|| Seat { hand: Vec::new() });

                seat.hand = hand.to_owned();
                let _ = session.update_tx.send(PlayUpdate::HandUpdate(color, hand));
            }
            StatusCode::NO_CONTENT
        }
        None => StatusCode::NOT_FOUND,
    }
}

async fn upgrade_play(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_play(socket, state))
}
