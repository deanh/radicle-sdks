//! Isolated Node binding. No shared-core extraction or Python changes.
mod patches;
mod repository;
use repository::{HistoryOptions, HistoryPage, SignedRefs, Tree, TreeEntry, page_number};
#[cfg(not(panic = "unwind"))]
compile_error!("radicle-sdk requires panic=unwind to contain worker task panics");
use napi::{
    Env, Error, Result, Status, Task,
    bindgen_prelude::{AsyncTask, Either, Null, ToNapiValue, TypeName, Uint8Array},
};
use napi_derive::napi;
use radicle::{
    cob::{
        Label, ObjectId, Title,
        cache::NoCache,
        issue::{self, Issue as RawIssue, Issues as RawIssues},
        store::access::{ReadOnly, WriteAs},
    },
    crypto::Signer as _,
    identity::{Did, RepoId},
    storage::{ReadRepository, ReadStorage, SignRepository},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, Weak},
};

type Job<T> = Box<dyn FnOnce() -> Result<T> + Send>;
pub struct Work<T>(Option<Job<T>>);
impl<T: Send + ToNapiValue + TypeName + 'static> Task for Work<T> {
    type Output = T;
    type JsValue = T;
    fn compute(&mut self) -> Result<T> {
        let job = self
            .0
            .take()
            .ok_or_else(|| err("INTERNAL", "task already consumed"))?;
        // A job is consumed even on panic. Do not resume a possibly modified
        // transaction or clear poisoned locks; errors do not imply rollback.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
            Ok(result) => result,
            Err(payload) => {
                // Do not expose a potentially sensitive panic payload to JS.
                // A custom payload can itself panic on Drop; contain that too.
                if let Err(nested) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)))
                {
                    std::mem::forget(nested);
                }
                Err(err(
                    "INTERNAL",
                    "Rust task panicked; operation may have partial side effects",
                ))
            }
        }
    }
    fn resolve(&mut self, _: Env, value: T) -> Result<T> {
        Ok(value)
    }
}
fn work<T: Send + ToNapiValue + TypeName + 'static>(
    f: impl FnOnce() -> Result<T> + Send + 'static,
) -> AsyncTask<Work<T>> {
    AsyncTask::new(Work(Some(Box::new(f))))
}
fn err(code: &str, e: impl std::fmt::Display) -> Error {
    Error::new(Status::GenericFailure, format!("RADICLE:{code}:{e}"))
}
fn rid(s: &str) -> Result<RepoId> {
    s.parse().map_err(|e| err("ARG_VALUE", e))
}
fn oid(s: &str) -> Result<ObjectId> {
    s.parse().map_err(|e| err("ARG_VALUE", e))
}
const DATE_MAX: i64 = 8_640_000_000_000_000;
fn millis(value: i64, seconds: bool) -> Result<f64> {
    let ms = if seconds {
        value.checked_mul(1000)
    } else {
        Some(value)
    }
    .filter(|v| (-DATE_MAX..=DATE_MAX).contains(v))
    .ok_or_else(|| err("INVALID_DATA", "timestamp outside Date range"))?;
    Ok(ms as f64)
}

fn unsigned_millis(value: u64) -> Result<f64> {
    millis(
        i64::try_from(value).map_err(|e| err("INVALID_DATA", e))?,
        false,
    )
}

// A peer controls stored metadata, not our Date/number representation. Do not
// let an unrepresentable date hide the object (or an entire issue listing).
fn metadata_timestamp(value: Result<f64>) -> Either<f64, Null> {
    match value {
        Ok(ms) => Either::A(ms),
        Err(_) => Either::B(Null),
    }
}

