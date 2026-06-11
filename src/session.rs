use crate::play::PlayUpdate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct Session {
    pub steam_name: String,
    pub seats: HashMap<String, Vec<HandObject>>,
    pub update_tx: broadcast::Sender<PlayUpdate>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum HandObject {
    CustomDeck(CustomDeck),
}

// TODO: These fields will be used in the future. When they are, the dead_code lint should no longer
//       be suppressed.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CustomDeck {
    /// The path/URL of the face cardsheet.
    pub face: String,
    /// The path/URL of the back cardsheet or card back.
    pub back: String,
    /// If each card has a unique card back (via a cardsheet).
    pub unique_back: bool,
    /// The number of columns on the cardsheet.
    pub width: f64,
    /// The number of rows on the cardsheet.
    pub height: f64,
    /// The number of cards on the cardsheet.
    pub number: f64,
    /// Whether the cards are horizontal, instead of vertical.
    pub sideways: bool,
    /// Whether the card back should be used as the hidden image (instead of the last slot of the
    /// `face` image).
    pub back_is_hidden: bool,
    /// ID of the custom card within the deck.
    pub card_id: f64,
}

impl Session {
    pub fn new(steam_name: String) -> Self {
        let (update_tx, _) = broadcast::channel(10);

        Session {
            steam_name,
            seats: HashMap::new(),
            update_tx,
        }
    }

    pub fn update_hands(&mut self, hands: HashMap<String, Vec<HandObject>>) {
        self.seats = hands.to_owned();
        // Updating the hand is a success regardless of whether there are players connected to
        // receive a hand update
        let _ = self.update_tx.send(PlayUpdate::HandUpdate(hands));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hand_update_transmit() {
        let mut session = Session::new("Sir Harold".to_string());
        let mut update_rx = session.update_tx.subscribe();

        let card = CustomDeck {
            face: "https://steamusercontent-a.akamaihd.net/ugc/1663479592506990057/B6EEB9A683A57C9A41CC9782993A8BAF9DCD72A1/".to_string(),
            back: "https://steamusercontent-a.akamaihd.net/ugc/1663479592507076702/D16FFBC8D87B4D4FB21C0057F2BBC9DC4D4FD379/".to_string(),
            unique_back: false,
            width: 5.0,
            height: 7.0,
            number: 6.0,
            sideways: false,
            back_is_hidden: false,
            card_id: 0.0,
        };

        let hands = HashMap::from([(
            "red".to_string(),
            vec![HandObject::CustomDeck(card.to_owned())],
        )]);
        session.update_hands(hands);

        // TODO: This lint allow be removed when PlayUpdate has more variants
        #[allow(irrefutable_let_patterns)]
        let PlayUpdate::HandUpdate(hand) = update_rx.recv().await.unwrap() else {
            panic!("Received update was not a HandUpdate");
        };

        assert_eq!(
            hand.get("red").unwrap().first().unwrap().to_owned(),
            HandObject::CustomDeck(card)
        );
    }
}
