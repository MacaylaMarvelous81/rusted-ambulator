use crate::session::Session;
use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

#[derive(Template)]
#[template(path = "session.html")]
pub struct SessionTemplate<'a> {
    pub id: &'a String,
    pub session: &'a Session,
}
