use serde::Deserialize;
use std::collections::HashMap;

pub struct Session {
    pub steam_name: String,
    pub passcode: String,
    pub hands: HashMap<String, Vec<HandObject>>,
}

#[derive(Deserialize)]
pub enum HandObject {
    CustomDeck(CustomDeck),
}

#[derive(Deserialize)]
pub struct CustomDeck {
    /// The path/URL of the face cardsheet.
    face: String,
    /// The path/URL of the back cardsheet or card back.
    back: String,
    /// If each card has a unique card back (via a cardsheet).
    unique_back: bool,
    /// The number of columns on the cardsheet.
    width: f64,
    /// The number of rows on the cardsheet.
    height: f64,
    /// The number of cards on the cardsheet.
    number: f64,
    /// Whether the cards are horizontal, instead of vertical.
    sideways: bool,
    /// Whether the card back should be used as the hidden image (instead of the last slot of the
    /// `face` image).
    back_is_hidden: bool,
}

impl Session {
    pub fn new(steam_name: String, passcode: String) -> Self {
        Session {
            steam_name,
            passcode,
            hands: HashMap::new(),
        }
    }
}
