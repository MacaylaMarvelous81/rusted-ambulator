use crate::AppState;
use crate::session::HandObject;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;

macro_rules! send_message_or_break {
    ($socket:expr, $message:expr) => {{
        let result = send_outgoing_message($socket, $message).await;
        if let Err(err) = result {
            eprintln!("Failed to send message to socket: {}", err);
            break;
        }
    }};
}

#[derive(Deserialize)]
enum IncomingPlayMessage {
    Initialize { id: String },
    Color(String),
}

#[derive(Serialize)]
enum OutgoingPlayMessage<'a> {
    Initialize { colors: Vec<&'a String> },
    Hand(Vec<&'a HandObject>),
    // TODO: include error details
    Error,
}

async fn send_outgoing_message(
    socket: &mut WebSocket,
    message: &OutgoingPlayMessage<'_>,
) -> Result<(), Box<dyn Error>> {
    let serialized = serde_json::to_string(message)?;
    socket
        .send(Message::Text(Utf8Bytes::from(serialized)))
        .await
        .map_err(Box::from)
}

pub async fn handle_play(mut socket: WebSocket, app_state: Arc<AppState>) {
    let mut player_session = None;

    while let Some(msg) = socket.recv().await {
        let Ok(Message::Text(text)) = msg else {
            continue;
        };

        match serde_json::from_str(text.as_str()) {
            Ok(IncomingPlayMessage::Initialize { id }) => {
                let session = {
                    let sessions = app_state.sessions.read().unwrap();
                    sessions
                        .get(&id)
                        .map(Arc::to_owned)
                        .ok_or("Session did not exist")
                };

                match session {
                    Ok(session) => {
                        let colors: Vec<String> = session
                            .lock()
                            .unwrap()
                            .seats
                            .keys()
                            .map(String::to_owned)
                            .collect();
                        player_session = Some(Arc::downgrade(&session));
                        let response = OutgoingPlayMessage::Initialize {
                            colors: colors.iter().collect(),
                        };
                        send_message_or_break!(&mut socket, &response);
                    }
                    Err(err) => {
                        eprintln!("Failed to access session: {}", err);
                        let response = OutgoingPlayMessage::Error;
                        send_message_or_break!(&mut socket, &response);
                    }
                }
            }
            Ok(IncomingPlayMessage::Color(color)) => {
                let Some(session) = player_session.clone().and_then(|session| session.upgrade())
                else {
                    let response = OutgoingPlayMessage::Error;
                    send_message_or_break!(&mut socket, &response);
                    break;
                };
                let hand = session
                    .lock()
                    .unwrap()
                    .seats
                    .get(&color)
                    .map(|seat| (&seat.hand).to_owned());
                match hand {
                    Some(hand) => {
                        // Response constructed here because the inner value of the Option would be dropped outside the match block
                        let response = OutgoingPlayMessage::Hand(hand.iter().collect());
                        send_message_or_break!(&mut socket, &response);
                    }
                    None => {
                        let response = OutgoingPlayMessage::Error;
                        send_message_or_break!(&mut socket, &response);
                    }
                };
            }
            Err(err) => {
                eprintln!(
                    "Encountered an error while handling a message from a player: {}",
                    err
                );
                break;
            }
        }
    }
}
