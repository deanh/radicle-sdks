//! TEST-ONLY helpers (cargo feature `test-helpers`) — compiled out of
//! release wheels, exactly like `Profile._from_seed`.
//!
//! The repository/COB reader tests predate the public write API, so their
//! fixtures create repositories through these helpers. Content is fully
//! deterministic given the profile, so tests can assert exact OIDs where
//! useful (commit timestamps come from the real clock, so most assertions
//! are structural instead).

use std::path::Path;
use std::str::FromStr;

use pyo3::prelude::*;
use radicle::git;
use radicle::identity::RepoId;
use radicle::node::{self, Alias};
use radicle::storage::{ReadStorage, RemoteId};

use crate::error;
use crate::profile::PyProfile;

/// Deterministic file set for the initial commit: (path, content).
fn fixture_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (
            "README.md",
            b"# fixture\n\nhermetic test repository\n".as_slice(),
        ),
        ("zoo.txt", b"z\n".as_slice()),
        ("assets/logo.svg", b"<svg/>\n".as_slice()),
    ]
}

fn commit_files(
    repo: &git::raw::Repository,
    files: &[(&str, &[u8])],
    message: &str,
    branch: &str,
) -> PyResult<git::raw::Oid> {
    let sig = git::raw::Signature::now("radiclepy-fixture", "fixture@example.com")
        .map_err(|e| error::op_err(format!("signature: {e}")))?;

    // Group files by directory, write one subtree per directory, then the
    // root tree. The fixture set nests exactly one level, so no deeper
    // recursion is needed.
    use std::collections::BTreeMap;
    let mut dirs: BTreeMap<&str, Vec<(String, git::raw::Oid, i32)>> = BTreeMap::new();
    for (path, content) in files {
        let oid = repo
            .blob(content)
            .map_err(|e| error::op_err(format!("blob: {e}")))?;
        match path.rsplit_once('/') {
            Some((dir, rest)) => {
                dirs.entry(dir)
                    .or_default()
                    .push((rest.to_string(), oid, 0o100644i32))
            }
            None => dirs
                .entry("")
                .or_default()
                .push((path.to_string(), oid, 0o100644i32)),
        }
    }
    // Sub-trees first.
    let mut root_entries: Vec<(String, git::raw::Oid, i32)> = Vec::new();
    for (dir, entries) in dirs {
        if dir.is_empty() {
            root_entries = entries;
            continue;
        }
        let mut tb = repo
            .treebuilder(None)
            .map_err(|e| error::op_err(format!("treebuilder: {e}")))?;
        for (name, oid, mode) in entries {
            tb.insert(name.as_str(), oid, mode)
                .map_err(|e| error::op_err(format!("insert: {e}")))?;
        }
        let tree_oid = tb
            .write()
            .map_err(|e| error::op_err(format!("tree write: {e}")))?;
        drop(tb);
        root_entries.push((dir.to_string(), tree_oid, 0o040000i32));
    }
    let mut tb = repo
        .treebuilder(None)
        .map_err(|e| error::op_err(format!("treebuilder: {e}")))?;
    for (name, oid, mode) in root_entries {
        tb.insert(name.as_str(), oid, mode)
            .map_err(|e| error::op_err(format!("insert: {e}")))?;
    }
    let tree_oid = tb
        .write()
        .map_err(|e| error::op_err(format!("tree write: {e}")))?;
    drop(tb);
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| error::op_err(format!("find tree: {e}")))?;

    let parents: Vec<git::raw::Commit> = match repo.head() {
        Ok(head) => head.peel_to_commit().map(|c| vec![c]).unwrap_or_default(),
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git::raw::Commit> = parents.iter().collect();
    let cid = repo
        .commit(
            Some(&format!("refs/heads/{branch}")),
            &sig,
            &sig,
            message,
            &tree,
            &parent_refs,
        )
        .map_err(|e| error::op_err(format!("commit: {e}")))?;
    Ok(cid)
}

