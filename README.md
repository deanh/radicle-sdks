# radicle-py — alpha release candidate

Typed Python bindings for the [`radicle`](https://crates.io/crates/radicle)
crate, pinned to **`=0.25.1`**. The alpha covers **local project workflows**:
profiles/signers, project init/fork/checkout, Git/storage inspection, local
policy/routing, issue discussions and common writes, patch
revision/review/lifecycle operations, and raw COB inspection.

This is **not a node or a network client**. Fetching/announcing projects,
connections and daemon control are outside this alpha. Policy writes change
persisted local policy; they do not perform networking. APIs may change during
alpha; pin installations to a reviewed Git commit.

## Install from Git

The alpha is distributed from Git, **not PyPI**. Requires CPython ≥ 3.9,
Git, Rust ≥ 1.88 and a C toolchain. libgit2 and SQLite build from bundled
sources. Linux and macOS are the targets; Windows is unsupported/unverified.
Git-URI installation compiles for the host and does not require a published
wheel. Python package version remains `0.1.0` with the Alpha classifier;
the **Git commit**, not the package version, identifies each RC build.

Set `SDK_GIT_URL` to the actual clone URL of the SDK repository (this checkout
has no configured remote), then install:

```sh
python -m venv .venv
. .venv/bin/activate
SDK_GIT_URL=https://YOUR-HOST/YOUR-REPOSITORY.git
python -m pip install "git+${SDK_GIT_URL}@alpha-rc"
# Reproducible install: replace the branch with a full reviewed commit SHA.
python -m pip install "git+${SDK_GIT_URL}@FULL_COMMIT_SHA"
```

A local Git checkout also works, after the RC changes have been committed:

```sh
python -m pip install "git+file:///absolute/path/to/radicle-py@alpha-rc"
```

Do not enable the `test-helpers` Cargo feature in alpha installations.
For troubleshooting, check `git --version`, `rustc --version` and your C
compiler before installation. No `rad` executable or running daemon is needed
for SDK local workflows. Subsequent external Git operations against the
configured `rad` remote require **`git-remote-rad`** separately.

## Runnable local quick start

Unlike the interactive **`rad init` CLI**, `radiclepy.init_project` calls the
crate directly with explicit metadata. Git subprocess calls below only create
the input Git repository; they use argument arrays, never a shell.

```python
from pathlib import Path
import subprocess
import tempfile
import radiclepy as rad

with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    work = root / "work"
    work.mkdir()

    def git(*args):
        subprocess.run(
            ["git", "-C", str(work), "-c", "user.name=Alice",
             "-c", "user.email=alice@example.invalid", *args],
            check=True, capture_output=True,
        )

    git("init", "--initial-branch=main")
    (work / "README.md").write_text("# Hello Radicle\n")
    git("add", "README.md")
    git("commit", "-m", "initial commit")

    profile = rad.Profile.init(str(root / "rad-home"), "alice")
    storage = rad.Storage.from_profile(profile)
    signer = profile.signer()
    rid = rad.init_project(storage, signer, str(work), "hello", "Demo", "main")
    repo = storage.repository(rid)
    print(rid, repo.commit(repo.head.oid).message_str)

    issues = rad.Issues.open_as(repo, signer)
    issue = issues.create("First issue", "Description", labels=["demo"])
    issue.comment("A reply")
    for comment in issue.reload().comments:
        print(comment.author, comment.body)
    issue.lifecycle("closed", "solved")

    rad.fork_project(storage, signer, rid)
    rad.checkout_project(storage, rid, signer.node_id, str(root / "checkout"))
```

For an existing home, `Profile.load()` resolves `RAD_HOME` or `~/.radicle`;
`Storage.from_profile(profile).repositories()` discovers stored projects.
Use `RAD_PASSPHRASE` for an encrypted keystore or the upstream SSH-agent path.
Encrypted-key success/wrong/missing-passphrase paths are tested; live
SSH-agent integration is not verified by the alpha suite.

## Contract and scope limits

- IDs are strings; `parse_*` functions validate/canonicalize them.
- Binding input validation raises `ValueError`; operational failures raise
  the `radiclepy.Error` hierarchy. Project multi-step failures can leave
  partial filesystem/configuration changes; no rollback guarantee is made.
- Git byte-typed fields preserve `bytes`, with lossy `*_str` conveniences.
- Typed comments/revisions/reviews use epoch **milliseconds**; raw COB
  operations use epoch **seconds**.
- Values are snapshots, collections are eager copies, and handles reopen
  on use. `Issues.open`/`Patches.open` are lazy; failures occur on operations.
- Signers and write-store/mutable handles are thread-affine; no async API or
  cross-thread guarantee. Paths currently accept strings, not `PathLike`.
- Repository canonical-ref writes do not silently sign afterward;
  `sign_refs(signer)` is explicit. **Typed COB mutations sign upstream**.
- Patch `merge` records an upstream COB merge operation: it does not merge
  Git, move a branch, or clean up refs. The commit must already be in storage.
  Target-branch/quorum evaluation may leave the patch open even if the call
  succeeds; inspect `patch.reload().state`.
- No generic Git write API: callers use Git for commits/imports/branch movement.
- Deferred: node control/network fetch/announce, identity governance/private
  access administration, address-store management, routing writes, inline
  review machinery, reactions, attachments and edit/redaction history APIs.

## Documentation

- [`API.md`](./API.md): current public API contract.
- [`ALPHA-PLAN.md`](./ALPHA-PLAN.md): bounded alpha/MVP scope and completion record.
- [`CHANGELOG.md`](./CHANGELOG.md): release changes.
- [`PLAN.md`](./PLAN.md): historical phased implementation design; not the
  current release checklist. `FINDINGS-*.md` retain phase evidence.
- `spike/` and `wheel-probe/`: historical feasibility probes, not the SDK.

## Development and validation

```sh
uv sync --dev --locked
uv run maturin develop --uv --features test-helpers
uv run pytest -q
cargo clippy --all-targets --locked --features test-helpers -- -D warnings
cargo clippy --all-targets --locked -- -D warnings

# Public-only build: helper-free workflows still run; legacy fixtures skip.
uv run maturin develop --uv
uv run pytest -q
uv run python tests/workflow_smoke.py
```

CI builds Linux manylinux_2_28 x86_64/aarch64 and macOS universal2 artifacts,
checks the source distribution, and executes installed-artifact workflow
smokes. These are validation artifacts, not PyPI publication. Host-built
Linux wheels may have a newer glibc floor and must not be substituted for
CI's repaired wheels. Platform verification is established by successful
native CI jobs, not the abi3 tag alone.

For an RC, commit the changes, run CI, then install the reviewed commit via
Git URI and run `tests/wheel_smoke.py` and `tests/workflow_smoke.py` using the
clean environment's interpreter. Do not install an uncommitted worktree via
Git URI expecting those changes to be included. No automated publisher or
PyPI upload is part of this alpha flow.

## License

Dual **MIT OR Apache-2.0**, matching upstream. See `LICENSE-MIT` and
`LICENSE-APACHE`.
