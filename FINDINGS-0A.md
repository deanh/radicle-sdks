# Phase 0A — Feasibility Findings

Date: 2026-07-28 · Target: `radicle = "=0.25.1"` (crates.io, authoritative) · Plan: [`../radicle-py-plan.md`](../../radicle-py-plan.md) (in-repo copy: `PLAN.md`)

## S2 — Deterministic temp profile ✅

- `Profile::init(Home::new(path), alias, None, Seed::new([7u8; 32]))` works with **no env vars**
  (no `RAD_HOME`, no `RAD_PASSPHRASE`, no ssh-agent). Unencrypted keystore.
- **Determinism proven**: same seed → same DID across two independent homes
  (`did:key:z6Mkv…5T3Z` both times). Fixtures can hardcode expected identities.
- `profile.signer()` loads with no prompts; `signer.public_key() == profile.id()`.
- Secure seed path for the public `Profile.init()`: `getrandom::fill(&mut [u8; 32])` —
  1000 fills in ~185 µs. Confirms PLAN §5.7 (no `Seed::generate()` upstream; we fill + pass).
- Gotcha: `Seed` is not `Copy`/`Clone`-friendly across `init` calls — construct per call.

## S3 — Storage, project init, hand conversions ✅

- **Public** `rad::init(git_repo, name, desc, branch, visibility, signer, storage)` initializes a
  project with zero test features. Returns `(RepoId, Doc, SignedRefs)`.
- `Storage` obtained via `profile.storage`; `storage.repository(rid)` reopens fine.
- `ReadRepository::head()` → `(Qualified, Oid)` = `refs/heads/master → <oid>` — confirms `HeadInfo(name, oid)`.
- **Commit conversion** (via `repo.backend` git2, all raw-byte accessors present):
  author/committer `name_bytes()`/`email_bytes()`, `when().seconds()` + `offset_minutes()`,
  `message_bytes()` — bytes-first is fully supported; git2's `message()` is the lossy convenience.
- **Tree conversion**: `entry.name_bytes()`, `entry.kind()`, `entry.filemode()` (int).
  A path containing invalid UTF-8 (`w\xc3\xa9ird-\xff\xfe.bin`) **survives write + read-back**.
- **§5.4 decision (evidence-based)**: bytes-first fields — `name: bytes`, `message: bytes`,
  plus lossy `str` conveniences (`name_str`, `message_str`) documented as such.
- BranchName = `RefString`; construct with `RefStr::try_from_str("master")?.to_ref_string()` (no FromStr).

## S4 — Typed write + raw COB inspection ✅ (with one important correction)

- Typed write: `Issues::open(&repo, WriteAs::new(&signer))` + `create(title, desc, [], [], [], &mut NoCache)`
  → issue created, typed read-back works. `cob::cache::NoCache` is the no-op cache for v1.
- Raw layer via upstream traits on `Repository` (no typed materializer needed):
  - `object::Storage::types(&typename)` → **entities** map (PLAN `Raw.entities` ✔)
  - `object::Storage::objects(&typename, &entity)` → **refs** with target change commit (PLAN `Raw.refs` ✔)
  - `change::Storage::load(&oid)` → `Entry`; walk change commits via git parents (PLAN `Raw.history` ✔)
  - Entry fields verified present: id, author (PublicKey), timestamp, parents, related, resource(identity), manifest.
- **Correction to PLAN §5.5**: `Op::<Vec<u8>>::try_from(&entry)` **fails at runtime**
  ("invalid type: map, expected a sequence") — it deserializes each action blob as JSON, and
  actions are JSON objects. The correct raw representation is:
  `entry.contents().iter()` → one raw JSON blob per action (`NonEmpty` by construction).
  i.e. decode per-action `serde_json::from_slice(blob)` for `actions_json()`; keep raw `bytes` otherwise.
- Observed shape: a fresh issue = 2 entries (root `identity` entry + tip entry with `comment` + `edit` actions).
  `RawOp` must expose multiple actions (as planned) and history must expose **all** entries, including the
  identity/root entry, with `identity: Option<Oid>` linking tip entries to it.

## Wheel/packaging probe (`wheel-probe/`) ✅ (one CI follow-up)

- `pyo3 0.23 + abi3-py39 + radicle =0.25.1` compiles and links into a cdylib; `uvx maturin build`
  produces an importable wheel; `parse_repo_id` + error mapping work from Python.
- **Static bundling confirmed**: `git_libgit2_init` and `sqlite3_open` are in-binary symbols
  (`libgit2-sys` from-source, `sqlite` crate bundles C source); `ldd` shows no libgit2/sqlite deps.
- ⚠ Host-built wheel tagged `manylinux_2_34` (host glibc). manylinux_2_28 cross-check **not run
  locally** (docker socket inaccessible). CI recipe: `messense/rust-musl-cross`-style flow via
  `ghcr.io/rust-cross/cargo-zigbuild`, or sibling project's approach: `maturing in
  container + `auditwheel repair`. Track as CI task before 0.1.0.
- SQLite note: radicle 0.25.1 uses the `sqlite` crate (bundled C), not `rusqlite`.

## PyPI ✅

- `radicle-py` — 404 (available) · `radiclepy` — 404 (available). Checked 2026-07-28.

## Plan deltas proposed from evidence

1. §5.4 → bytes-first fields + lossy `*_str` conveniences (decided; evidence S3).
2. §5.5 → raw actions come from `entry.contents()`, **not** `Op<Vec<u8>>::try_from`; document the
   identity/root entry and the `identity: Option[Oid]` link.
3. §8.1 → UTF-8 policy: **resolved** (bytes-first).
4. §8.3 → PyPI names: **resolved** (both available).
5. CI: manylinux rebuild + `auditwheel` check as explicit task before release.
