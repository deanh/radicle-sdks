//! Raw local policy tables, not Profile::policies' config-combining facade.
//! Only paths/access modes cross jobs; each connection is opened and dropped on
//! its worker. SQLite's upstream busy timeouts arbitrate writers, without retries.
use crate::{
    error::err,
    ids::rid,
    profile::{NativeProfile, path_string},
    tasks::{Work, work},
};
use napi::{Result, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use radicle::node::{
    NodeId,
    policy::{self, Policy, Scope},
};
use std::path::PathBuf;

type Reader = policy::store::Store<policy::store::Read>;
type Writer = policy::store::Store<policy::store::Write>;

#[napi(object)]
pub struct FollowPolicyInfo {
    #[napi(readonly)]
    pub node_id: String,
    #[napi(readonly)]
    pub alias: Option<String>,
    #[napi(readonly, ts_type = "'allow' | 'block'")]
    pub policy: String,
}
#[napi(object)]
pub struct SeedPolicyInfo {
    #[napi(readonly)]
    pub rid: String,
    #[napi(readonly, ts_type = "'allow' | 'block'")]
    pub policy: String,
    #[napi(readonly, ts_type = "'all' | 'followed'")]
    pub scope: Option<String>,
}
fn follow_info(value: policy::FollowPolicy) -> FollowPolicyInfo {
    FollowPolicyInfo {
        node_id: value.nid.to_string(),
        alias: value.alias.map(|a| a.to_string()),
        policy: match value.policy {
            Policy::Allow => "allow",
            Policy::Block => "block",
        }
        .into(),
    }
}
fn seed_info(value: policy::SeedPolicy) -> SeedPolicyInfo {
    let (policy, scope) = match value.policy {
        policy::SeedingPolicy::Allow { scope } => (
            "allow",
            Some(
                match scope {
                    Scope::All => "all",
                    Scope::Followed => "followed",
                }
                .into(),
            ),
        ),
        policy::SeedingPolicy::Block => ("block", None),
    };
    SeedPolicyInfo {
        rid: value.rid.to_string(),
        policy: policy.into(),
        scope,
    }
}
fn nid(value: &str) -> Result<NodeId> {
    value.parse().map_err(|e| err("ARG_VALUE", e))
}
fn parse_policy(value: &str) -> Result<Policy> {
    match value {
        "allow" => Ok(Policy::Allow),
        "block" => Ok(Policy::Block),
        _ => Err(err("ARG_VALUE", "policy must be 'allow' or 'block'")),
    }
}
fn scope(value: &str) -> Result<Scope> {
    match value {
        "all" => Ok(Scope::All),
        "followed" => Ok(Scope::Followed),
        _ => Err(err("ARG_VALUE", "scope must be 'all' or 'followed'")),
    }
}
#[napi]
#[derive(Clone)]
pub struct NativePolicyStore {
    path: PathBuf,
    read_only: bool,
}
impl NativePolicyStore {
    fn create(path: PathBuf, read_only: bool) -> Result<Self> {
        // Per-operation connections require a persistent file, not SQLite's
        // temporary/in-memory connection names. Paths are filesystem paths, not URIs.
        if path.as_os_str().is_empty() || path == std::path::Path::new(":memory:") {
            return Err(err(
                "ARG_VALUE",
                "policy store requires a persistent file path",
            ));
        }
        let path = std::path::absolute(path).map_err(|e| err("POLICY", e))?;
        // Eager validation/migration at the factory, but no connection retained.
        if read_only {
            Reader::reader(&path).map_err(|e| err("POLICY", e))?;
        } else {
            Writer::open(&path).map_err(|e| err("POLICY", e))?;
        }
        // Freeze relative paths/symlinks at factory time before reopening later.
        let path = path.canonicalize().map_err(|e| err("POLICY", e))?;
        Ok(Self { path, read_only })
    }
    fn read<T>(&self, operation: impl FnOnce(&Reader) -> Result<T>) -> Result<T> {
        // Even reads through a writable handle cannot recreate a deleted file.
        let store = Reader::reader(&self.path).map_err(|e| err("POLICY", e))?;
        operation(&store)
    }
    fn write(
        &self,
        operation: impl FnOnce(&mut Writer) -> std::result::Result<bool, policy::store::Error>,
    ) -> Result<bool> {
        if self.read_only {
            return Err(err("READ_ONLY", "policy store is read-only"));
        }
        let mut store = Writer::open(&self.path).map_err(|e| err("POLICY", e))?;
        operation(&mut store).map_err(|e| err("POLICY", e))
    }
}
#[napi]
impl NativePolicyStore {
    #[napi(catch_unwind, ts_return_type = "Promise<NativePolicyStore>")]
    pub fn from_profile(profile: &NativeProfile) -> AsyncTask<Work<NativePolicyStore>> {
        let path = profile.inner.node().join(radicle::node::POLICIES_DB_FILE);
        work(move || Self::create(path, false))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativePolicyStore>")]
    pub fn reader(profile: &NativeProfile) -> AsyncTask<Work<NativePolicyStore>> {
        let path = profile.inner.node().join(radicle::node::POLICIES_DB_FILE);
        work(move || Self::create(path, true))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativePolicyStore>")]
    pub fn open(path: String) -> AsyncTask<Work<NativePolicyStore>> {
        work(move || Self::create(path.into(), false))
    }
    #[napi(getter, catch_unwind)]
    pub fn path(&self) -> Result<String> {
        path_string(&self.path)
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn follow(&self, node_id: String, alias: Option<String>) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = nid(&node_id)?;
            let alias = alias
                .map(|v| v.parse().map_err(|e| err("ARG_VALUE", e)))
                .transpose()?;
            me.write(|s| s.follow(&id, alias.as_ref()))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn unfollow(&self, node_id: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = nid(&node_id)?;
            me.write(|s| s.unfollow(&id))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn seed(&self, repo_id: String, value: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = rid(&repo_id)?;
            let scope = scope(&value)?;
            me.write(|s| s.seed(&id, scope))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn unseed(&self, repo_id: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = rid(&repo_id)?;
            me.write(|s| s.unseed(&id))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn set_follow_policy(&self, node_id: String, value: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = nid(&node_id)?;
            let policy = parse_policy(&value)?;
            me.write(|s| s.set_follow_policy(&id, policy))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn set_seed_policy(&self, repo_id: String, value: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = rid(&repo_id)?;
            let policy = parse_policy(&value)?;
            me.write(|s| s.set_seed_policy(&id, policy))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn unblock_rid(&self, repo_id: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = rid(&repo_id)?;
            me.write(|s| s.unblock_rid(&id))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn unblock_nid(&self, node_id: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = nid(&node_id)?;
            me.write(|s| s.unblock_nid(&id))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn is_following(&self, node_id: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = nid(&node_id)?;
            me.read(|s| s.is_following(&id).map_err(|e| err("POLICY", e)))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn is_blocked(&self, node_id: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = nid(&node_id)?;
            me.read(|s| s.is_blocked(&id).map_err(|e| err("POLICY", e)))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn is_seeding(&self, repo_id: String) -> AsyncTask<Work<bool>> {
        let me = self.clone();
        work(move || {
            let id = rid(&repo_id)?;
            me.read(|s| s.is_seeding(&id).map_err(|e| err("POLICY", e)))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<FollowPolicyInfo | null>")]
    pub fn follow_policy(&self, node_id: String) -> AsyncTask<Work<Option<FollowPolicyInfo>>> {
        let me = self.clone();
        work(move || {
            let id = nid(&node_id)?;
            me.read(|s| {
                s.follow_policy(&id)
                    .map(|p| p.map(follow_info))
                    .map_err(|e| err("POLICY", e))
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<SeedPolicyInfo | null>")]
    pub fn seed_policy(&self, repo_id: String) -> AsyncTask<Work<Option<SeedPolicyInfo>>> {
        let me = self.clone();
        work(move || {
            let id = rid(&repo_id)?;
            me.read(|s| {
                s.seed_policy(&id)
                    .map(|p| p.map(seed_info))
                    .map_err(|e| err("POLICY", e))
            })
        })
    }
    #[napi(
        catch_unwind,
        ts_return_type = "Promise<ReadonlyArray<FollowPolicyInfo>>"
    )]
    pub fn follow_policies(&self) -> AsyncTask<Work<Vec<FollowPolicyInfo>>> {
        let me = self.clone();
        work(move || {
            me.read(|s| {
                s.follow_policies()
                    .map_err(|e| err("POLICY", e))?
                    .map(|p| p.map(follow_info).map_err(|e| err("POLICY", e)))
                    .collect()
            })
        })
    }
    #[napi(
        catch_unwind,
        ts_return_type = "Promise<ReadonlyArray<SeedPolicyInfo>>"
    )]
    pub fn seed_policies(&self) -> AsyncTask<Work<Vec<SeedPolicyInfo>>> {
        let me = self.clone();
        work(move || {
            me.read(|s| {
                s.seed_policies()
                    .map_err(|e| err("POLICY", e))?
                    .map(|p| p.map(seed_info).map_err(|e| err("POLICY", e)))
                    .collect()
            })
        })
    }
    #[napi(
        catch_unwind,
        ts_return_type = "Promise<ReadonlyArray<readonly [string, string]>>"
    )]
    pub fn nodes_by_alias(&self, alias: String) -> AsyncTask<Work<Vec<Vec<String>>>> {
        let me = self.clone();
        work(move || {
            let alias = alias.parse().map_err(|e| err("ARG_VALUE", e))?;
            me.read(|s| {
                s.nodes_by_alias(&alias)
                    .map_err(|e| err("POLICY", e))?
                    .map(|p| {
                        p.map(|(id, a)| vec![id.to_string(), a.to_string()])
                            .map_err(|e| err("POLICY", e))
                    })
                    .collect()
            })
        })
    }
}