/// TEST-ONLY: create a workdir repo with one deterministic commit and
/// initialize it into `profile`'s storage. Returns the new repo id string.
#[pyfunction]
#[pyo3(signature = (profile, workdir, name, description = "fixture repo", default_branch = "master"))]
pub fn init_project(
    profile: PyRef<'_, PyProfile>,
    workdir: &str,
    name: &str,
    description: &str,
    default_branch: &str,
) -> PyResult<String> {
    let path = Path::new(workdir);
    std::fs::create_dir_all(path).map_err(|e| error::op_err(format!("workdir: {e}")))?;
    let repo =
        git::raw::Repository::init(path).map_err(|e| error::op_err(format!("git init: {e}")))?;

    let files = fixture_files();
    let file_refs: Vec<(&str, &[u8])> = files.iter().map(|(p, c)| (*p, *c)).collect();
    commit_files(
        &repo,
        &file_refs,
        "fixture: initial commit\n",
        default_branch,
    )?;

    let branch = git::fmt::RefStr::try_from_str(default_branch)
        .map_err(|e| error::value_err(format!("invalid branch: {e}")))?
        .to_ref_string();
    let (rid, _doc, _refs) = radicle::rad::init(
        &repo,
        name.try_into()
            .map_err(|e| error::value_err(format!("invalid name: {e}")))?,
        description,
        branch,
        radicle::identity::Visibility::default(),
        &profile.inner.signer().map_err(error::signer_err)?,
        &profile.inner.storage,
    )
    .map_err(|e| error::op_err(format!("rad init: {e}")))?;
    Ok(rid.to_string())
}

/// TEST-ONLY: write a tag into the storage repository's local refs.
/// `annotated` selects an annotated tag object vs a lightweight ref.
#[pyfunction]
#[pyo3(signature = (profile, rid, name, target = "HEAD", annotated = true))]
pub fn create_tag(
    profile: PyRef<'_, PyProfile>,
    rid: &str,
    name: &str,
    target: &str,
    annotated: bool,
) -> PyResult<String> {
    let rid_t =
        RepoId::from_str(rid).map_err(|e| error::value_err(format!("invalid RepoId: {e}")))?;
    let storage = &profile.inner.storage;
    let repo = storage.repository(rid_t).map_err(error::repository_err)?;
    let obj = repo
        .backend
        .revparse_single(target)
        .map_err(|e| error::git_err(format!("revparse {target:?}: {e}")))?;
    let tag_ref = format!("refs/tags/{name}");
    if annotated {
        let sig = git::raw::Signature::now("radiclepy-fixture", "fixture@example.com")
            .map_err(|e| error::op_err(format!("signature: {e}")))?;
        // NB: git2 `tag()` prepends `refs/tags/` itself; pass the short name.
        repo.backend
            .tag(name, &obj, &sig, "fixture tag\n", true)
            .map_err(error::git_err)?;
    } else {
        repo.backend
            .reference(&tag_ref, obj.id(), true, "fixture")
            .map_err(error::git_err)?;
    }
    Ok(tag_ref)
}

/// TEST-ONLY: create an issue in the storage repository via the typed store
/// with deterministic content for reader tests. Returns the new issue id.
#[pyfunction]
#[pyo3(signature = (profile, rid, title, description, labels = None, comments = None))]
pub fn create_issue(
    profile: PyRef<'_, PyProfile>,
    rid: &str,
    title: &str,
    description: &str,
    labels: Option<Vec<String>>,
    comments: Option<Vec<String>>,
) -> PyResult<String> {
    use radicle::cob::issue::Issues;
    use radicle::cob::store::access::WriteAs;
    use radicle::cob::{Label, Title, cache::NoCache};

    let rid_t =
        RepoId::from_str(rid).map_err(|e| error::value_err(format!("invalid RepoId: {e}")))?;
    let title = Title::new(title).map_err(|e| error::value_err(format!("invalid title: {e}")))?;
    let labels: Vec<Label> = labels
        .unwrap_or_default()
        .iter()
        .map(|l| {
            Label::new(l.as_str()).map_err(|e| error::value_err(format!("invalid label: {e}",)))
        })
        .collect::<PyResult<_>>()?;

    let storage = &profile.inner.storage;
    let repo = storage.repository(rid_t).map_err(error::repository_err)?;
    let signer = profile.inner.signer().map_err(error::signer_err)?;
    let mut issues = Issues::open(&repo, WriteAs::new(&signer)).map_err(error::cob_err)?;
    let mut nocache = NoCache;
    let mut created = issues
        .create(title, description, &labels, &[], [], &mut nocache)
        .map_err(error::cob_err)?;
    // Optional follow-up comments → each is a separate COB operation, letting
    // tests exercise multi-op raw history and topological ordering.
    let root_comment = created
        .comments()
        .next()
        .map(|(id, _)| *id)
        .ok_or_else(|| error::op_err("issue has no root comment"))?;
    for body in comments.unwrap_or_default() {
        created
            .comment(body, root_comment, [])
            .map_err(error::cob_err)?;
    }
    Ok(created.id().to_string())
}

