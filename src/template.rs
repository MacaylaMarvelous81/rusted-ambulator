use askama::Template;
use crate::session::Session;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate<'a> {
    pub sessions: &'a Vec<Session>
}