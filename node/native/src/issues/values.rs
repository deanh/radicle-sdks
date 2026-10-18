//! Detached issue snapshots, materialized eagerly.
use crate::{
    comment::Comment,
    timestamps::{metadata_timestamp, unsigned_millis},
};
use napi_derive::napi;
use radicle::{
    cob::{
        ObjectId,
        issue::{self, Issue as RawIssue},
    },
    identity::Did,
};
#[napi(object)]
pub struct Issue {
    #[napi(readonly)]
    pub comment_count: f64,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub comment_ids: Vec<String>,
    #[napi(readonly)]
    pub id: String,
    #[napi(readonly)]
    pub title: String,
    #[napi(readonly)]
    pub description: String,
    #[napi(readonly)]
    pub author: String,
    #[napi(readonly, ts_type = "'solved' | 'unspecified'")]
    pub close_reason: Option<String>,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub labels: Vec<String>,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub assignees: Vec<String>,
    #[napi(readonly, ts_type = "'open' | 'closed'")]
    pub state: String,
    #[napi(readonly, ts_type = "ReadonlyArray<Comment>")]
    pub comments: Vec<Comment>,
}
#[napi(object)]
pub struct IssueChange {
    #[napi(readonly)]
    pub entry_id: String,
    #[napi(readonly)]
    pub snapshot: Issue,
}
pub(super) fn snapshot(id: &ObjectId, issue: &RawIssue) -> Issue {
    let mut labels: Vec<_> = issue.labels().map(|v| v.name().to_owned()).collect();
    let mut assignees: Vec<_> = issue.assignees().map(ToString::to_string).collect();
    labels.sort();
    assignees.sort();
    let comment_ids: Vec<_> = issue.comments().map(|(id, _)| id.to_string()).collect();
    Issue {
        comment_count: comment_ids.len() as f64,
        comment_ids,
        id: id.to_string(),
        title: issue.title().to_string(),
        description: issue.description().to_owned(),
        author: issue.author().id().to_string(),
        close_reason: match issue.state() {
            issue::State::Open => None,
            issue::State::Closed { reason } => Some(reason.to_string()),
        },
        labels,
        assignees,
        state: if matches!(issue.state(), radicle::cob::issue::State::Open) {
            "open"
        } else {
            "closed"
        }
        .into(),
        comments: issue
            .comments()
            .map(|(id, c)| Comment {
                id: id.to_string(),
                author: Did::from(*c.author()).to_string(),
                body: c.body().to_owned(),
                timestamp: metadata_timestamp(unsigned_millis(c.timestamp().as_millis())),
                reply_to: c.reply_to().map(|id| id.to_string()),
            })
            .collect(),
    }
}
