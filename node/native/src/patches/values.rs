//! Detached patch/revision/review snapshots and eager materialization.
use crate::{
    comment::Comment,
    timestamps::{metadata_timestamp, unsigned_millis},
};
use napi::bindgen_prelude::{Either, Null};
use napi_derive::napi;
use radicle::{
    cob::{
        ObjectId,
        patch::{self, Patch as RawPatch},
    },
    identity::Did,
};

#[napi(object)]
pub struct Review {
    #[napi(readonly)]
    pub id: String,
    #[napi(readonly)]
    pub author: String,
    #[napi(readonly, ts_type = "'accept' | 'reject'")]
    pub verdict: Option<String>,
    #[napi(readonly)]
    pub summary: String,
    #[napi(readonly)]
    pub timestamp: Either<f64, Null>,
}
#[napi(object)]
pub struct Revision {
    #[napi(readonly)]
    pub id: String,
    #[napi(readonly)]
    pub author: String,
    #[napi(readonly)]
    pub description: String,
    #[napi(readonly)]
    pub base: String,
    #[napi(readonly)]
    pub head: String,
    #[napi(readonly)]
    pub timestamp: Either<f64, Null>,
    #[napi(readonly, ts_type = "ReadonlyArray<Comment>")]
    pub comments: Vec<Comment>,
    #[napi(readonly, ts_type = "ReadonlyArray<Review>")]
    pub reviews: Vec<Review>,
}
#[napi(object)]
pub struct Patch {
    #[napi(readonly)]
    pub revision_count: f64,
    #[napi(readonly)]
    pub id: String,
    #[napi(readonly)]
    pub title: String,
    #[napi(readonly)]
    pub description: String,
    #[napi(readonly)]
    pub author: String,
    #[napi(readonly, ts_type = "'draft' | 'open' | 'archived' | 'merged'")]
    pub state: String,
    #[napi(readonly, ts_type = "'delegates' | `branch:${string}`")]
    pub target: String,
    #[napi(readonly)]
    pub base: String,
    #[napi(readonly)]
    pub head: String,
    #[napi(readonly)]
    pub latest_revision: String,
    #[napi(readonly)]
    pub version: f64,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub labels: Vec<String>,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub assignees: Vec<String>,
    #[napi(readonly, ts_type = "ReadonlyArray<Revision>")]
    pub revisions: Vec<Revision>,
    #[napi(readonly)]
    pub merged_revision: Option<String>,
    #[napi(readonly)]
    pub merged_commit: Option<String>,
}
#[napi(object)]
pub struct PatchChange {
    #[napi(readonly)]
    pub entry_id: String,
    #[napi(readonly)]
    pub snapshot: Patch,
}
#[napi(object)]
pub struct PatchRevisionChange {
    #[napi(readonly)]
    pub revision_id: String,
    #[napi(readonly)]
    pub snapshot: Patch,
}
#[napi(object)]
pub struct PatchStateChange {
    #[napi(readonly)]
    pub changed: bool,
    #[napi(readonly)]
    pub snapshot: Patch,
}
pub(super) fn snapshot(id: &ObjectId, p: &RawPatch) -> Patch {
    let mut labels: Vec<_> = p.labels().map(ToString::to_string).collect();
    let mut assignees: Vec<_> = p.assignees().map(|v| v.to_string()).collect();
    labels.sort();
    assignees.sort();
    let (latest, revision) = p.latest();
    Patch {
        revision_count: p.revisions().count() as f64,
        id: id.to_string(),
        title: p.title().to_owned(),
        description: revision.description().to_owned(),
        author: p.author().id().to_string(),
        state: match p.state() {
            patch::State::Draft => "draft",
            patch::State::Open { .. } => "open",
            patch::State::Archived => "archived",
            patch::State::Merged { .. } => "merged",
        }
        .into(),
        target: match p.target() {
            patch::MergeTarget::Delegates => "delegates".into(),
            patch::MergeTarget::Branch(branch) => format!(
                "branch:{}",
                branch
                    .as_str()
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch.as_str())
            ),
        },
        base: revision.base().to_string(),
        head: revision.head().to_string(),
        latest_revision: latest.to_string(),
        // Slot count minus one; a plain JS number like HistoryPage.offset.
        version: p.version() as f64,
        labels,
        assignees,
        revisions: p
            .revisions()
            .map(|(_, r)| Revision {
                id: r.id().to_string(),
                author: r.author().id().to_string(),
                description: r.description().to_owned(),
                base: r.base().to_string(),
                head: r.head().to_string(),
                timestamp: metadata_timestamp(unsigned_millis(r.timestamp().as_millis())),
                comments: r
                    .replies()
                    .map(|(id, c)| Comment {
                        id: id.to_string(),
                        author: Did::from(*c.author()).to_string(),
                        body: c.body().to_owned(),
                        timestamp: metadata_timestamp(unsigned_millis(c.timestamp().as_millis())),
                        reply_to: c.reply_to().map(|id| id.to_string()),
                    })
                    .collect(),
                reviews: r
                    .reviews()
                    .map(|(_, v)| Review {
                        id: v.id().to_string(),
                        author: v.author().id().to_string(),
                        verdict: v.verdict().map(|v| v.to_string()),
                        summary: v.summary().to_owned(),
                        timestamp: metadata_timestamp(unsigned_millis(v.timestamp().as_millis())),
                    })
                    .collect(),
            })
            .collect(),
        merged_revision: match p.state() {
            patch::State::Merged { revision, .. } => Some(revision.to_string()),
            _ => None,
        },
        merged_commit: match p.state() {
            patch::State::Merged { commit, .. } => Some(commit.to_string()),
            _ => None,
        },
    }
}
