//! Local issue workflows; writes snapshot under repository then signer locks.
mod values;
use crate::{
    error::err,
    tasks::{Work, work},
};
use crate::{
    ids::{oid, parse_dids, parse_labels},
    locks::repo_lock,
    profile::NativeSigner,
    repository::NativeRepository,
};
use napi::{Result, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use radicle::{
    cob::{
        Label, ObjectId, Title,
        cache::NoCache,
        issue::{self, Issues as RawIssues},
        store::access::{ReadOnly, WriteAs},
    },
    identity::Did,
};
use values::{Issue, IssueChange, snapshot};
#[napi(object)]
pub struct CreateIssueOptions {
    pub title: String,
    pub description: String,
    #[napi(ts_type = "ReadonlyArray<string>")]
    pub labels: Option<Vec<String>>,
    #[napi(ts_type = "ReadonlyArray<string>")]
    pub assignees: Option<Vec<String>>,
}
#[napi(object)]
pub struct CloseIssueOptions {
    #[napi(ts_type = "'solved' | 'unspecified'")]
    pub reason: Option<String>,
}
#[napi]
#[derive(Clone)]
pub struct NativeIssues {
    repo: NativeRepository,
    signer: Option<NativeSigner>,
}
#[napi]
impl NativeIssues {
    #[napi(factory, catch_unwind)]
    pub fn open(repo: &NativeRepository) -> Self {
        Self {
            repo: repo.clone(),
            signer: None,
        }
    }
    #[napi(factory, catch_unwind)]
    pub fn open_as(repo: &NativeRepository, signer: &NativeSigner) -> Self {
        Self {
            repo: repo.clone(),
            signer: Some(signer.clone()),
        }
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativeIssueMut>")]
    pub fn create(&self, opts: CreateIssueOptions) -> AsyncTask<Work<NativeIssueMut>> {
        let me = self.clone();
        work(move || {
            let signer = me
                .signer
                .ok_or_else(|| err("READ_ONLY", "issue store is read-only"))?;
            let title = Title::new(&opts.title).map_err(|e| err("ARG_VALUE", e))?;
            let labels = parse_labels(opts.labels.unwrap_or_default())?;
            let assignees = parse_dids(opts.assignees.unwrap_or_default())?;
            let lock = repo_lock(&me.repo)?;
            let _guard = lock.lock().map_err(|e| err("INTERNAL", e))?;
            let repo = me.repo.open()?;
            let sg = signer.inner.lock().map_err(|e| err("INTERNAL", e))?;
            let mut store =
                RawIssues::open(&repo, WriteAs::new(&*sg)).map_err(|e| err("COB", e))?;
            let mut cache = NoCache;
            let issue = store
                .create(title, opts.description, &labels, &assignees, [], &mut cache)
                .map_err(|e| err("COB", e))?;
            Ok(NativeIssueMut {
                repo: me.repo,
                signer: signer.clone(),
                id: *issue.id(),
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativeIssueMut>")]
    pub fn get_mut(&self, id: String) -> AsyncTask<Work<NativeIssueMut>> {
        let me = self.clone();
        work(move || {
            let signer = me
                .signer
                .ok_or_else(|| err("READ_ONLY", "issue store is read-only"))?;
            let handle = NativeIssueMut {
                repo: me.repo,
                signer,
                id: oid(&id)?,
            };
            handle.read()?;
            Ok(handle)
        })
    }
    /// Eager, ID-sorted snapshots; a store/item error rejects the whole listing.
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<Issue>>")]
    pub fn all(&self) -> AsyncTask<Work<Vec<Issue>>> {
        let me = self.clone();
        work(move || {
            let repo = me.repo.open()?;
            let store = RawIssues::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
            let mut out = Vec::new();
            for item in store.all().map_err(|e| err("COB", e))? {
                let (id, issue) = item.map_err(|e| err("COB", e))?;
                out.push(snapshot(&id, &issue)?);
            }
            out.sort_by(|a, b| a.id.cmp(&b.id));
            Ok(out)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<Issue | null>")]
    pub fn get(&self, id: String) -> AsyncTask<Work<Option<Issue>>> {
        let me = self.clone();
        work(move || {
            let id = oid(&id)?;
            let repo = me.repo.open()?;
            let store = RawIssues::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
            store
                .get(&id)
                .map_err(|e| err("COB", e))?
                .map(|i| snapshot(&id, &i))
                .transpose()
        })
    }
}
#[napi]
#[derive(Clone)]
pub struct NativeIssueMut {
    repo: NativeRepository,
    signer: NativeSigner,
    id: ObjectId,
}
// Issue-local dispatch keeps the lock/load/commit/snapshot boundary identical
// for every operation without introducing a shared core or transaction framework.
enum IssueOperation {
    Comment(String, Option<radicle::git::Oid>),
    Edit(Title),
    EditDescription(String),
    Label(Vec<Label>),
    Assign(Vec<Did>),
    Lifecycle(issue::State),
}
impl NativeIssueMut {
    fn read(&self) -> Result<Issue> {
        // Objects are written before refs move: concurrent readers can observe
        // an older or newer persisted view without serializing on write locks.
        let repo = self.repo.open()?;
        let store = RawIssues::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
        let issue = store
            .get(&self.id)
            .map_err(|e| err("COB", e))?
            .ok_or_else(|| err("COB", "issue not found"))?;
        snapshot(&self.id, &issue)
    }
    fn change(&self, operation: IssueOperation) -> Result<IssueChange> {
        let lock = repo_lock(&self.repo)?;
        let _guard = lock.lock().map_err(|e| err("INTERNAL", e))?;
        let repo = self.repo.open()?;
        let sg = self.signer.inner.lock().map_err(|e| err("INTERNAL", e))?;
        let mut store = RawIssues::open(&repo, WriteAs::new(&*sg)).map_err(|e| err("COB", e))?;
        let mut cache = NoCache;
        let mut issue = store
            .get_mut(&self.id, &mut cache)
            .map_err(|e| err("COB", e))?;
        let entry = match operation {
            IssueOperation::Comment(body, reply) => {
                let reply = reply
                    .or_else(|| issue.comments().next().map(|(id, _)| *id))
                    .ok_or_else(|| err("COB", "no root comment"))?;
                issue.comment(body, reply, [])
            }
            IssueOperation::Edit(title) => issue.edit(title),
            IssueOperation::EditDescription(description) => issue.edit_description(description, []),
            IssueOperation::Label(labels) => issue.label(labels),
            IssueOperation::Assign(assignees) => issue.assign(assignees),
            IssueOperation::Lifecycle(state) => issue.lifecycle(state),
        }
        .map_err(|e| err("COB", e))?;
        Ok(IssueChange {
            entry_id: entry.to_string(),
            snapshot: snapshot(&self.id, &issue)?,
        })
    }
}
#[napi]
impl NativeIssueMut {
    #[napi(getter, catch_unwind)]
    pub fn id(&self) -> String {
        self.id.to_string()
    }
    #[napi(catch_unwind, ts_return_type = "Promise<Issue>")]
    pub fn reload(&self) -> AsyncTask<Work<Issue>> {
        let me = self.clone();
        work(move || me.read())
    }
    #[napi(catch_unwind, ts_return_type = "Promise<IssueChange>")]
    pub fn comment(&self, body: String, reply_to: Option<String>) -> AsyncTask<Work<IssueChange>> {
        let me = self.clone();
        work(move || {
            let reply = reply_to
                .map(|s| {
                    s.parse::<radicle::git::Oid>()
                        .map_err(|e| err("ARG_VALUE", e))
                })
                .transpose()?;
            me.change(IssueOperation::Comment(body, reply))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<IssueChange>")]
    pub fn edit(&self, title: String) -> AsyncTask<Work<IssueChange>> {
        let me = self.clone();
        work(move || {
            let title = Title::new(&title).map_err(|e| err("ARG_VALUE", e))?;
            me.change(IssueOperation::Edit(title))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<IssueChange>")]
    pub fn edit_description(&self, description: String) -> AsyncTask<Work<IssueChange>> {
        let me = self.clone();
        work(move || me.change(IssueOperation::EditDescription(description)))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<IssueChange>")]
    pub fn label(
        &self,
        #[napi(ts_arg_type = "ReadonlyArray<string>")] labels: Vec<String>,
    ) -> AsyncTask<Work<IssueChange>> {
        let me = self.clone();
        work(move || me.change(IssueOperation::Label(parse_labels(labels)?)))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<IssueChange>")]
    pub fn assign(
        &self,
        #[napi(ts_arg_type = "ReadonlyArray<string>")] assignees: Vec<String>,
    ) -> AsyncTask<Work<IssueChange>> {
        let me = self.clone();
        work(move || me.change(IssueOperation::Assign(parse_dids(assignees)?)))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<IssueChange>")]
    pub fn close(&self, options: CloseIssueOptions) -> AsyncTask<Work<IssueChange>> {
        let me = self.clone();
        work(move || {
            let reason = match options.reason.as_deref().unwrap_or("unspecified") {
                "solved" => issue::CloseReason::Solved,
                "unspecified" => issue::CloseReason::Other,
                _ => return Err(err("ARG_VALUE", "reason must be 'solved' or 'unspecified'")),
            };
            me.change(IssueOperation::Lifecycle(issue::State::Closed { reason }))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<IssueChange>")]
    pub fn reopen(&self) -> AsyncTask<Work<IssueChange>> {
        let me = self.clone();
        work(move || me.change(IssueOperation::Lifecycle(issue::State::Open)))
    }
}
