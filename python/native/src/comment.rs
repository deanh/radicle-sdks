//! Shared collaboration comment snapshot; timestamps are milliseconds.
use pyo3::prelude::*;
use radicle::cob::thread;
use radicle::identity::Did;

#[pyclass(name = "Comment", module = "radiclepy", get_all, frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub timestamp: u64,
    pub reply_to: Option<String>,
}

#[pymethods]
impl Comment {
    fn __hash__(&self) -> u64 {
        crate::values::snapshot_hash(self)
    }

    fn __repr__(&self) -> String {
        format!("Comment({}, {})", self.id, self.author)
    }
}

pub fn comment<L>(id: &thread::CommentId, c: &thread::Comment<L>) -> Comment {
    Comment {
        id: id.to_string(),
        author: Did::from(*c.author()).to_string(),
        body: c.body().to_owned(),
        timestamp: c.timestamp().as_millis(),
        reply_to: c.reply_to().map(|id| id.to_string()),
    }
}
