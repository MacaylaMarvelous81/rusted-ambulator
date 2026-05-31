use crate::session::Session;
use askama::Template;
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate<'a> {
    pub sessions: &'a HashMap<String, Session>,
}

#[derive(Template)]
#[template(path = "session.html")]
pub struct SessionTemplate<'a> {
    pub session: &'a Session,
}
