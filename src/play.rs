use crate::AppState;
use crate::session::{HandObject, PlayerColor};
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::mem;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc, oneshot};

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
    Error,
}

#[derive(Clone)]
pub enum PlayUpdate {
    HandUpdate([Vec<HandObject>; PlayerColor::COUNT]),
}

// TODO: Use in OutgoingPlayMessage::Error, then remove allow(dead_code)
#[allow(dead_code)]
pub enum Error {
    BadJson(serde_json::Error),
    Closed,
    InvalidSession(String),
    InvalidColor,
}

struct PlayState {
    session: Option<String>,
    color: PlayerColor,
    update_cancel_tx: oneshot::Sender<()>,
}
impl PlayState {
    pub fn new(update_cancel_tx: oneshot::Sender<()>) -> Self {
        Self {
            session: Default::default(),
            color: Default::default(),
            update_cancel_tx,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::BadJson(value)
    }
}

impl From<mpsc::error::SendError<OutgoingPlayMessage>> for Error {
    fn from(_: mpsc::error::SendError<OutgoingPlayMessage>) -> Self {
        Self::Closed
    }
}

pub async fn handle_play(socket: WebSocket, app_state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let (sender_tx, mut sender_rx) = mpsc::channel(2);

    let (update_cancel_tx, _) = oneshot::channel();
    let state = Arc::new(Mutex::new(PlayState::new(update_cancel_tx)));

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
        tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                let Ok(Message::Text(text)) = msg else {
                    continue;
                };

                match serde_json::from_str(text.as_str()) {
                    Ok(msg) => {
                        let result =
                            handle_play_message(msg, sender_tx.clone(), &state, &app_state).await;
                        match result {
                            Ok(_) => (),
                            Err(Error::Closed) => {
                                eprintln!("Failed to send play message as the channel closed.");
                                break;
                            }
                            Err(_) => {
                                let result = sender_tx.send(OutgoingPlayMessage::Error).await;
                                if let Err(err) = result {
                                    eprintln!(
                                        "Failed to send play message as the channel closed: {}",
                                        err
                                    );
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // TODO: include error details
                        let result = sender_tx.send(OutgoingPlayMessage::Error).await;
                        if let Err(err) = result {
                            eprintln!("Failed to send play message as the channel closed: {}", err);
                            break;
                        }
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

async fn handle_play_message(
    message: IncomingPlayMessage,
    sender_tx: mpsc::Sender<OutgoingPlayMessage>,
    state: &Arc<Mutex<PlayState>>,
    app_state: &Arc<AppState>,
) -> Result<(), Error> {
    match message {
        IncomingPlayMessage::Initialize { id } => {
            let (colors, update_rx) = {
                let sessions = app_state.sessions.read().unwrap();
                let session = sessions.get(&id).map(|session| session.lock().unwrap());
                // The Option is unwrapped with let else here instead of ok_or and propagating
                // because the error moves id which would be used later, and the compiler does not
                // reason about propagation returning early.
                let Some(session) = session else {
                    return Err(Error::InvalidSession(id));
                };

                let colors: Vec<String> = session
                    .seats
                    .iter()
                    .enumerate()
                    .filter(|(_, hand)| !hand.is_empty())
                    .flat_map(|(index, _)| PlayerColor::try_from(index).ok())
                    .map(|color| String::from(color.as_ref()))
                    .collect();
                let update_rx = session.update_tx.subscribe();

                (colors, update_rx)
            };

            let update_cancel_rx = {
                let mut state = state.lock().unwrap();
                let (update_cancel_tx, update_cancel_rx) = oneshot::channel();
                let _ = mem::replace(&mut state.update_cancel_tx, update_cancel_tx).send(());
                state.session = Some(id);

                update_cancel_rx
            };
            {
                let sender_tx = sender_tx.clone();
                let state = state.clone();

                tokio::spawn(async move {
                    handle_update(update_rx, update_cancel_rx, sender_tx, state).await;
                });
            }

            sender_tx
                .send(OutgoingPlayMessage::Initialize { colors })
                .await
                .map_err(Error::from)
        }
        IncomingPlayMessage::Color(color) => {
            let hand = {
                let mut state = state.lock().unwrap();
                state.color =
                    PlayerColor::try_from(color.as_str()).map_err(|_| Error::InvalidColor)?;
                let name = state
                    .session
                    .as_ref()
                    .ok_or_else(|| Error::InvalidSession(String::default()))?;
                let sessions = app_state.sessions.read().unwrap();
                let session = sessions
                    .get(name)
                    .ok_or(Error::InvalidSession(name.clone()))?;

                session.lock().unwrap().seats[&state.color].clone()
            };

            sender_tx
                .send(OutgoingPlayMessage::Hand(hand))
                .await
                .map_err(Error::from)
        }
    }
}

async fn handle_update(
    mut update_rx: broadcast::Receiver<PlayUpdate>,
    mut cancel_rx: oneshot::Receiver<()>,
    sender_tx: mpsc::Sender<OutgoingPlayMessage>,
    state: Arc<Mutex<PlayState>>,
) {
    loop {
        tokio::select! {
            update = update_rx.recv() => match update {
                Ok(PlayUpdate::HandUpdate(hands)) => {
                    let colors: Vec<String> = hands.iter().enumerate()
                        .filter(|(_, hand)| !hand.is_empty())
                        .flat_map(|(index, _)| PlayerColor::try_from(index).ok())
                        .map(|color| String::from(color.as_ref()))
                        .collect();
                    let _ = sender_tx
                        .send(OutgoingPlayMessage::Initialize {
                            colors,
                        })
                        .await;
                    let hand = {
                        let color = &state.lock().unwrap().color;
                        hands[color].to_owned()
                    };
                    let _ = sender_tx.send(OutgoingPlayMessage::Hand(hand)).await;
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            },
            _ = &mut cancel_rx => break,
        }
    }
}