/// TEST-ONLY: create a patch in the storage repository (trivial patch whose
/// base and head are the current canonical head commit). Returns the patch id.
#[pyfunction]
#[pyo3(signature = (profile, rid, title, description))]
pub fn create_patch(
    profile: PyRef<'_, PyProfile>,
    rid: &str,
    title: &str,
    description: &str,
) -> PyResult<String> {
    use radicle::cob::patch::{MergeTarget, Patches};
    use radicle::cob::store::access::WriteAs;
    use radicle::cob::{Title, cache::NoCache};
    use radicle::storage::ReadRepository;

    let rid_t =
        RepoId::from_str(rid).map_err(|e| error::value_err(format!("invalid RepoId: {e}")))?;
    let title = Title::new(title).map_err(|e| error::value_err(format!("invalid title: {e}")))?;

    let storage = &profile.inner.storage;
    let repo = storage.repository(rid_t).map_err(error::repository_err)?;
    let (_head_name, head) = ReadRepository::head(&repo).map_err(error::repository_err)?;
    let signer = profile.inner.signer().map_err(error::signer_err)?;
    let mut patches = Patches::open(&repo, WriteAs::new(&signer)).map_err(error::cob_err)?;
    let mut nocache = NoCache;
    let created = patches
        .create(
            title,
            description,
            MergeTarget::Delegates,
            head,
            head,
            &[],
            &mut nocache,
        )
        .map_err(error::cob_err)?;
    Ok(created.id().to_string())
}

/// TEST-ONLY: create an arbitrary git ref `name` pointing at `target`
/// (revspec). Used by tests to plant non-COB refs shaped like COB refs
/// (typenames() false-positive regression).
#[pyfunction]
pub fn create_ref(
    profile: PyRef<'_, PyProfile>,
    rid: &str,
    name: &str,
    target: &str,
) -> PyResult<String> {
    let rid_t =
        RepoId::from_str(rid).map_err(|e| error::value_err(format!("invalid RepoId: {e}")))?;
    let storage = &profile.inner.storage;
    let repo = storage.repository(rid_t).map_err(error::repository_err)?;
    let obj = repo
        .backend
        .revparse_single(target)
        .map_err(|e| error::git_err(format!("revparse {target:?}: {e}")))?;
    repo.backend
        .reference(name, obj.id(), true, "radiclepy-test")
        .map_err(error::git_err)?;
    Ok(name.to_owned())
}

/// TEST-ONLY: create/insert a routing entry into the profile's node
/// database (the public `RoutingStore` is read-only).
#[pyfunction]
#[pyo3(signature = (profile, rid, node_id, timestamp_ms = 1_700_000_000_000))]
pub fn add_routing_entry(
    profile: PyRef<'_, PyProfile>,
    rid: &str,
    node_id: &str,
    timestamp_ms: u64,
) -> PyResult<bool> {
    use radicle::node::routing::Store as _;

    let rid_t =
        RepoId::from_str(rid).map_err(|e| error::value_err(format!("invalid RepoId: {e}")))?;
    let nid = node::NodeId::from_str(node_id)
        .map_err(|e| error::value_err(format!("invalid NodeId: {e}")))?;
    let mut db = profile.inner.database_mut().map_err(error::database_err)?;
    let updated = db
        .add_inventory(
            [&rid_t],
            nid,
            node::Timestamp::try_from(timestamp_ms)
                .map_err(|e| error::value_err(format!("invalid timestamp: {e}")))?,
        )
        .map_err(error::database_err)?;
    Ok(!updated.is_empty())
}

/// TEST-ONLY: fetcher node id of the profile (for remote assertions).
#[pyfunction]
pub fn local_remote_id(profile: PyRef<'_, PyProfile>) -> String {
    let me: RemoteId = *profile.inner.id();
    me.to_string()
}

/// TEST-ONLY: alias helper to surface alias-store seeding (used to assert
/// `Aliases.get(own)` returns the profile alias).
#[pyfunction]
pub fn parse_alias(alias: &str) -> PyResult<String> {
    Ok(Alias::from_str(alias)
        .map_err(|e| error::value_err(format!("invalid Alias: {e}")))?
        .to_string())
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let test_mod = PyModule::new(py, "_test")?;
    test_mod.add_function(wrap_pyfunction!(init_project, &test_mod)?)?;
    test_mod.add_function(wrap_pyfunction!(create_tag, &test_mod)?)?;
    test_mod.add_function(wrap_pyfunction!(create_issue, &test_mod)?)?;
    test_mod.add_function(wrap_pyfunction!(create_patch, &test_mod)?)?;
    test_mod.add_function(wrap_pyfunction!(add_routing_entry, &test_mod)?)?;
    test_mod.add_function(wrap_pyfunction!(create_ref, &test_mod)?)?;
    test_mod.add_function(wrap_pyfunction!(local_remote_id, &test_mod)?)?;
    test_mod.add_function(wrap_pyfunction!(parse_alias, &test_mod)?)?;
    m.add_submodule(&test_mod)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("radiclepy._test", &test_mod)?;
    Ok(())
}
