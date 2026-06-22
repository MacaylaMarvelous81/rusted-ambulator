//! Internal app logic.

use crate::session::Session;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, RwLock};

/// Provider of the state that Tabletop Ambulator needs to keep track of.
///
/// Provides the app state, including runtime data such as the active game
/// sessions. It provides access to relevant data through synchronization
/// primitives, but does not handle shared ownership; for that, you may
/// want to use a solution like [`std::sync::Arc`].
#[derive(Debug, Default)]
pub struct AppState {
    sessions: RwLock<HashMap<String, Mutex<Session>>>,
}

impl AppState {
    /// Creates a new `AppState`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates and stores a new [`Session`], which can later be accessed with [`with_session`].
    pub fn create_session(&self, id: String, name: String) {
        let session = Mutex::new(Session::new(name));
        self.sessions.write().unwrap().insert(id, session);
    }

    /// Finds a [`Session`] by the provided `id`. If the session exists, it is provided within a
    /// [`MutexGuard`] to the supplied function. Returns the result of the function, or `None` if
    /// there was no session with the supplied id.
    pub fn with_session<T>(&self, id: &str, f: impl FnOnce(MutexGuard<Session>) -> T) -> Option<T> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(id).map(|session| f(session.lock().unwrap()))
    }
}
