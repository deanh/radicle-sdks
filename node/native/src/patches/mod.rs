//! Local patch workflows. Reads stay parallel; every write reloads, signs and
//! materializes its own result snapshot under the shared repository lock.
mod values;
use values::{Patch, PatchChange, PatchRevisionChange, PatchStateChange, snapshot};

use crate::{
    error::err,
    ids::{oid, parse_dids, parse_labels},
    locks::repo_lock,
    profile::NativeSigner,
    repository::NativeRepository,
    tasks::{Work, work},
};
use napi::{Result, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use radicle::cob::{
    ObjectId, Title,
    cache::NoCache,
    patch::{self, Patches as RawPatches},
    store::access::{ReadOnly, WriteAs},
};
use std::collections::BTreeSet;

#[napi(object)]
pub struct CreatePatchOptions {
    pub title: String,
    pub description: String,
    pub base: String,
    pub head: String,
    #[napi(ts_type = "'delegates' | `branch:${string}`")]
    pub target: Option<String>,
    #[napi(ts_type = "ReadonlyArray<string>")]
    pub labels: Option<Vec<String>>,
}
#[napi(object)]
pub struct EditPatchOptions {
    pub title: String,
    #[napi(ts_type = "'delegates' | `branch:${string}`")]
    pub target: String,
}
#[napi(object)]
pub struct UpdatePatchOptions {
    pub description: String,
    pub base: String,
    pub head: String,
}
#[napi(object)]
pub struct PatchCommentOptions {
    pub reply_to: Option<String>,
}
#[napi(object)]
pub struct ReviewPatchOptions {
    #[napi(ts_type = "'accept' | 'reject'")]
    pub verdict: Option<String>,
    pub summary: Option<String>,
}
fn target(value: &str) -> Result<patch::MergeTarget> {
    match value {
        "delegates" => Ok(patch::MergeTarget::Delegates),
        _ => {
            let name = value
                .strip_prefix("branch:")
                .ok_or_else(|| err("ARG_VALUE", "target must be 'delegates' or 'branch:<name>'"))?;
            let branch = patch::TargetBranch::try_from(name).map_err(|e| err("ARG_VALUE", e))?;
            Ok(patch::MergeTarget::Branch(branch))
        }
    }
}
fn revision_id(value: &str) -> Result<patch::RevisionId> {
    oid(value).map(|id| (*id).into())
}
#[napi]
#[derive(Clone)]
pub struct NativePatches {
    repo: NativeRepository,
    signer: Option<NativeSigner>,
}
impl NativePatches {
    fn create_or_draft(
        &self,
        options: CreatePatchOptions,
        draft: bool,
    ) -> AsyncTask<Work<NativePatchMut>> {
        let me = self.clone();
        work(move || {
            let signer = me
                .signer
                .ok_or_else(|| err("READ_ONLY", "patch store is read-only"))?;
            let title = Title::new(&options.title).map_err(|e| err("ARG_VALUE", e))?;
            let target = target(options.target.as_deref().unwrap_or("delegates"))?;
            let base = *oid(&options.base)?;
            let head = *oid(&options.head)?;
            let labels = parse_labels(options.labels.unwrap_or_default())?;
            let lock = repo_lock(&me.repo)?;
            let _guard = lock.lock().map_err(|e| err("INTERNAL", e))?;
            let repo = me.repo.open()?;
            let sg = signer.inner.lock().map_err(|e| err("INTERNAL", e))?;
            let mut store =
                RawPatches::open(&repo, WriteAs::new(&*sg)).map_err(|e| err("COB", e))?;
            let mut cache = NoCache;
            let p = if draft {
                store.draft(
                    title,
                    options.description,
                    target,
                    base,
                    head,
                    &labels,
                    &mut cache,
                )
            } else {
                store.create(
                    title,
                    options.description,
                    target,
                    base,
                    head,
                    &labels,
                    &mut cache,
                )
            }
            .map_err(|e| err("COB", e))?;
            Ok(NativePatchMut {
                repo: me.repo,
                signer: signer.clone(),
                id: *p.id(),
            })
        })
    }
}
#[napi]
impl NativePatches {
    #[napi(getter, catch_unwind)]
    pub fn rid(&self) -> String {
        self.repo.rid.to_string()
    }
    #[napi(catch_unwind, ts_return_type = "Promise<number>")]
    pub fn count(&self) -> AsyncTask<Work<f64>> {
        let me = self.clone();
        work(move || {
            let repo = me.repo.open()?;
            let store = RawPatches::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
            crate::tasks::count_number(store.count().map_err(|e| err("COB", e))?)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn is_empty(&self) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let repo = me.repo.open()?;
            let store = RawPatches::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
            store.is_empty().map_err(|e| err("COB", e))
        })
    }
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
    #[napi(catch_unwind, ts_return_type = "Promise<NativePatchMut>")]
    pub fn create(&self, options: CreatePatchOptions) -> AsyncTask<Work<NativePatchMut>> {
        self.create_or_draft(options, false)
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativePatchMut>")]
    pub fn draft(&self, options: CreatePatchOptions) -> AsyncTask<Work<NativePatchMut>> {
        self.create_or_draft(options, true)
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<Patch>>")]
    pub fn all(&self) -> AsyncTask<Work<Vec<Patch>>> {
        let me = self.clone();
        work(move || {
            let repo = me.repo.open()?;
            let store = RawPatches::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
            let mut out = Vec::new();
            for item in store.all().map_err(|e| err("COB", e))? {
                let (id, patch) = item.map_err(|e| err("COB", e))?;
                out.push(snapshot(&id, &patch));
            }
            out.sort_by(|a, b| a.id.cmp(&b.id));
            Ok(out)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<Patch | null>")]
    pub fn get(&self, id: String) -> AsyncTask<Work<Option<Patch>>> {
        let me = self.clone();
        work(move || {
            let id = oid(&id)?;
            let repo = me.repo.open()?;
            let store = RawPatches::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
            Ok(store
                .get(&id)
                .map_err(|e| err("COB", e))?
                .map(|p| snapshot(&id, &p)))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativePatchMut>")]
    pub fn get_mut(&self, id: String) -> AsyncTask<Work<NativePatchMut>> {
        let me = self.clone();
        work(move || {
            let signer = me
                .signer
                .ok_or_else(|| err("READ_ONLY", "patch store is read-only"))?;
            let handle = NativePatchMut {
                repo: me.repo,
                signer,
                id: oid(&id)?,
            };
            handle.read()?;
            Ok(handle)
        })
    }
}
#[napi]
#[derive(Clone)]
pub struct NativePatchMut {
    repo: NativeRepository,
    signer: NativeSigner,
    id: ObjectId,
}
// This helper is confined to patches and accepts only an upstream loaded patch.
// T never borrows the transaction. Concrete public envelopes stay independent.
type LoadedPatch<'a, 'b, 'c> = patch::PatchMut<
    'a,
    'b,
    'c,
    radicle::storage::git::Repository,
    radicle::profile::Signer,
    NoCache,
>;
impl NativePatchMut {
    fn read(&self) -> Result<Patch> {
        let repo = self.repo.open()?;
        let store = RawPatches::open(&repo, ReadOnly).map_err(|e| err("COB", e))?;
        let p = store
            .get(&self.id)
            .map_err(|e| err("COB", e))?
            .ok_or_else(|| err("COB", "patch not found"))?;
        Ok(snapshot(&self.id, &p))
    }
    fn change<T>(
        &self,
        operation: impl FnOnce(&mut LoadedPatch<'_, '_, '_>) -> Result<T>,
    ) -> Result<(T, Patch)> {
        let lock = repo_lock(&self.repo)?;
        let _guard = lock.lock().map_err(|e| err("INTERNAL", e))?;
        let repo = self.repo.open()?;
        let sg = self.signer.inner.lock().map_err(|e| err("INTERNAL", e))?;
        let mut store = RawPatches::open(&repo, WriteAs::new(&*sg)).map_err(|e| err("COB", e))?;
        let mut cache = NoCache;
        let mut p = store
            .get_mut(&self.id, &mut cache)
            .map_err(|e| err("COB", e))?;
        let result = operation(&mut p)?;
        Ok((result, snapshot(&self.id, &p)))
    }
    fn entry(
        &self,
        operation: impl FnOnce(&mut LoadedPatch<'_, '_, '_>) -> Result<String>,
    ) -> Result<PatchChange> {
        let (entry_id, snapshot) = self.change(operation)?;
        Ok(PatchChange { entry_id, snapshot })
    }
    fn transition(&self, transition: Transition) -> Result<PatchStateChange> {
        let (changed, snapshot) = self.change(|p| {
            match transition {
                Transition::Ready => p.ready(),
                Transition::Unready => p.unready(),
                Transition::Archive => p.archive(),
                Transition::Unarchive => p.unarchive(),
            }
            .map_err(|e| err("COB", e))
        })?;
        Ok(PatchStateChange { changed, snapshot })
    }
}
enum Transition {
    Ready,
    Unready,
    Archive,
    Unarchive,
}
#[napi]
impl NativePatchMut {
    #[napi(getter, catch_unwind)]
    pub fn id(&self) -> String {
        self.id.to_string()
    }
    #[napi(catch_unwind, ts_return_type = "Promise<Patch>")]
    pub fn reload(&self) -> AsyncTask<Work<Patch>> {
        let me = self.clone();
        work(move || me.read())
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchChange>")]
    pub fn edit(&self, options: EditPatchOptions) -> AsyncTask<Work<PatchChange>> {
        let me = self.clone();
        work(move || {
            let title = Title::new(&options.title).map_err(|e| err("ARG_VALUE", e))?;
            let target = target(&options.target)?;
            me.entry(|p| {
                p.edit(title, target)
                    .map(|id| id.to_string())
                    .map_err(|e| err("COB", e))
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchChange>")]
    pub fn label(
        &self,
        #[napi(ts_arg_type = "ReadonlyArray<string>")] labels: Vec<String>,
    ) -> AsyncTask<Work<PatchChange>> {
        let me = self.clone();
        work(move || {
            let labels = parse_labels(labels)?;
            me.entry(|p| {
                p.label(labels)
                    .map(|id| id.to_string())
                    .map_err(|e| err("COB", e))
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchChange>")]
    pub fn assign(
        &self,
        #[napi(ts_arg_type = "ReadonlyArray<string>")] assignees: Vec<String>,
    ) -> AsyncTask<Work<PatchChange>> {
        let me = self.clone();
        work(move || {
            let assignees: BTreeSet<_> = parse_dids(assignees)?.into_iter().collect();
            me.entry(|p| {
                p.assign(assignees)
                    .map(|id| id.to_string())
                    .map_err(|e| err("COB", e))
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchRevisionChange>")]
    pub fn update(&self, options: UpdatePatchOptions) -> AsyncTask<Work<PatchRevisionChange>> {
        let me = self.clone();
        work(move || {
            let base = *oid(&options.base)?;
            let head = *oid(&options.head)?;
            let (revision, snapshot) = me.change(|p| {
                p.update(options.description, base, head)
                    .map_err(|e| err("COB", e))
            })?;
            Ok(PatchRevisionChange {
                revision_id: revision.to_string(),
                snapshot,
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchStateChange>")]
    pub fn ready(&self) -> AsyncTask<Work<PatchStateChange>> {
        let me = self.clone();
        work(move || me.transition(Transition::Ready))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchStateChange>")]
    pub fn unready(&self) -> AsyncTask<Work<PatchStateChange>> {
        let me = self.clone();
        work(move || me.transition(Transition::Unready))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchStateChange>")]
    pub fn archive(&self) -> AsyncTask<Work<PatchStateChange>> {
        let me = self.clone();
        work(move || me.transition(Transition::Archive))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchStateChange>")]
    pub fn unarchive(&self) -> AsyncTask<Work<PatchStateChange>> {
        let me = self.clone();
        work(move || me.transition(Transition::Unarchive))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchChange>")]
    pub fn comment(
        &self,
        revision: String,
        body: String,
        options: PatchCommentOptions,
    ) -> AsyncTask<Work<PatchChange>> {
        let me = self.clone();
        work(move || {
            let revision = revision_id(&revision)?;
            let reply = options
                .reply_to
                .map(|s| oid(&s).map(|id| *id))
                .transpose()?;
            me.entry(|p| {
                p.comment(revision, body, reply, None, [])
                    .map(|id| id.to_string())
                    .map_err(|e| err("COB", e))
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchChange>")]
    pub fn review(
        &self,
        revision: String,
        options: ReviewPatchOptions,
    ) -> AsyncTask<Work<PatchChange>> {
        let me = self.clone();
        work(move || {
            let revision = revision_id(&revision)?;
            let verdict = match options.verdict.as_deref() {
                Some("accept") => Some(patch::Verdict::Accept),
                Some("reject") => Some(patch::Verdict::Reject),
                None => None,
                _ => return Err(err("ARG_VALUE", "verdict must be 'accept' or 'reject'")),
            };
            me.entry(|p| {
                p.review(revision, verdict, options.summary, vec![])
                    .map(|id| id.to_string())
                    .map_err(|e| err("COB", e))
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<PatchChange>")]
    pub fn merge(&self, revision: String, commit: String) -> AsyncTask<Work<PatchChange>> {
        let me = self.clone();
        work(move || {
            let revision = revision_id(&revision)?;
            let commit = *oid(&commit)?;
            me.entry(|p| {
                p.merge(revision, commit)
                    .map(|merged| merged.entry.to_string())
                    .map_err(|e| err("COB", e))
            })
        })
    }
}
