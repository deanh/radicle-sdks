# radicle-py

Python bindings for the [`radicle`](https://crates.io/crates/radicle) crate
(exact pin **`=0.25.1`**, crates.io source
authoritative). abi3 wheels for **CPython ≥ 3.9** on Linux and macOS; fully
typed (PEP 561). No async runtime, no p2p networking: this is the radicle
node library — identities, storage, policy/routing stores, COB reads and a
narrow, upstream-shaped write surface — not a node.

**0.1.0** — first release. See [`CHANGELOG.md`](./CHANGELOG.md).

## Install

| Platform | Wheel |
|---|---|
| Linux x86_64 | `radicle_py-0.1.0-cp39-abi3-manylinux_2_28_x86_64.whl` (glibc ≥ 2.28; auditwheel-repaired) |
| Linux aarch64 | `radicle_py-0.1.0-cp39-abi3-manylinux_2_28_aarch64.whl` |
| macOS | universal2 wheel (arm64 + x86_64) |
| Windows | **unsupported, unverified** (v1 scope; revisit post-v1) |

```sh
pip install radicle-py        # once published to PyPI (see RELEASING below)
```

Wheels bundle libgit2 and SQLite statically — no system packages needed at
runtime (or to build from sdist, beyond a C toolchain and Rust ≥ 1.88).
Python 3.9+ on all platforms (single abi3 wheel per platform).

## Quick start

```python
import radiclepy as rad

profile = rad.Profile.init("/tmp/rad-demo", "alice")   # secure random key
storage = rad.Storage.from_profile(profile)
repo = storage.repository("rad:z…")                    # read-only handle
print(repo.head.name, repo.commit(repo.head.oid).message_str)

policy = profile.policy_store()                        # follow/seed policies
policy.follow(profile.node_id, alias="self")

for issue in rad.Issues.open(repo).all():              # typed COB readers
    print(issue.title, issue.state, issue.labels)

# Writes are narrow and upstream-shaped: the three canonical-ref writes take
# no signer; sign_refs is the only signer-taking write; COB write stores fix
# the signer at open (`open_as`) — mutators take none.
signed = repo.sign_refs(profile.signer())
issues = rad.Issues.open_as(repo, profile.signer())
m = issues.create("hello", "first issue", labels=["demo"])
m.comment("…", reply_to=m.issue.comment_ids[0])
```

## Conventions (contract)

- Identifiers (`RepoId`, `Did`, `NodeId`, `Oid`, `ObjectId`, `Alias`) are
  `str` at the boundary; `parse_*` functions validate.
- `ValueError` comes only from binding validators; operational failures
  raise the `radiclepy.Error` hierarchy (`StorageError`, `RepositoryError`,
  `GitError`, `DatabaseError`, `IdentityError`, `SignerError`,
  `PolicyError`, `CobError`).
- Byte-typed sources (git names/messages) are `bytes`; explicit lossy
  `*_str` accessors are provided. COB `String` fields are plain `str`.
- Test-only helpers (`Profile._from_seed`, `radiclepy._test`) are
  compile-time gated and never ship in release wheels (CI wheel smoke
  asserts their absence).

## Documentation

- [`API.md`](./API.md) — the public API contract (the binding's spec).
- [`CHANGELOG.md`](./CHANGELOG.md) — release notes.
- [`PLAN.md`](./PLAN.md) — the design (v3.2), reviewed three times.
  Canonical; `../radicle-py-plan.md` is a symlink here.
- Feasibility & phase evidence:
  [`FINDINGS-0A.md`](./FINDINGS-0A.md),
  [`FINDINGS-02.md`](./FINDINGS-02.md) (tree order, tag peel),
  [`FINDINGS-03.md`](./FINDINGS-03.md) (stores, sqlite semantics),
  [`FINDINGS-04.md`](./FINDINGS-04.md) (COB walk contract),
  [`FINDINGS-05.md`](./FINDINGS-05.md) (writes).
- `spike/`, `wheel-probe/` — throwaway Phase 0A probes. NOT the binding.

## Development

Requires Rust ≥ 1.88 (MSRV, from `radicle 0.25.1`), [uv](https://docs.astral.sh/uv/),
and a C toolchain (libgit2/SQLite build from source).

```sh
uv sync --dev
uv run maturin develop --uv --features test-helpers   # test build (deterministic fixtures)
uv run pytest -q                                      # 168 tests

# release surface (what ships): no test-helpers — helpers are skipped/absent
uv run maturin develop --uv
uv run pytest -q

cargo clippy --all-targets --locked --features test-helpers -- -D warnings
cargo clippy --all-targets --locked -- -D warnings
```

Release wheels build **without** `test-helpers`; the deterministic seed
helper is absent there by construction — CI's wheel smoke step asserts it.

## Releasing

1. Bump `version` in `Cargo.toml` + `pyproject.toml` (keep `Cargo.lock` /
   `uv.lock` in sync); update `CHANGELOG.md`.
2. Tag `vX.Y.Z` — `wheels.yml` builds the manylinux ×2 + macOS universal2
   wheels and the sdist, audits/repairs Linux wheels, and smoke-imports
   every artifact.
3. Publishing to PyPI is manual for now: download the tag's artifacts and
   `twine upload` them.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
- MIT license ([LICENSE-MIT](./LICENSE-MIT))

at your option. Same terms as the upstream `radicle` crate.
