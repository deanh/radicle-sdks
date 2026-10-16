# Phase 0A — Feasibility Findings

Date: 2026-07-28 (rev 2, post-review) · Target: `radicle = "=0.25.1"` (crates.io, authoritative) · Plan: [`PLAN.md`](./PLAN.md) (canonical)

## S2 — Deterministic temp profile ✅

- `Profile::init(Home::new(path), alias, None, Seed::new([7u8; 32]))` works with **no env vars**
  (no `RAD_HOME`, no `RAD_PASSPHRASE`, no ssh-agent). Unencrypted keystore.
- **Determinism proven**: same seed → same DID across two independent homes
  (`did:key:z6Mkv…5T3Z` both times). Fixtures can hardcode expected identities.
- `profile.signer()` loads with no prompts; `signer.public_key() == profile.id()`.
- Secure seed path for the public `Profile.init()`: `getrandom::fill(&mut [u8; 32])` —
  1000 fills in ~185 µs. Confirms PLAN §5.7 (no `Seed::generate()` upstream; we fill + pass).
- Gotcha: `Seed` is not `Copy`; construct per `init` call.

## S3 — Storage, project init, hand conversions ✅

- **Public** `rad::init(git_repo, name, desc, branch, visibility, signer, storage)` initializes a
  project with zero test features. Returns `(RepoId, Doc, SignedRefs)`.
- `Storage` obtained via `profile.storage`; `storage.repository(rid)` reopens fine.
- `ReadRepository::head()` → `(Qualified, Oid)` = `refs/heads/master → <oid>` — confirms `HeadInfo(name, oid)`.
- **Commit conversion** (via `repo.backend` git2, all raw-byte accessors present):
  author/committer `name_bytes()`/`email_bytes()`, `when().seconds()` + `offset_minutes()`,
  `message_bytes()`. Bytes-first is fully supported.
  ⚠ Precision: git2's `Commit::message()` is **strict** (`Result<&str, Error>`), *not* lossy.
  Any `message_str` convenience is our own `String::from_utf8_lossy(message_bytes())` — a
  binding decision, not a git2 behavior.
- **Tree conversion**: `entry.name_bytes()`, `entry.kind()`, `entry.filemode()` (int).
  A path containing invalid UTF-8 (`w\xc3\xa9ird-\xff\xfe.bin`) **survives write + read-back**.
  ⚠ Scope: the spike byte-sorts entries for display only; **git's canonical tree ordering
  (including directory quirks) was NOT validated** — settle in the Phase 2 schema.
- **§5.4 decision (evidence-based)**: bytes-first fields — `name: bytes`, `message: bytes`,
  plus explicitly-lossy `str` conveniences (`name_str`, `message_str`) documented as such.
- BranchName = `RefString`; construct with `RefStr::try_from_str("master")?.to_ref_string()` (no FromStr).

## S4 — Typed write + raw COB inspection ✅ (rev 2: traversal corrected after review)

- Typed write: `Issues::open(&repo, WriteAs::new(&signer))` + `create(title, desc, [], [], [], &mut NoCache)`
  → issue created, typed read-back works. `cob::cache::NoCache` is the no-op cache for v1.
- Raw layer via upstream traits on `Repository` (no typed materializer needed):
  - `object::Storage::types(&typename)` → **entities** map (PLAN `Raw.entities` ✔)
  - `object::Storage::objects(&typename, &entity)` → **COB refs** with target change commit —
    terminology: these are COB refs, *not* "signed refs"; ref authenticity lives in rad/sigrefs,
    COB refs are not individually signatures (PLAN `Raw.refs` ✔)
  - `change::Storage::load(&oid)` → `Entry` (PLAN `Raw.history` ✔)
  - Entry fields verified present: id, author (PublicKey), timestamp, parents, related,
    resource(identity), manifest.
- **Correction to PLAN §5.5 (rev 1 finding)**: `Op::<Vec<u8>>::try_from(&entry)` **fails at runtime**
  ("invalid type: map, expected a sequence") — it deserializes each action blob as JSON, and
  actions are JSON objects. The raw representation is `entry.contents().iter()` → one raw JSON
  blob per action (`NonEmpty` by construction).
- **Critical traversal rule (rev 2, from review)**: COB change commits have **three categories of
  git parents** — causal history tips, related objects, and the identity/resource commit. Walking
  raw `parent_ids()` conflates them and wrongly pulls the *identity COB's* entry into the issue's
  history (this was an actual bug in spike rev 1). Raw history must traverse **`entry.parents`
  only**; `entry.related` and `entry.resource()` are exposed as links, never walked.
- **Observed shape (corrected)**: a fresh issue is exactly **one operation with two actions**
  (`comment` + `edit`, asserted in the spike: `seen_entries == 1`, `total_actions == 2`). Its
  identity commit belongs to the identity COB, appears only as `identity: Some(<oid>)` on the
  op, and is *not* part of the issue's history.
- `RawOp` therefore exposes multiple actions (as planned) plus `parents` / `related` /
  `identity` / `manifest` as data, with traversal following `parents` only.

## Wheel/packaging probe (`wheel-probe/`) ◐ host-verified, manylinux deferred to CI job

- `pyo3 0.23 + abi3-py39 + radicle =0.25.1` compiles and links into a cdylib; `uvx maturin build`
  produces an importable wheel; `parse_repo_id` + error mapping work from Python.
- **Static bundling confirmed**: `git_libgit2_init` and `sqlite3_open` are in-binary symbols
  (`libgit2-sys` builds from source; `sqlite` crate bundles C source); `ldd` shows no
  libgit2/sqlite dynamic deps. SQLite note: radicle 0.25.1 uses the `sqlite` crate, not `rusqlite`.
- **Gate status (from review)**: only a host-built `manylinux_2_34` wheel was verified locally —
  the manylinux leg of 0A is **partially complete**. Added `wheels.yml` CI job (maturin's default
  glibc manylinux container; no musl tooling) to close the gate on first run. Local docker was
  unusable (socket permission denied), so the cross-glibc check could not run here.

## PyPI ✅

- `radicle-py` — 404 (available) · `radiclepy` — 404 (available). Checked 2026-07-28.

## Plan deltas from evidence

1. §5.4 → bytes-first fields + explicitly-lossy `*_str` conveniences (decided; evidence S3;
   tree ordering explicitly out of scope until Phase 2).
2. §5.5 → raw actions from `entry.contents()`, **not** `Op<Vec<u8>>::try_from`; raw history
   traverses `entry.parents` only (never git parents / related / identity); a fresh issue is
   one op with two actions, identity is a link only.
3. §3 → `Raw.refs` described as COB refs, not signed refs.
4. §8 → UTF-8 policy and PyPI names **resolved**; CI manylinux task now tracked in `wheels.yml`.
