//! Phase 2 feasibility spikes — throwaway, NOT the binding.
//!
//! S5: canonical tree ordering incl. directories (the 0A-deferred question).
//! S6: tags — lightweight vs annotated: raw target, peel, object kind.
//! S7: history pages via ReadRepository::revwalk (skip/limit semantics).
//! S8: remotes() + references_of() after rad::init.
//! S9: canonical_identity_doc() -> Project fields, default_branch.

use anyhow::{Result, anyhow};
use radicle::git::{self, Oid};
use radicle::prelude::*;
use radicle::storage::{ReadRepository, RemoteRepository};

fn raw(oid: Oid) -> Result<git::raw::Oid> {
    git::raw::Oid::from_str(&oid.to_string()).map_err(|e| anyhow!("{e}"))
}

fn list_refs(repo: &radicle::Storage, rid: RepoId, prefix: &str) -> Result<Vec<(String, Oid)>> {
    let repo = repo.repository(rid)?;
    let mut out = Vec::new();
    for r in repo.backend.references()? {
        let r = r?;
        let name = r.name()?.to_owned();
        if let Some(target) = r.target() {
            if name.starts_with(prefix) {
                out.push((name, Oid::try_from(target)?));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn make_workdir(path: &std::path::Path) -> Result<git::raw::Repository> {
    std::fs::create_dir_all(path)?;
    let r = git::raw::Repository::init(path)?;
    let sig = git::raw::Signature::now("Spike", "s@e.com")?;

    // commit 1: nested structure to probe tree ordering (git sorts a
    // directory entry as if it had a trailing '/').
    let blob = r.blob(b"x\n")?;
    let mut sub = r.treebuilder(None)?;
    sub.insert("inner.txt", blob, 0o100644)?;
    let sub_tree = sub.write()?;
    drop(sub);
    let mut tb = r.treebuilder(None)?;
    tb.insert("zoo", blob, 0o100644)?;
    tb.insert("assets", sub_tree, 0o040000)?; // directory entry
    tb.insert("alpha", blob, 0o100644)?;
    let t1 = tb.write()?;
    drop(tb);
    let c1 = r.commit(Some("refs/heads/master"), &sig, &sig, "c1\n", &r.find_tree(t1)?, &[])?;

    // commit 2: child of c1
    let blob2 = r.blob(b"y\n")?;
    let mut tb2 = r.treebuilder(None)?;
    tb2.insert("second.txt", blob2, 0o100644)?;
    let t2 = tb2.write()?;
    drop(tb2);
    let c2 = r.commit(
        Some("refs/heads/master"),
        &sig,
        &sig,
        "c2\n",
        &r.find_tree(t2)?,
        &[&r.find_commit(c1)?],
    )?;

    // tags on c2: annotated + lightweight; plus an annotated tag on a BLOB
    // (peel target kind != commit).
    r.tag("plain", &r.find_commit(c2)?.as_object().clone(), &sig, "", false)?;
    r.tag("annotated", &r.find_commit(c2)?.as_object().clone(), &sig, "hi\n", true)?;
    let c2first = r.find_commit(c2)?.tree()?.get(0).unwrap().id();
    r.tag("blobtag", &r.find_blob(c2first)?.as_object().clone(), &sig, "blob tag\n", true)?;
    Ok(r)
}

fn main() -> Result<()> {
    let tmp = tempfile::tempdir()?.into_path();
    let home = radicle::profile::Home::new(tmp.join("rad"))?;
    let seed = radicle::crypto::Seed::new([11u8; 32]);
    let profile =
        radicle::Profile::init(home, "spike2".parse::<Alias>().unwrap(), None, seed)?;

    let workdir = make_workdir(&tmp.join("wd"))?;
    let (rid, _doc, _refs) = radicle::rad::init(
        &workdir,
        "phase2".try_into()?,
        "phase 2 spike repo",
        radicle::git::fmt::RefStr::try_from_str("master")?.to_ref_string(),
        radicle::identity::Visibility::default(),
        &profile.signer()?,
        &profile.storage,
    )?;
    let repo = profile.storage.repository(rid)?;

    // ── S9: identity doc ────────────────────────────────────────────────────
    println!("== S9: identity doc ==");
    let doc = repo.canonical_identity_doc()?;
    let project = doc.project()?;
    println!(
        "  project: name={:?} desc={:?} default_branch={:?}",
        project.name().to_string(),
        project.description(),
        project.default_branch().as_str(),
    );
    println!(
        "  doc: commit={} blob={} version={}",
        doc.commit, doc.blob, doc.version().number()
    );
    println!(
        "  delegates={:?}",
        doc.delegates().iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    println!("  default_branch()={}", ReadRepository::default_branch(&repo)?.as_str());

    // ── S5: tree ordering (directories!) ───────────────────────────────────
    println!("\n== S5: tree ordering ==");
    let (_name, head_oid) = ReadRepository::head(&repo)?;
    let head_commit = repo.backend.find_commit(raw(head_oid)?)?;
    // The interesting trees are on the FIRST commit.
    let c1 = head_commit.parent(0)?;
    let tree = c1.tree()?;
    let mut walk_order: Vec<Vec<u8>> = Vec::new();
    for e in tree.iter() {
        walk_order.push(e.name_bytes().to_vec());
        println!(
            "    git2-walk: {:?} kind={:?} mode={:#o}",
            e.name_bytes(),
            e.kind(),
            e.filemode()
        );
    }
    let mut plain_sorted = walk_order.clone();
    plain_sorted.sort();
    println!(
        "  git walk == naive byte-sort (names as-is)? {}",
        walk_order == plain_sorted
    );
    // The trailing-slash quirk needs a prefix collision: dir "foo" vs file "foo!".
    {
        let b = &repo.backend;
        let blob = b.blob(b"q\n")?;
        let mut sub = b.treebuilder(None)?;
        sub.insert("i", blob, 0o100644)?;
        let foo_dir = sub.write()?;
        drop(sub);
        let mut tb = b.treebuilder(None)?;
        tb.insert("foo!", blob, 0o100644)?;
        tb.insert("foo", foo_dir, 0o040000)?;
        let t = tb.write()?;
        drop(tb);
        println!("  quirk probe — git canonical order for dir 'foo' + file 'foo!':");
        let tree2 = b.find_tree(t)?;
        let names: Vec<_> = tree2.iter().map(|e| String::from_utf8_lossy(e.name_bytes()).into_owned()).collect();
        let mut naive = names.clone();
        naive.sort();
        println!("    git:   {names:?}");
        println!("    naive: {naive:?}  (differ: {}) — walk order is the ONLY contract", names != naive);
    }

    // Tag probes run on the STORAGE backend (workdir tags are not replicated
    // by rad::init; the binding reads local refs, so probe those).
    let sig = git::raw::Signature::now("Spike", "s@e.com")?;
    let head_obj = repo.backend.find_object(raw(head_oid)?, None)?;
    // LIGHTWEIGHT tag = a plain ref pointing at the target (git2's tag()
    // always makes annotated tags).
    repo.backend.reference("refs/tags/plain", head_obj.id(), true, "spike")?;
    repo.backend.tag("annotated", &head_obj, &sig, "annotated tag\n", true)?;
    let blob_oid = repo.backend.blob(b"tagged blob\n")?;
    let blob_obj = repo.backend.find_object(blob_oid, None)?;
    repo.backend.tag("blobtag", &blob_obj, &sig, "blob tag\n", true)?;

    // ── S6: tags — raw vs peeled vs kind ───────────────────────────────────
    println!("\n== S6: tags ==");
    for (name, oid) in list_refs(&profile.storage, rid, "refs/tags/")? {
        let obj = repo.backend.find_object(raw(oid)?, None)?;
        let kind = obj.kind();
        // ObjectType::Any = peel the whole tag chain (full peel).
        match obj.peel(git::raw::ObjectType::Any) {
            Ok(p) => println!(
                "  {name}: raw={oid} raw_kind={kind:?} peeled={} peeled_kind={:?}",
                p.id(),
                p.kind()
            ),
            Err(e) => println!("  {name}: raw={oid} raw_kind={kind:?} peel ERROR: {e}"),
        }
    }

    // ── S7: history pages (skip/limit) ─────────────────────────────────────
    println!("\n== S7: history pages (revwalk skip/limit) ==");
    let all: Vec<String> =
        ReadRepository::revwalk(&repo, head_oid)?.map(|r| r.unwrap().to_string()).collect();
    println!("  full history (default sort): {all:?}");
    let page: Vec<String> = ReadRepository::revwalk(&repo, head_oid)?
        .skip(1)
        .take(1)
        .map(|r| r.unwrap().to_string())
        .collect();
    println!("  page(skip=1, limit=1): {page:?}");
    if page.len() != 1 || page[0] != all[1] {
        return Err(anyhow!("skip/limit semantics not as expected"));
    }

    // ── S8: remotes ────────────────────────────────────────────────────────
    println!("\n== S8: remotes ==");
    // Note: the inherent Repository::remotes() yields an iterator of
    // (PublicKey, Remote) — not the trait's `Remotes` container.
    let remotes: Vec<_> = repo
        .remotes()?
        .collect::<Result<Vec<_>, _>>()?;
    println!("  count={}", remotes.len());
    for (remote_id, remote) in &remotes {
        println!("  remote {remote_id}: {} signed refs", remote.refs.len());
    }
    let me: radicle::storage::RemoteId = *profile.id(); // RemoteId = PublicKey
    let refs = ReadRepository::references_of(&repo, &me)?;
    println!(
        "  references_of(self): {} -> {:?}",
        refs.len(),
        refs.iter().map(|(n, _)| n.as_str().to_owned()).collect::<Vec<_>>()
    );

    // ── branches ───────────────────────────────────────────────────────────
    println!("\n== branches ==");
    for (name, oid) in list_refs(&profile.storage, rid, "refs/heads/")? {
        println!("  {name} -> {oid}");
    }

    println!("\nPHASE-2 SPIKE OK");
    Ok(())
}
