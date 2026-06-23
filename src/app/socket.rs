use crate::AppState;
use crate::session::{HandObject, PlayerColor};
use axum::extract::ws::{Message, Utf8Bytes};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::mem;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc, oneshot};

#[derive(Debug)]
pub enum Error {
    BadJson(serde_json::Error),
    Closed,
    InvalidSession(String),
    InvalidColor,
}

#[derive(Clone)]
pub enum PlayUpdate {
    HandUpdate([Vec<HandObject>; PlayerColor::COUNT]),
}

#[derive(Deserialize)]
enum IncomingMessage {
    Initialize { id: String },
    Color(String),
}

// TODO: Maybe derive Clone, reference interior vals
#[derive(Serialize)]
enum OutgoingMessage {
    Initialize { colors: Vec<String> },
    Hand(Vec<HandObject>),
    Error(String),
}

struct PlayState {
    session: Option<String>,
    color: PlayerColor,
    update_cancel_tx: oneshot::Sender<()>,
}

impl From<Error> for OutgoingMessage {
    fn from(value: Error) -> Self {
        let message = format!("{}", value);
        Self::Error(message)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            // TODO: messages for serde_json errors other than bad JSON, such as IO errors
            Error::BadJson(err) => write!(f, "received bad json: {}", err),
            Error::Closed => write!(f, "message channel was closed"),
            Error::InvalidSession(id) => write!(f, "session by id {} does not or no longer exists", id),
            Error::InvalidColor => write!(f, "a color was provided that is not a valid Tabletop Simulator color"),
        }
    }
}

impl From<mpsc::error::SendError<OutgoingMessage>> for Error {
    fn from(_: mpsc::error::SendError<OutgoingMessage>) -> Self {
        Self::Closed
    }
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

pub async fn handle_play<S, R>(mut sender: S, mut receiver: R, app_state: Arc<AppState>)
where
    S: Sink<Message, Error: Debug> + Unpin + Send + 'static,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin + Send + 'static,
{
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
                eprintln!("Failed to send serialized websocket message: {:?}", err);
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
                            Err(err) => {
                                let result = sender_tx.send(OutgoingMessage::from(err)).await;
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
                    Err(err) => {
                        // TODO: include error details
                        let result = sender_tx.send(OutgoingMessage::from(Error::BadJson(err))).await;
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
    message: IncomingMessage,
    sender_tx: mpsc::Sender<OutgoingMessage>,
    state: &Arc<Mutex<PlayState>>,
    app_state: &Arc<AppState>,
) -> Result<(), Error> {
    match message {
        IncomingMessage::Initialize { id } => {
            let data_opt = app_state.with_session(id.as_str(), |session| {
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
            });
            // let else used instead of propagating Option::ok_or_else because compiler wouldn't
            // know about early return when moving id
            let Some((colors, update_rx)) = data_opt else {
                return Err(Error::InvalidSession(id));
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
                .send(OutgoingMessage::Initialize { colors })
                .await
                .map_err(Error::from)
        }
        IncomingMessage::Color(color) => {
            let hand = {
                let mut state = state.lock().unwrap();
                state.color =
                    PlayerColor::try_from(color.as_str()).map_err(|_| Error::InvalidColor)?;

                let name = state
                    .session
                    .as_ref()
                    .ok_or_else(|| Error::InvalidSession(String::default()))?;

                app_state
                    .with_session(name.as_str(), |session| session.seats[&state.color].clone())
                    .ok_or_else(|| Error::InvalidSession(name.clone()))?
            };

            sender_tx
                .send(OutgoingMessage::Hand(hand))
                .await
                .map_err(Error::from)
        }
    }
}

async fn handle_update(
    mut update_rx: broadcast::Receiver<PlayUpdate>,
    mut cancel_rx: oneshot::Receiver<()>,
    sender_tx: mpsc::Sender<OutgoingMessage>,
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
                        .send(OutgoingMessage::Initialize {
                            colors,
                        })
                        .await;
                    let hand = {
                        let color = &state.lock().unwrap().color;
                        hands[color].to_owned()
                    };
                    let _ = sender_tx.send(OutgoingMessage::Hand(hand)).await;
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            },
            _ = &mut cancel_rx => break,
        }
    }
}
