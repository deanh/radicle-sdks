//! Owned, immutable collaboration snapshots. Typed COB timestamps are
//! milliseconds (common::Timestamp), unlike RawOp's change timestamp seconds.
use pyo3::prelude::*;
use radicle::cob::{patch, thread};
use radicle::identity::Did;

#[pyclass(name = "Comment", module = "radiclepy", get_all, frozen)]
#[derive(Clone)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub timestamp: u64,
    pub reply_to: Option<String>,
}

#[pymethods]
impl Comment {
    fn __repr__(&self) -> String { format!("Comment({}, {})", self.id, self.author) }
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

#[pyclass(name = "Review", module = "radiclepy", get_all, frozen)]
#[derive(Clone)]
pub struct Review {
    pub id: String,
    pub author: String,
    pub verdict: Option<String>,
    pub summary: String,
    pub timestamp: u64,
}

#[pymethods]
impl Review {
    fn __repr__(&self) -> String { format!("Review({}, {}, {:?})", self.id, self.author, self.verdict) }
}

#[pyclass(name = "Revision", module = "radiclepy", get_all, frozen)]
#[derive(Clone)]
pub struct Revision {
    pub id: String,
    pub author: String,
    pub description: String,
    pub base: String,
    pub head: String,
    pub timestamp: u64,
    pub comments: Vec<Comment>,
    pub reviews: Vec<Review>,
}

#[pymethods]
impl Revision {
    fn __repr__(&self) -> String { format!("Revision({}, {})", self.id, self.head) }
}

pub fn revision(r: &patch::Revision) -> Revision {
    Revision {
        id: r.id().to_string(),
        author: r.author().id().to_string(),
        description: r.description().to_owned(),
        base: r.base().to_string(),
        head: r.head().to_string(),
        timestamp: r.timestamp().as_millis(),
        comments: r.replies().map(|(id, c)| comment(id, c)).collect(),
        reviews: r
            .reviews()
            .map(|(_, v)| Review {
                id: v.id().to_string(),
                author: v.author().id().to_string(),
                verdict: v.verdict().map(|v| v.to_string()),
                summary: v.summary().to_owned(),
                timestamp: v.timestamp().as_millis(),
            })
            .collect(),
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Comment>()?;
    m.add_class::<Review>()?;
    m.add_class::<Revision>()?;
    Ok(())
}
