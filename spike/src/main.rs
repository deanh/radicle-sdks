//! Phase 0A feasibility spikes for `radicle-py` — throwaway code, NOT the binding.
//!
//! Run: `cargo run` (prints evidence per spike; exits non-zero on failure).
//!
//! Spike goals (see PLAN.md §7, Phase 0A):
//!   S2: deterministic temp profile — explicit seed → Profile::init; determinism check;
//!       getrandom fill (for Profile.init()'s secure seed path).
//!   S3: create a project (public `rad::init`), read head, hand-convert one commit + one
//!       tree, including a non-UTF-8 file path (evidence for the §5.4 policy).
//!   S4: create an issue via the typed store, then inspect COBs raw: types/entities,
//!       refs, entries and raw ops (Op<Vec<u8>>) — the runtime-generic path.

use anyhow::{Context, Result, anyhow};
use radicle::cob::issue::Issues;
use radicle::cob::store::access::WriteAs;
use radicle::crypto::Signer as _;
use radicle::git::Oid;
use radicle::prelude::*;
use radicle::profile::Home;
use radicle::{Storage, git, rad};

fn alias() -> Alias {
    "spike".parse::<Alias>().expect("valid alias")
}

fn to_raw_oid(oid: Oid) -> Result<git::raw::Oid> {
    let hex = oid.to_string();
    git::raw::Oid::from_str(&hex).map_err(|e| anyhow!("oid conversion: {e}"))
}

// ─── S2: deterministic temp profile ──────────────────────────────────────────

fn s2_profile() -> Result<radicle::Profile> {
    println!("== S2: deterministic temp profile ==");

    let tmp = tempfile::tempdir()?.into_path();
    let home = Home::new(tmp.join("rad"))?;

    let seed = || radicle::crypto::Seed::new([7u8; 32]);
    let profile = radicle::Profile::init(home, alias(), None, seed())
        .context("Profile::init")?;
    println!("  did      = {}", profile.did());
    println!("  node_id  = {}", profile.id());

    // Determinism: same seed, different home → same key.
    let home2 = Home::new(tempfile::tempdir()?.into_path().join("rad"))?;
    let profile2 = radicle::Profile::init(home2, alias(), None, seed())
        .context("Profile::init (2)")?;
    if profile.did() != profile2.did() {
        return Err(anyhow!("determinism FAILED: {} != {}", profile.did(), profile2.did()));
    }
    println!("  determinism: OK (same seed → same DID)");

    // Signer loads unencrypted (passphrase None), no env vars needed.
    let signer = profile.signer().context("profile.signer()")?;
    println!("  signer key = {} (matches profile node id: {})",
        signer.public_key(), signer.public_key() == profile.id());

    // Secure seed fill for the future public Profile.init() (PLAN §5.7): OS RNG, 32 bytes.
    let mut buf = [0u8; 32];
    let t = std::time::Instant::now();
    for _ in 0..1000 {
        getrandom::fill(&mut buf).map_err(|e| anyhow!("getrandom: {e}"))?;
    }
    println!("  getrandom::fill([u8;32]) x1000: {:?} — usable for Profile.init() secure seed", t.elapsed());

    Ok(profile)
}

// ─── helpers: plain git repo with a non-UTF-8 tree entry ─────────────────────

fn make_git_repo(path: &std::path::Path) -> Result<git::raw::Repository> {
    std::fs::create_dir_all(path)?;
    let repo = git::raw::Repository::init(path)?;

    let blob = repo.blob(b"hello spike\n")?;
    let mut tb = repo.treebuilder(None)?;
    tb.insert("hello.txt", blob, 0o100644)?;
    // Non-UTF-8 path (evidence for PLAN §5.4): raw bytes in a filename.
    let weird = std::path::Path::new(<&std::ffi::OsStr>::from(
        std::os::unix::ffi::OsStrExt::from_bytes(b"w\xc3\xa9ird-\xff\xfe.bin"),
    ));
    tb.insert(weird, blob, 0o100644)?;
    let tree = tb.write()?;
    drop(tb);

    let sig = git::raw::Signature::now("Spike", "spike@example.com")?;
    {
        let tree = repo.find_tree(tree)?;
        repo.commit(Some("refs/heads/master"), &sig, &sig, "initial commit\n", &tree, &[])?;
    }
    repo.set_head("refs/heads/master")?;
    Ok(repo)
}

// ─── S3: storage, project init, commit + tree conversion ─────────────────────

