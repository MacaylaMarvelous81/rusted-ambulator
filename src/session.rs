use std::array;
use crate::play::PlayUpdate;
use serde::{Deserialize, Serialize};
use std::ops::Index;
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct Session {
    pub steam_name: String,
    pub seats: [Vec<HandObject>; PlayerColor::COUNT],
    pub update_tx: broadcast::Sender<PlayUpdate>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HandObject {
    CustomDeck(CustomDeck),
}

// TODO: These fields will be used in the future. When they are, the dead_code lint should no longer
//       be suppressed.
/// Similar to the table defined at [Custom Deck](https://api.tabletopsimulator.com/custom-game-objects/#custom-deck)
/// in the Tabletop Simulator API knowledge base, but `card_id` is used to identify the card number
/// within the deck.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CustomDeck {
    /// The path/URL of the face cardsheet.
    pub face: String,
    /// The path/URL of the back cardsheet or card back.
    pub back: String,
    /// If each card has a unique card back (via a cardsheet).
    pub unique_back: bool,
    /// The number of columns on the cardsheet.
    pub width: u64,
    /// The number of rows on the cardsheet.
    pub height: u64,
    /// The number of cards on the cardsheet.
    pub number: u64,
    /// Whether the cards are horizontal, instead of vertical.
    pub sideways: bool,
    /// Whether the card back should be used as the hidden image (instead of the last slot of the
    /// `face` image).
    pub back_is_hidden: bool,
    /// ID of the custom card within the deck, starting from 1.
    pub card_id: u64,
}

/// One of the colors defined by Tabletop Simulator as a player color. All players are assigned
/// one of these twelve player colors.
///
/// See [Player Colors](https://api.tabletopsimulator.com/player/colors/) from the Tabletop
/// Simulator API knowledge base.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlayerColor {
    White,
    Brown,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
    #[default]
    Grey,
    Black,
}

impl Session {
    pub fn new(steam_name: String) -> Self {
        let (update_tx, _) = broadcast::channel(10);

        Session {
            steam_name,
            seats: array::from_fn(|_| Vec::new()),
            update_tx,
        }
    }

    pub fn update_hands(&mut self, hands: [Vec<HandObject>; PlayerColor::COUNT]) {
        self.seats = hands.to_owned();
        // Updating the hand is a success regardless of whether there are players connected to
        // receive a hand update
        let _ = self.update_tx.send(PlayUpdate::HandUpdate(hands));
    }
}

impl PlayerColor {
    pub const COUNT: usize = 12;
}

impl AsRef<str> for PlayerColor {
    fn as_ref(&self) -> &str {
        match self {
            PlayerColor::White => "White",
            PlayerColor::Brown => "Brown",
            PlayerColor::Red => "Red",
            PlayerColor::Orange => "Orange",
            PlayerColor::Yellow => "Yellow",
            PlayerColor::Green => "Green",
            PlayerColor::Teal => "Teal",
            PlayerColor::Blue => "Blue",
            PlayerColor::Purple => "Purple",
            PlayerColor::Pink => "Pink",
            PlayerColor::Grey => "Grey",
            PlayerColor::Black => "Black",
        }
    }
}

impl From<&PlayerColor> for usize {
    fn from(value: &PlayerColor) -> Self {
        match value {
            PlayerColor::White => 0,
            PlayerColor::Brown => 1,
            PlayerColor::Red => 2,
            PlayerColor::Orange => 3,
            PlayerColor::Yellow => 4,
            PlayerColor::Green => 5,
            PlayerColor::Teal => 6,
            PlayerColor::Blue => 7,
            PlayerColor::Purple => 8,
            PlayerColor::Pink => 9,
            PlayerColor::Grey => 10,
            PlayerColor::Black => 11,
        }
    }
}

impl<T> Index<&PlayerColor> for [T] {
    type Output = T;

    fn index(&self, index: &PlayerColor) -> &Self::Output {
        &self[usize::from(index)]
    }
}

impl TryFrom<&str> for PlayerColor {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "White" => Ok(Self::White),
            "Brown" => Ok(Self::Brown),
            "Red" => Ok(Self::Red),
            "Orange" => Ok(Self::Orange),
            "Yellow" => Ok(Self::Yellow),
            "Green" => Ok(Self::Green),
            "Teal" => Ok(Self::Teal),
            "Blue" => Ok(Self::Blue),
            "Purple" => Ok(Self::Purple),
            "Pink" => Ok(Self::Pink),
            "Grey" => Ok(Self::Grey),
            "Black" => Ok(Self::Black),
            _ => Err(())
        }
    }
}

impl TryFrom<usize> for PlayerColor {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::White),
            1 => Ok(Self::Brown),
            2 => Ok(Self::Red),
            3 => Ok(Self::Orange),
            4 => Ok(Self::Yellow),
            5 => Ok(Self::Green),
            6 => Ok(Self::Teal),
            7 => Ok(Self::Blue),
            8 => Ok(Self::Purple),
            9 => Ok(Self::Pink),
            10 => Ok(Self::Grey),
            11 => Ok(Self::Black),
            _ => Err(())
        }
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
            width: 5,
            height: 7,
            number: 6,
            sideways: false,
            back_is_hidden: false,
            card_id: 1,
        };

        let mut hands: [Vec<HandObject>; PlayerColor::COUNT] = array::from_fn(|_| Vec::new());
        hands[PlayerColor::Red as usize].push(HandObject::CustomDeck(card.to_owned()));

        session.update_hands(hands);

        let PlayUpdate::HandUpdate(hand) = update_rx.recv().await.unwrap();

        assert_eq!(
            hand[PlayerColor::Red as usize].first().unwrap().to_owned(),
            HandObject::CustomDeck(card)
        );
    }
}
