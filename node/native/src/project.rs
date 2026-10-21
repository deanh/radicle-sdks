//! Local project init, fork and checkout; Git owns commits and import.
use crate::{
    error::err,
    tasks::{Work, work},
};
use crate::{
    ids::rid,
    locks::repo_lock,
    profile::NativeSigner,
    repository::{NativeRepository, NativeStorage},
};
use napi::bindgen_prelude::AsyncTask;
use napi_derive::napi;
/// Unverified working-copy fetch-remote metadata; not a storage handle.
#[napi(object)]
pub struct ProjectLocation {
    #[napi(readonly)]
    pub rid: String,
    #[napi(readonly)]
    pub remote_name: String,
    #[napi(readonly)]
    pub namespace: Option<String>,
}

#[napi(catch_unwind, ts_return_type = "Promise<ProjectLocation | null>")]
pub fn project_at(path: String, remote_name: String) -> AsyncTask<Work<Option<ProjectLocation>>> {
    work(move || {
        use radicle::git;
        if !git::raw::Remote::is_valid_name(&remote_name) {
            return Err(err(
                "ARG_VALUE",
                "remoteName must be a valid Git remote name",
            ));
        }
        std::fs::metadata(&path).map_err(|e| err("GIT", e))?;
        let repo = match git::raw::Repository::discover(path) {
            Ok(repo) => repo,
            Err(e) if e.code() == git::raw::ErrorCode::NotFound => return Ok(None),
            Err(e) => return Err(err("GIT", e)),
        };
        let remote = match repo.find_remote(&remote_name) {
            Ok(remote) => remote,
            Err(e) if e.code() == git::raw::ErrorCode::NotFound => {
                // NotFound can mean a configured remote without a fetch URL.
                let config = repo.config().map_err(|e| err("GIT", e))?;
                let mut entries = config.entries(None).map_err(|e| err("GIT", e))?;
                let prefix = format!("remote.{remote_name}.");
                while let Some(entry) = entries.next() {
                    if entry
                        .map_err(|e| err("GIT", e))?
                        .name_bytes()
                        .starts_with(prefix.as_bytes())
                    {
                        return Err(err("GIT", e));
                    }
                }
                return Ok(None);
            }
            Err(e) => return Err(err("GIT", e)),
        };
        let url: git::Url = remote
            .url()
            .map_err(|e| err("GIT", e))?
            .parse()
            .map_err(|e| err("GIT", e))?;
        Ok(Some(ProjectLocation {
            rid: url.repo.to_string(),
            remote_name,
            namespace: url.namespace.map(|key| key.to_string()),
        }))
    })
}

#[napi(object)]
pub struct InitOptions {
    pub name: String,
    pub description: String,
    pub default_branch: String,
    #[napi(ts_type = "'public' | 'private'")]
    pub visibility: Option<String>,
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
    bare: bool,
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
            bare,
        )
        .map_err(|e| err("REPOSITORY", e))?;
        Ok(path)
    })
}
