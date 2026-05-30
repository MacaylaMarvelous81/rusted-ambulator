use std::collections::HashMap;

pub struct Session {
    pub steam_name: String,
    pub passcode: String,
    pub hands: HashMap<String, Hand>
}

pub struct Hand {
}

impl Session {
    pub fn new(steam_name: String, passcode: String) -> Self {
        Session {
            steam_name,
            passcode,
            hands: HashMap::new()
        }
    }
}