#[napi(object)]
pub struct InitOptions {
    pub name: String,
    pub description: String,
    pub default_branch: String,
    #[napi(ts_type = "'public' | 'private'")]
    pub visibility: Option<String>,
}
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
#[napi(object)]
pub struct HeadInfo {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub oid: String,
}
#[napi(object)]
pub struct ProjectDoc {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub description: String,
    #[napi(readonly)]
    pub default_branch: String,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub delegates: Vec<String>,
    #[napi(readonly, ts_type = "'public' | 'private'")]
    pub visibility: String,
}
#[napi(object)]
pub struct Commit {
    #[napi(readonly)]
    pub oid: String,
    #[napi(readonly)]
    pub message: Uint8Array,
    #[napi(readonly)]
    pub message_str: String,
    #[napi(readonly)]
    pub author_name: Uint8Array,
    #[napi(readonly)]
    pub author_name_str: String,
    #[napi(readonly)]
    pub author_email: Uint8Array,
    #[napi(readonly)]
    pub author_email_str: String,
    #[napi(readonly)]
    pub author_time: Either<f64, Null>,
    #[napi(readonly)]
    pub author_offset_min: i32,
    #[napi(readonly)]
    pub committer_name: Uint8Array,
    #[napi(readonly)]
    pub committer_name_str: String,
    #[napi(readonly)]
    pub committer_email: Uint8Array,
    #[napi(readonly)]
    pub committer_email_str: String,
    #[napi(readonly)]
    pub committer_time: Either<f64, Null>,
    #[napi(readonly)]
    pub committer_offset_min: i32,
    #[napi(readonly)]
    pub tree: String,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub parents: Vec<String>,
    #[napi(readonly)]
    pub encoding: Option<String>,
}
#[napi(object)]
pub struct Comment {
    #[napi(readonly)]
    pub id: String,
    #[napi(readonly)]
    pub author: String,
    #[napi(readonly)]
    pub body: String,
    #[napi(readonly)]
    pub timestamp: Either<f64, Null>,
    #[napi(readonly)]
    pub reply_to: Option<String>,
}
#[napi(object)]
pub struct Issue {
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
fn parse_labels(values: Vec<String>) -> Result<Vec<Label>> {
    values
        .into_iter()
        .map(|v| Label::new(v).map_err(|e| err("ARG_VALUE", e)))
        .collect()
}
fn parse_dids(values: Vec<String>) -> Result<Vec<Did>> {
    values
        .into_iter()
        .map(|v| v.parse().map_err(|e| err("ARG_VALUE", e)))
        .collect()
}
fn snapshot(id: &ObjectId, issue: &RawIssue) -> Result<Issue> {
    let mut labels: Vec<_> = issue.labels().map(|v| v.name().to_owned()).collect();
    let mut assignees: Vec<_> = issue.assignees().map(ToString::to_string).collect();
    labels.sort();
    assignees.sort();
    Ok(Issue {
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
            .map(|(id, c)| {
                Ok(Comment {
                    id: id.to_string(),
                    author: Did::from(*c.author()).to_string(),
                    body: c.body().to_owned(),
                    timestamp: metadata_timestamp(unsigned_millis(c.timestamp().as_millis())),
                    reply_to: c.reply_to().map(|id| id.to_string()),
                })
            })
            .collect::<Result<_>>()?,
    })
}

// Weak entries avoid keeping an unbounded number of repository mutexes alive.
// Clean dead keys on acquisition; every active operation owns the same Arc.
type LockKey = (PathBuf, RepoId);
type Locks = HashMap<LockKey, Weak<Mutex<()>>>;
fn repo_lock(repo: &NativeRepository) -> Result<Arc<Mutex<()>>> {
    static LOCKS: OnceLock<Mutex<Locks>> = OnceLock::new();
    let key = (
        repo.storage
            .path()
            .canonicalize()
            .map_err(|e| err("STORAGE", e))?,
        repo.rid,
    );
    let mut locks = LOCKS
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| err("INTERNAL", e))?;
    locks.retain(|_, v| v.strong_count() != 0);
    let entry = locks.entry(key).or_default();
    if let Some(lock) = entry.upgrade() {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    *entry = Arc::downgrade(&lock);
    Ok(lock)
}

#[napi]
#[derive(Clone)]
pub struct NativeProfile {
    inner: Arc<radicle::Profile>,
}
#[napi]
impl NativeProfile {
    #[napi(catch_unwind, ts_return_type = "Promise<NativeProfile>")]
    pub fn init(
        home: String,
        alias: String,
        passphrase: Option<String>,
    ) -> AsyncTask<Work<NativeProfile>> {
        work(move || {
            let alias = alias.parse().map_err(|e| err("ARG_VALUE", e))?;
            let mut seed = [0; 32];
            getrandom::fill(&mut seed).map_err(|e| err("INTERNAL", e))?;
            let home = radicle::profile::Home::new(home).map_err(|e| err("STORAGE", e))?;
            let profile = radicle::Profile::init(
                home,
                alias,
                passphrase.map(Into::into),
                radicle::crypto::Seed::new(seed),
            )
            .map_err(|e| err("IDENTITY", e))?;
            Ok(Self {
                inner: Arc::new(profile),
            })
        })
    }
    #[napi(factory, catch_unwind)]
    pub fn load() -> Result<Self> {
        Ok(Self {
            inner: Arc::new(radicle::Profile::load().map_err(|e| err("IDENTITY", e))?),
        })
    }
    #[napi(getter, catch_unwind)]
    pub fn did(&self) -> String {
        self.inner.did().to_string()
    }
    #[napi(getter, catch_unwind)]
    pub fn node_id(&self) -> String {
        self.inner.id().to_string()
    }
    #[napi(getter, catch_unwind)]
    pub fn storage(&self) -> NativeStorage {
        NativeStorage {
            inner: self.inner.storage.clone(),
        }
    }
    #[napi(catch_unwind)]
    pub fn signer(&self) -> Result<NativeSigner> {
        let signer = self.inner.signer().map_err(|e| err("SIGNER", e))?;
        Ok(NativeSigner {
            node_id: signer.public_key().to_string(),
            inner: Arc::new(Mutex::new(signer)),
        })
    }
}
#[napi]
#[derive(Clone)]
pub struct NativeSigner {
    inner: Arc<Mutex<radicle::profile::Signer>>,
    node_id: String,
}
#[napi]
impl NativeSigner {
    #[napi(getter, catch_unwind)]
    pub fn node_id(&self) -> String {
        self.node_id.clone()
    }
}
#[napi]
#[derive(Clone)]
pub struct NativeStorage {
    inner: radicle::storage::git::Storage,
}
#[napi]
impl NativeStorage {
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn contains(&self, repo_id: String) -> AsyncTask<Work<bool>> {
        let storage = self.inner.clone();
        work(move || {
            storage
                .contains(&rid(&repo_id)?)
                .map_err(|e| err("REPOSITORY", e))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<string>>")]
    pub fn repositories(&self) -> AsyncTask<Work<Vec<String>>> {
        let storage = self.inner.clone();
        work(move || {
            Ok(storage
                .repositories()
                .map_err(|e| err("STORAGE", e))?
                .into_iter()
                .map(|info| info.rid.to_string())
                .collect())
        })
    }
    #[napi(catch_unwind)]
    pub fn repository(&self, repo_id: String) -> Result<NativeRepository> {
        Ok(NativeRepository {
            storage: self.inner.clone(),
            rid: rid(&repo_id)?,
        })
    }
}
#[napi]
#[derive(Clone)]
pub struct NativeRepository {
    storage: radicle::storage::git::Storage,
    rid: RepoId,
}
impl NativeRepository {
    fn open(&self) -> Result<radicle::storage::git::Repository> {
        self.storage
            .repository(self.rid)
            .map_err(|e| err("REPOSITORY", e))
    }
}
#[napi]
impl NativeRepository {
    #[napi(getter, catch_unwind)]
    pub fn rid(&self) -> String {
        self.rid.to_string()
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<string>>")]
    pub fn branches(&self) -> AsyncTask<Work<Vec<String>>> {
        let me = self.clone();
        work(move || {
            let repo = me.open()?;
            let mut names = Vec::new();
            for reference in repo.backend.references().map_err(|e| err("GIT", e))? {
                let reference = reference.map_err(|e| err("GIT", e))?;
                if let Some(name) = reference
                    .name()
                    .map_err(|e| err("GIT", e))?
                    .strip_prefix("refs/heads/")
                {
                    names.push(name.to_owned());
                }
            }
            names.sort();
            Ok(names)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<string | null>")]
    pub fn branch_head(&self, name: String) -> AsyncTask<Work<Option<String>>> {
        let me = self.clone();
        work(move || {
            let full = format!("refs/heads/{name}");
            if name.is_empty() || !radicle::git::raw::Reference::is_valid_name(&full) {
                return Err(err("ARG_VALUE", "invalid branch name"));
            }
            let repo = me.open()?;
            match repo.backend.refname_to_id(&full) {
                Ok(id) => Ok(Some(id.to_string())),
                Err(e) if e.code() == radicle::git::raw::ErrorCode::NotFound => Ok(None),
                Err(e) => Err(err("GIT", e)),
            }
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<Tree>")]
    pub fn tree(&self, oid: String) -> AsyncTask<Work<Tree>> {
        let me = self.clone();
        work(move || {
            let oid = radicle::git::raw::Oid::from_str(&oid).map_err(|e| err("ARG_VALUE", e))?;
            let repo = me.open()?;
            let tree = repo.backend.find_tree(oid).map_err(|e| err("GIT", e))?;
            Ok(Tree {
                oid: tree.id().to_string(),
                entries: tree
                    .iter()
                    .map(|e| TreeEntry {
                        name: e.name_bytes().to_vec().into(),
                        name_str: String::from_utf8_lossy(e.name_bytes()).into_owned(),
                        kind: e.kind().map(|kind| {
                            match kind {
                                radicle::git::raw::ObjectType::Blob => "blob",
                                radicle::git::raw::ObjectType::Tree => "tree",
                                radicle::git::raw::ObjectType::Commit => "commit",
                                radicle::git::raw::ObjectType::Tag => "tag",
                                radicle::git::raw::ObjectType::Any => "any",
                            }
                            .to_owned()
                        }),
                        mode: e.filemode() as u32,
                        oid: e.id().to_string(),
                    })
                    .collect(),
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<Uint8Array>")]
    pub fn blob(&self, oid: String) -> AsyncTask<Work<Uint8Array>> {
        let me = self.clone();
        work(move || {
            let id = *self::oid(&oid)?;
            let repo = me.open()?;
            let blob = repo.blob(id).map_err(|e| err("GIT", e))?;
            Ok(blob.content().to_vec().into())
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<HistoryPage>")]
    pub fn history(&self, rev: String, options: HistoryOptions) -> AsyncTask<Work<HistoryPage>> {
        let me = self.clone();
        work(move || {
            let limit = page_number(options.limit.unwrap_or(100.0), "limit", 1.0)?.min(1000);
            let offset = page_number(options.offset.unwrap_or(0.0), "offset", 0.0)?;
            let repo = me.open()?;
            let head = repo
                .backend
                .revparse_single(&rev)
                .map_err(|e| err("GIT", e))?
                .id();
            let mut walk = repo.revwalk(head.into()).map_err(|e| err("GIT", e))?;
            // Iterator::skip would silently discard upstream errors in skipped
            // ancestry. Check every entry consumed, including before the page.
            for skipped in walk.by_ref().take(offset) {
                skipped.map_err(|e| err("GIT", e))?;
            }
            let mut oids = walk
                .take(limit + 1)
                .map(|oid| oid.map(|id| id.to_string()).map_err(|e| err("GIT", e)))
                .collect::<Result<Vec<_>>>()?;
            let has_more = oids.len() > limit;
            oids.truncate(limit);
            Ok(HistoryPage {
                name: rev,
                head: head.to_string(),
                oids,
                offset: offset as f64,
                has_more,
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<SignedRefs>")]
    pub fn sign_refs(&self, signer: &NativeSigner) -> AsyncTask<Work<SignedRefs>> {
        let me = self.clone();
        let signer = signer.clone();
        work(move || {
            let lock = repo_lock(&me)?;
            let _guard = lock.lock().map_err(|e| err("INTERNAL", e))?;
            let repo = me.open()?;
            let sg = signer.inner.lock().map_err(|e| err("INTERNAL", e))?;
            let signed = repo.sign_refs(&*sg).map_err(|e| err("REPOSITORY", e))?;
            Ok(SignedRefs {
                node_id: signed.id().to_string(),
                at: signed.at.to_string(),
                parent: signed.parent().map(|id| id.to_string()),
                feature_level: signed.feature_level().to_string(),
                refs: signed
                    .refs()
                    .iter()
                    .map(|(name, id)| (name.to_string(), id.to_string()))
                    .collect(),
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<HeadInfo>")]
    pub fn head(&self) -> AsyncTask<Work<HeadInfo>> {
        let me = self.clone();
        work(move || {
            let repo = me.open()?;
            let (name, oid) = repo.head().map_err(|e| err("REPOSITORY", e))?;
            Ok(HeadInfo {
                name: name.to_string(),
                oid: oid.to_string(),
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ProjectDoc>")]
    pub fn identity(&self) -> AsyncTask<Work<ProjectDoc>> {
        let me = self.clone();
        work(move || {
            let repo = me.open()?;
            let doc = repo
                .canonical_identity_doc()
                .map_err(|e| err("IDENTITY", e))?;
            let p = doc.project().map_err(|e| err("IDENTITY", e))?;
            Ok(ProjectDoc {
                name: p.name().to_string(),
                description: p.description().to_owned(),
                default_branch: p.default_branch().to_string(),
                delegates: doc.delegates().iter().map(ToString::to_string).collect(),
                visibility: if matches!(doc.visibility(), radicle::identity::Visibility::Public) {
                    "public"
                } else {
                    "private"
                }
                .into(),
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<Commit>")]
    pub fn commit(&self, oid: String) -> AsyncTask<Work<Commit>> {
        let me = self.clone();
        work(move || {
            let oid = radicle::git::raw::Oid::from_str(&oid).map_err(|e| err("ARG_VALUE", e))?;
            let repo = me.open()?;
            let c = repo.backend.find_commit(oid).map_err(|e| err("GIT", e))?;
            Ok(Commit {
                oid: c.id().to_string(),
                message: c.message_bytes().to_vec().into(),
                message_str: String::from_utf8_lossy(c.message_bytes()).into_owned(),
                author_name: c.author().name_bytes().to_vec().into(),
                author_name_str: String::from_utf8_lossy(c.author().name_bytes()).into_owned(),
                author_email: c.author().email_bytes().to_vec().into(),
                author_email_str: String::from_utf8_lossy(c.author().email_bytes()).into_owned(),
                author_time: metadata_timestamp(millis(c.author().when().seconds(), true)),
                author_offset_min: c.author().when().offset_minutes(),
                committer_name: c.committer().name_bytes().to_vec().into(),
                committer_name_str: String::from_utf8_lossy(c.committer().name_bytes())
                    .into_owned(),
                committer_email: c.committer().email_bytes().to_vec().into(),
                committer_email_str: String::from_utf8_lossy(c.committer().email_bytes())
                    .into_owned(),
                committer_time: metadata_timestamp(millis(c.committer().when().seconds(), true)),
                committer_offset_min: c.committer().when().offset_minutes(),
                tree: c.tree_id().to_string(),
                parents: c.parent_ids().map(|id| id.to_string()).collect(),
                encoding: c.message_encoding().ok().flatten().map(str::to_owned),
            })
        })
    }
}
#[napi(catch_unwind, ts_return_type = "Promise<string>")]
pub fn init_project(
    storage: &NativeStorage,
    signer: &NativeSigner,
    path: String,
    opts: InitOptions,
) -> AsyncTask<Work<String>> {
    let storage = storage.inner.clone();
    let signer = signer.inner.clone();
    work(move || {
        let visibility = match opts.visibility.as_deref().unwrap_or("public") {
            "public" => radicle::identity::Visibility::Public,
            "private" => radicle::identity::Visibility::private([]),
            _ => return Err(err("ARG_VALUE", "visibility must be 'public' or 'private'")),
        };
        let name = opts.name.try_into().map_err(|e| err("ARG_VALUE", e))?;
        let b = &opts.default_branch;
        if b.is_empty()
            || b.starts_with("refs/")
            || !radicle::git::raw::Reference::is_valid_name(&format!("refs/heads/{b}"))
        {
            return Err(err(
                "ARG_VALUE",
                "defaultBranch must be a valid short branch name",
            ));
        }
        let branch = radicle::git::fmt::RefStr::try_from_str(b)
            .map_err(|e| err("ARG_VALUE", e))?
            .to_ref_string();
        let repo = radicle::git::raw::Repository::open(path).map_err(|e| err("GIT", e))?;
        match repo.find_remote(&radicle::rad::REMOTE_NAME) {
            Ok(_) => {
                return Err(err(
                    "REPOSITORY",
                    "repository already has a rad remote; refusing to initialize",
                ));
            }
            Err(e) if e.code() == radicle::git::raw::ErrorCode::NotFound => {}
            Err(e) => return Err(err("GIT", e)),
        }
        repo.find_reference(&format!("refs/heads/{b}"))
            .and_then(|r| r.peel_to_commit())
            .map_err(|e| err("GIT", e))?;
        // RID does not exist yet. Existing-repository locks do not cover init.
        let signer = signer.lock().map_err(|e| err("INTERNAL", e))?;
        let (id, _, _) = radicle::rad::init(
            &repo,
            name,
            &opts.description,
            branch,
            visibility,
            &*signer,
            &storage,
        )
        .map_err(|e| err("REPOSITORY", e))?;
        Ok(id.to_string())
    })
}
#[napi(catch_unwind, ts_return_type = "Promise<void>")]
pub fn fork_project(
    storage: &NativeStorage,
    signer: &NativeSigner,
    repo_id: String,
) -> AsyncTask<Work<()>> {
    let storage = storage.inner.clone();
    let signer = signer.inner.clone();
    work(move || {
        let repo = NativeRepository {
            storage,
            rid: rid(&repo_id)?,
        };
        let lock = repo_lock(&repo)?;
        let _guard = lock.lock().map_err(|e| err("INTERNAL", e))?;
        let signer = signer.lock().map_err(|e| err("INTERNAL", e))?;
        radicle::rad::fork(repo.rid, &*signer, &repo.storage).map_err(|e| err("REPOSITORY", e))
    })
}
#[napi(catch_unwind, ts_return_type = "Promise<string>")]
pub fn checkout_project(
    storage: &NativeStorage,
    repo_id: String,
    remote: String,
    path: String,
) -> AsyncTask<Work<String>> {
    let storage = storage.inner.clone();
    work(move || {
        let repo = NativeRepository {
            storage,
            rid: rid(&repo_id)?,
        };
        let remote = remote.parse().map_err(|e| err("ARG_VALUE", e))?;
        radicle::rad::checkout(
            repo.rid,
            &remote,
            std::path::Path::new(&path),
            &repo.storage,
            false,
        )
        .map_err(|e| err("REPOSITORY", e))?;
        Ok(path)
    })
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

/// T0-only native probe: decimal strings avoid JS precision loss before testing.
/// Not re-exported by the SDK wrapper.
#[napi(catch_unwind, ts_return_type = "Promise<number>")]
pub fn probe_timestamp(value: String, seconds: bool) -> AsyncTask<Work<f64>> {
    work(move || {
        let value = value.parse::<i64>().map_err(|e| {
            let code = match e.kind() {
                std::num::IntErrorKind::PosOverflow | std::num::IntErrorKind::NegOverflow => {
                    "ARG_RANGE"
                }
                _ => "ARG_VALUE",
            };
            err(code, e)
        })?;
        millis(value, seconds)
    })
}

/// T0-only containment probe, not re-exported by the package.
#[napi(catch_unwind, ts_return_type = "Promise<void>")]
pub fn probe_panic() -> AsyncTask<Work<()>> {
    work(|| panic!("T0 injected worker panic"))
}

/// T0-only synchronous FFI containment probe.
#[napi(catch_unwind)]
pub fn probe_sync_panic() {
    panic!("T0 injected synchronous panic");
}

#[cfg(test)]
mod tests {
    use super::*;
    static_assertions::assert_impl_all!(radicle::profile::Signer: Send);
    static_assertions::assert_not_impl_any!(radicle::profile::Signer: Sync);
    static_assertions::assert_impl_all!(radicle::Profile: Send, Sync);
    static_assertions::assert_impl_all!(radicle::storage::git::Storage: Send, Sync);
    static_assertions::assert_impl_all!(radicle::storage::git::Repository: Send);
    static_assertions::assert_not_impl_any!(radicle::storage::git::Repository: Sync);
    static_assertions::assert_impl_all!(patches::NativePatches: Send, Sync);
    static_assertions::assert_impl_all!(patches::NativePatchMut: Send, Sync);
    #[test]
    fn task_panic_fails_closed_without_unwinding_or_clearing_poison() {
        let lock = Arc::new(Mutex::new(()));
        let held = lock.clone();
        let mut task = Work::<()>(Some(Box::new(move || {
            let _guard = held.lock().unwrap();
            panic!("injected while locked");
        })));
        assert!(
            task.compute()
                .unwrap_err()
                .reason
                .starts_with("RADICLE:INTERNAL:")
        );
        assert!(lock.is_poisoned());
        assert!(task.compute().is_err()); // consumed; never rerun
        let mut next = Work(Some(Box::new(|| Ok(42))));
        assert_eq!(next.compute().unwrap(), 42);
    }

    #[test]
    fn stored_timestamp_unrepresentable_values_are_null() {
        assert!(
            matches!(metadata_timestamp(millis(DATE_MAX, false)), Either::A(v) if v == DATE_MAX as f64)
        );
        for value in [
            millis(DATE_MAX + 1, false),
            millis(-DATE_MAX - 1, false),
            millis(i64::MAX, true),
            millis(i64::MIN, true),
            unsigned_millis(u64::MAX),
        ] {
            assert!(matches!(metadata_timestamp(value), Either::B(Null)));
        }
    }

    #[test]
    fn timestamp_bounds() {
        assert_eq!(millis(DATE_MAX, false).unwrap(), DATE_MAX as f64);
        assert_eq!(millis(-DATE_MAX, false).unwrap(), -DATE_MAX as f64);
        assert!(millis(DATE_MAX + 1, false).is_err());
        assert!(millis(i64::MAX, true).is_err());
        assert!(millis(i64::MIN, true).is_err());
        assert!(unsigned_millis(u64::MAX).is_err());
        assert!(unsigned_millis(DATE_MAX as u64 + 1).is_err());
    }
}