fn s3_repo_storage(profile: &radicle::Profile) -> Result<RepoId> {
    println!("\n== S3: storage + project + hand conversions ==");

    let tmp = tempfile::tempdir()?;
    let storage: &Storage = &profile.storage;
    println!("  storage path = {}", storage.path().display());

    let signer = profile.signer()?;

    let wd = tmp.path().join("workdir");
    let gitrepo = make_git_repo(&wd)?;

    // Public initialization path (no test features): rad::init.
    let (rid, doc, refs) = rad::init(
        &gitrepo,
        "spike-repo".try_into()?,
        "spike repo",
        radicle::git::fmt::RefStr::try_from_str("master")?.to_ref_string(),
        radicle::identity::Visibility::default(),
        &signer,
        storage,
    )
    .context("rad::init")?;
    println!("  rid       = {}", rid);
    println!("  delegates = {:?}", doc.delegates().iter().collect::<Vec<_>>());
    println!("  signed refs = {} entries", refs.iter().count());

    // Reopen through storage and read via the binding's intended path.
    let repo = storage.repository(rid)?;
    let (head_ref, head_oid) = ReadRepository::head(&repo)?;
    println!("  head      = {} → {}", head_ref, head_oid);

    // Hand-convert the head commit (PLAN §5.4 conversion schema evidence).
    let commit = repo.backend.find_commit(to_raw_oid(head_oid)?)?;
    let msg_bytes: &[u8] = commit.message_bytes();
    println!("  commit    = {}", head_oid);
    println!("    author    = name={:?} email={:?} time={} offset_min={}",
        commit.author().name_bytes(), commit.author().email_bytes(),
        commit.author().when().seconds(), commit.author().when().offset_minutes());
    println!("    committer name matches author: {}",
        commit.author().name_bytes() == commit.committer().name_bytes());
    println!("    message bytes = {:?} (lossy str = {:?})", msg_bytes, commit.message());
    println!("    tree = {}  parents = {:?}", commit.tree_id(),
        commit.parent_ids().collect::<Vec<_>>());

    // Hand-convert the tree: ordering + modes + non-UTF-8 name evidence.
    let tree = commit.tree()?;
    let mut entries = Vec::new();
    for e in tree.iter() {
        entries.push((
            e.name_bytes().to_vec(),
            format!("{:?}", e.kind()),
            e.filemode(),
            e.id().to_string(),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, kind, mode, oid) in &entries {
        println!("    entry: name_bytes={name:?} kind={kind} mode={mode:#o} oid={oid}");
    }
    let has_invalid_utf8 = entries.iter().any(|(n, ..)| std::str::from_utf8(n).is_err());
    println!("  non-UTF-8 path survived git + read-back: {has_invalid_utf8} → §5.4 needs bytes-first fields");

    Ok(rid)
}

// ─── S4: write an issue (typed), then inspect COBs raw ───────────────────────

fn s4_cob_raw(profile: &radicle::Profile, rid: RepoId) -> Result<()> {
    println!("\n== S4: typed issue write + raw COB inspection ==");

    let storage = &profile.storage;
    let repo = storage.repository(rid)?;
    let signer = profile.signer()?;

    // Typed write (Phase 4's typed adapter shape).
    let mut issues = Issues::open(&repo, WriteAs::new(&signer))?;
    let mut nocache = radicle::cob::cache::NoCache;
    let created = issues.create(
        "spike issue".parse()?,
        "hello from the spike",
        &[], &[], [],
        &mut nocache,
    )?;
    let iid = *created.id();
    let title = created.title().to_owned();
    drop(created);
    println!("  created issue {iid} — title={title:?}");

    // Reopen read-only; raw layer below (PLAN §3 `Raw`).
    drop(issues);
    let repo = storage.repository(rid)?;

    use radicle::cob::change::store::Storage as _; // load / parents_of
    use radicle::cob::object::Storage as _;        // types / objects

    // Raw entities: `types(typename)` — entities + their refs per typename.
    let typename = radicle::cob::issue::TYPENAME.clone();
    let entities = repo.types(&typename)?;
    println!("  entities of {:?}: {}", typename.as_str(), entities.len());
    for (eid, objects) in &entities {
        println!("    entity {eid}: {} ref(s)", objects.iter().count());
    }
    if !entities.contains_key(&iid) {
        return Err(anyhow!("created issue {iid} not found via raw types()"));
    }

    // Raw history for the entity: refs → change commits → entries → raw ops.
    let objects = repo.objects(&typename, &iid)?;
    let mut seen_entries = 0usize;
    for reference in objects.iter() {
        let tip = reference.target.id;
        println!("  ref {} → change commit {tip}", reference.name);
        // Walk the change commit graph (parents) from each ref tip.
        let mut stack = vec![tip];
        let mut visited = vec![];
        while let Some(oid) = stack.pop() {
            if visited.contains(&oid) {
                continue;
            }
            visited.push(oid);

            // Raw entry decode (what `Raw.history` would return per item).
            // NOTE: `Op<Vec<u8>>::try_from` is WRONG for raw — it deserializes each action blob
            // (serde can't map a JSON object into Vec<u8>). The raw representation is the entry
            // itself: contents().iter() yields ONE raw JSON action blob per action.
            let entry = repo.load(oid)
                .with_context(|| format!("load entry {oid}"))?;
            let actions: Vec<&[u8]> = entry.contents().iter().map(|b| b.as_slice()).collect();
            println!("    entry {} author={} timestamp={} actions={} parents={} related={} identity={:?}",
                entry.id(), entry.author(), entry.timestamp, actions.len(),
                entry.parents.len(), entry.related.len(), entry.resource());
            for (i, action) in actions.iter().enumerate() {
                let json: serde_json::Value = serde_json::from_slice(action)
                    .with_context(|| format!("action #{i} of entry {}", entry.id()))?;
                println!("      action #{i} ({} bytes): type={}",
                    action.len(), json.get("type").and_then(|t| t.as_str()).unwrap_or("?"));
            }
            seen_entries += 1;

            // Parents via git (change commits are the entries).
            let commit = repo.backend.find_commit(to_raw_oid(oid)?)?;
            for p in commit.parent_ids() {
                stack.push(Oid::try_from(p).map_err(|e| anyhow!("{e}"))?);
            }
        }
    }
    if seen_entries == 0 {
        return Err(anyhow!("raw history walk found no entries"));
    }
    println!("  raw walk: {seen_entries} entries decoded via entry.contents() (per-action JSON blobs)");

    Ok(())
}

fn main() -> Result<()> {
    let profile = s2_profile()?;
    let rid = s3_repo_storage(&profile)?;
    s4_cob_raw(&profile, rid)?;
    println!("\nALL SPIKES OK");
    Ok(())
}
