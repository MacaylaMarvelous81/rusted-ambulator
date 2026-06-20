use crate::AppState;
use crate::session::{HandObject, PlayerColor, Session};
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock, Weak};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

#[derive(Deserialize)]
enum IncomingPlayMessage {
    Initialize { id: String },
    Color(String),
}

// TODO: Maybe derive Clone, reference interior vals
#[derive(Serialize)]
enum OutgoingPlayMessage {
    Initialize { colors: Vec<String> },
    Hand(Vec<HandObject>),
    // TODO: include error details
    Error,
}

#[derive(Clone)]
pub enum PlayUpdate {
    HandUpdate([Vec<HandObject>; PlayerColor::COUNT]),
}

pub async fn handle_play(socket: WebSocket, app_state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let (sender_tx, mut sender_rx) = mpsc::channel(2);

    let mut send_task = tokio::spawn(async move {
        while let Some(message) = sender_rx.recv().await {
            let serialized = match serde_json::to_string(&message) {
                Ok(serialized) => serialized,
                Err(err) => {
                    eprintln!("Failed to serialize outgoing websocket message: {}", err);
                    break;
                }
            };
            if let Err(err) = sender
                .send(Message::Text(Utf8Bytes::from(serialized)))
                .await
            {
                eprintln!("Failed to send serialized websocket message: {}", err);
                break;
            }
        }
    });

    let mut recv_task = {
        let sender_tx = sender_tx.clone();
        tokio::spawn(async move {
            let mut player_session = None;
            let player_color = Arc::new(RwLock::new(PlayerColor::Grey));

            while let Some(msg) = receiver.next().await {
                let Ok(Message::Text(text)) = msg else {
                    continue;
                };

                match serde_json::from_str(text.as_str()) {
                    Ok(IncomingPlayMessage::Initialize { id }) => {
                        let session = {
                            let sessions = app_state.sessions.read().unwrap();
                            sessions
                                .get(&id)
                                .map(Arc::clone)
                                .ok_or("Session did not exist")
                        };

                        match session {
                            Ok(session) => {
                                let (colors, update_rx) = {
                                    let session = session.lock().unwrap();

                                    let colors: Vec<String> = session.seats.iter().enumerate()
                                        .filter(|(_, hand)| hand.len() > 0)
                                        .flat_map(|(index, _)| PlayerColor::try_from(index).ok())
                                        .map(|color| String::from(color.as_ref()))
                                        .collect();
                                    let update_rx = session.update_tx.subscribe();

                                    (colors, update_rx)
                                };

                                player_session = Some(Arc::downgrade(&session));
                                {
                                    let sender_tx = sender_tx.clone();
                                    let player_session = Arc::downgrade(&session);
                                    let player_color = player_color.clone();

                                    tokio::spawn(async move {
                                        handle_update(
                                            update_rx,
                                            sender_tx,
                                            player_session,
                                            player_color,
                                        )
                                        .await
                                    });
                                }

                                let response = OutgoingPlayMessage::Initialize { colors };
                                if sender_tx.send(response).await.is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                eprintln!("Failed to access session: {}", err);
                                let response = OutgoingPlayMessage::Error;
                                if sender_tx.send(response).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(IncomingPlayMessage::Color(color)) => {
                        let Some(session) =
                            player_session.clone().and_then(|session| session.upgrade())
                        else {
                            let _ = sender_tx.send(OutgoingPlayMessage::Error).await;
                            break
                        };
                        let Ok(color) = PlayerColor::try_from(color.as_str()) else {
                            let _ = sender_tx.send(OutgoingPlayMessage::Error).await;
                            break
                        };

                        let hand = session.lock().unwrap().seats[&color].clone();
                        *player_color.write().unwrap() = color;
                        if sender_tx
                            .send(OutgoingPlayMessage::Hand(hand))
                            .await
                            .is_err()
                        {
                            break;
                        }
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
        })
    };

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

async fn handle_update(
    mut update_rx: Receiver<PlayUpdate>,
    sender_tx: Sender<OutgoingPlayMessage>,
    _player_session: Weak<Mutex<Session>>,
    player_color: Arc<RwLock<PlayerColor>>,
) {
    loop {
        match update_rx.recv().await {
            Ok(PlayUpdate::HandUpdate(hands)) => {
                let colors: Vec<String> = hands.iter().enumerate()
                    .filter(|(_, hand)| hand.len() > 0)
                    .flat_map(|(index, _)| PlayerColor::try_from(index).ok())
                    .map(|color| String::from(color.as_ref()))
                    .collect();
                let _ = sender_tx
                    .send(OutgoingPlayMessage::Initialize {
                        colors,
                    })
                    .await;
                let hand = {
                    let color = player_color.read().unwrap();
                    hands[usize::from(&*color)].to_owned()
                };
                let _ = sender_tx.send(OutgoingPlayMessage::Hand(hand)).await;
            }
            Err(RecvError::Closed) => break,
            Err(RecvError::Lagged(_)) => continue,
        }
    }
}
