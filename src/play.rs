use crate::AppState;
use crate::session::HandObject;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;

#[derive(Deserialize)]
enum IncomingPlayMessage {
    Initialize { id: String },
    Color(String),
}

#[derive(Serialize)]
enum OutgoingPlayMessage<'a> {
    Initialize { colors: Vec<&'a String> },
    Hand(Vec<&'a HandObject>),
}

struct PlayState {
    id: Option<String>,
}

pub async fn handle_play(mut socket: WebSocket, app_state: Arc<AppState>) {
    let mut play_state = PlayState { id: None };

    while let Some(msg) = socket.recv().await {
        let mut process = async |msg: Result<Message, axum::Error>,
                                 play_state: &mut PlayState|
               -> Result<(), Box<dyn Error>> {
            let msg: IncomingPlayMessage = serde_json::from_str(msg?.to_text()?)?;
            match msg {
                IncomingPlayMessage::Initialize { id } => {
                    // Blocked so that the guard is dropped after cloning the color names,
                    // preventing a potential deadlock when using .await after sending something
                    // through the socket, which would be possible if using tokio::sync::Mutex.
                    // Of course, the sessions HashMap is wrapped instd::sync types instead, which
                    // does not enable locking the mutex through .await anyway.
                    let colors: Vec<String> = {
                        let sessions = app_state.sessions.read().unwrap();
                        // TODO: Non-string Error might be useful
                        let session = sessions.get(&id).ok_or("Session did not exist")?;

                        session.hands.keys().cloned().collect()
                    };

                    play_state.id = Some(id);

                    let response = OutgoingPlayMessage::Initialize {
                        colors: colors.iter().collect(),
                    };
                    socket
                        .send(Message::Text(Utf8Bytes::from(serde_json::to_string(
                            &response,
                        )?)))
                        .await?;
                }
                IncomingPlayMessage::Color(color) => {
                    let hand: Vec<HandObject> = {
                        let sessions = app_state.sessions.read().unwrap();
                        let session = sessions
                            .get(play_state.id.as_ref().ok_or("No session was joined")?)
                            .ok_or("Session did not exist")?;

                        (*session
                            .hands
                            .get(&color)
                            .ok_or("No player seated by that color")?)
                        .clone()
                    };

                    let response = OutgoingPlayMessage::Hand(hand.iter().collect());
                    socket
                        .send(Message::Text(Utf8Bytes::from(serde_json::to_string(
                            &response,
                        )?)))
                        .await?;
                }
            }
            Ok(())
        };

        if let Ok(Message::Text(_)) = msg
            && let Err(err) = process(msg, &mut play_state).await
        {
            eprintln!(
                "Encountered an error while handling a message from a player: {}",
                err
            );
            break;
        }
    }
}
