//! Complementary web service for Tabletop Simulator.
//!
//! This service is based on Tabletop Ambulator, which allows hosts on Tabletop Simulator to grant
//! access to players to access and manage the contents of their hands from without using the game
//! screen by using a web browser.

#![warn(missing_docs, missing_debug_implementations)]

mod app;
mod play;
mod session;
mod template;

use crate::app::AppState;
use crate::play::handle_play;
use crate::session::{HandObject, PlayerColor};
use crate::template::{IndexTemplate, SessionTemplate};
use askama::Template;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{StatusCode, header};
use axum::response::{ErrorResponse, Html, IntoResponse, Redirect, Response};
use axum::routing::{any, get, put};
use axum::{Json, Router};
use rust_embed::Embed;
use std::array;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Embed)]
#[folder = "assets/"]
struct EmbedAsset;

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

async fn serve_index() -> axum::response::Result<Html<String>> {
    Template::render(&IndexTemplate)
        .map(Html)
        .inspect_err(|e| eprintln!("Template render error: {}", e))
        .map_err(|_| ErrorResponse::from(StatusCode::INTERNAL_SERVER_ERROR))
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
    let rendered = state
        .with_session(id.as_str(), |session| {
            Template::render(&SessionTemplate {
                id: id.as_str(),
                session: &session,
            })
            .map(Html)
        })
        .ok_or(StatusCode::NOT_FOUND)?;
    rendered
        .inspect_err(|e| eprintln!("Template render error: {}", e))
        .map_err(|_| ErrorResponse::from(StatusCode::INTERNAL_SERVER_ERROR))
}

async fn create_session(
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    let name = query
        .get("name")
        .cloned()
        .unwrap_or("Unknown User".to_string());

    state.create_session(id, name);

    StatusCode::CREATED
}

async fn update_hands(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<HashMap<String, Vec<HandObject>>>,
) -> StatusCode {
    let hand = array::from_fn(|i| {
        let color = PlayerColor::try_from(i);
        let color = color.as_ref().map(AsRef::as_ref);
        if let Ok(color) = color {
            payload.remove(color).unwrap_or_else(Vec::new)
        } else {
            Vec::new()
        }
    });

    state
        .with_session(id.as_str(), |mut session| {
            session.update_hands(hand);
            StatusCode::NO_CONTENT
        })
        .unwrap_or(StatusCode::NOT_FOUND)
}

async fn upgrade_play(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_play(socket, state))
}
