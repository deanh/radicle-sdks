//! Storage discovery and repository reads/writes.
mod values;
use crate::{
    error::err,
    tasks::{Work, work},
};
use crate::{
    ids::rid,
    locks::repo_lock,
    profile::NativeSigner,
    timestamps::{metadata_timestamp, millis},
};
use napi::bindgen_prelude::Uint8Array;
use napi::{Result, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use radicle::{
    identity::RepoId,
    storage::{ReadRepository, ReadStorage, SignRepository},
};
use values::*;
#[napi]
#[derive(Clone)]
pub struct NativeStorage {
    pub(crate) inner: radicle::storage::git::Storage,
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
    pub(crate) storage: radicle::storage::git::Storage,
    pub(crate) rid: RepoId,
}
impl NativeRepository {
    pub(crate) fn open(&self) -> Result<radicle::storage::git::Repository> {
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
            let id = *crate::ids::oid(&oid)?;
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
