# Radicle SDKs — Python and TypeScript alpha

Typed Python and Node.js/TypeScript bindings for the
[`radicle`](https://crates.io/crates/radicle) crate, pinned to **`=0.25.1`**.
Both SDKs support **local signed project, issue and patch workflows**:
profile/signer setup, project init/fork/checkout, storage and Git inspection,
issue discussions and common writes, and patch revisions/reviews/lifecycle.

| SDK | Package / import | Runtime | Guide |
| --- | --- | --- | --- |
| Python | `radicle-py` / `radiclepy` | CPython ≥ 3.9 (abi3) | [Installation below](#python-installation), [API](docs/API.md) |
| TypeScript | `radicle-sdk` | Node.js ≥ 26.12.0 | [Installation, quick start and contract](node/README.md) |

These are **experimental, source-installed alphas**, not nodes or network
clients. Fetching/announcing projects, connections and daemon control are outside
the SDKs. Git handles commit creation/import and working-copy pushes. Python
additionally exposes local policy management, routing reads and raw COB inspection;
full API parity is not the alpha goal. No shared Python/TypeScript core has been
extracted.

Both package versions are `0.1.0`; the **exact reviewed Git commit**, not the
version, identifies each alpha candidate. APIs may change. Install from Git;
PyPI/npm registry publication is deferred. Installation compiles native code and
requires **Rust ≥ 1.91**, Git and a C toolchain. Source builds can take minutes
on a cold cache.

## Examples

The [project digest](examples/README.md) is a read-only Markdown report of local
patches, latest-revision reviews, merged patches, open issues by label and items
assigned to you. [Python](examples/digest.py) and [TypeScript](examples/digest.mjs)
implement the same program, with tested byte-identical output. They use your
existing profile without loading a signer or making signed writes.

The runnable quick starts create temporary local data: [Python below](#runnable-local-quick-start)
and [TypeScript](node/README.md#runnable-local-quick-start). A sandboxed write-side
review-bot example is planned separately; it is not implemented yet.

## TypeScript installation

Initial native support is **Linux x86_64 (GNU/glibc) only**. Use Node.js ≥ 26.12.0;
there is no upper-version cap. Set `SDK_GIT_URL` to a clone URL supported by your
Git client and replace `FULL_COMMIT_SHA` with a full reviewed commit:

```sh
# npm 12+: explicitly approve the root Git dependency's build scripts.
npm install --allow-git=root "git+${SDK_GIT_URL}#FULL_COMMIT_SHA"
# npm 11: omit the Git approval option.
npm install "git+${SDK_GIT_URL}#FULL_COMMIT_SHA"
```

The npm package is at repository root and builds an optimized native addon during
`prepare`; no prebuilt download or registry publication is required. See the
[TypeScript guide](node/README.md) for pnpm approval, Bun's best-effort limitation,
platform boundaries, executable quick start and async API contracts.

## Python installation

The alpha is distributed from Git, **not PyPI**. Requires CPython ≥ 3.9,
Git, Rust ≥ 1.91 and a C toolchain. libgit2 and SQLite build from bundled
sources. Linux and macOS are the targets; Windows is unsupported/unverified.
Git-URI installation compiles for the host and does not require a published
wheel. Python package version remains `0.1.0` with the Alpha classifier;
the **Git commit**, not the package version, identifies each RC build.

Set `SDK_GIT_URL` to a clone URL of the SDK repository supported by your Git
installation, then install:

```sh
python -m venv .venv
. .venv/bin/activate
SDK_GIT_URL=https://YOUR-HOST/YOUR-REPOSITORY.git
# Replace FULL_COMMIT_SHA with a full reviewed commit SHA.
python -m pip install "git+${SDK_GIT_URL}@FULL_COMMIT_SHA"
```

A local Git checkout also works, after the RC changes have been committed:

```sh
python -m pip install "git+file:///absolute/path/to/checkout@FULL_COMMIT_SHA"
```

Do not enable the `test-helpers` Cargo feature in alpha installations.
For troubleshooting, check `git --version`, `rustc --version` and your C
compiler before installation. No `rad` executable or running daemon is needed
for SDK local workflows. Subsequent external Git operations against the
configured `rad` remote require **`git-remote-rad`** separately.

## Runnable local quick start

This Python example creates a temporary profile and working copy. For the
TypeScript equivalent, see its [runnable quick start](node/README.md#runnable-local-quick-start).

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

    profile = rad.Profile.init(root / "rad-home", "alice")
    storage = profile.storage
    signer = profile.signer()
    rid = rad.init_project(storage, signer, work,
                           name="hello", description="Demo", default_branch="main")
    repo = storage.repository(rid)
    print(rid, repo.commit(repo.head.oid).message_str)

    issues = rad.Issues.open_as(repo, signer)
    issue = issues.create("First issue", "Description", labels=["demo"])
    issue.comment("A reply")
    for comment in issue.reload().comments:
        print(comment.author, comment.body)
    issue.close("solved")

    rad.fork_project(storage, signer, rid)
    rad.checkout_project(storage, rid, signer.node_id, str(root / "checkout"))
```

For an existing home, `Profile.load()` resolves `RAD_HOME` or `~/.radicle`;
`Storage.from_profile(profile).repositories()` discovers stored projects.
Use `RAD_PASSPHRASE` for an encrypted keystore or the upstream SSH-agent path.
Encrypted-key success/wrong/missing-passphrase paths are tested; live
SSH-agent integration is not verified by the alpha suite.

## Python contract and scope limits

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
  cross-thread guarantee. Init/checkout retain the GIL during upstream Git
  subprocess calls. Path inputs accept `str` and `os.PathLike[str]`; byte
  paths are rejected. Path returns remain strings.
- Init refuses an existing `rad` remote without altering its configuration
  or tracking refs. Checkout uses canonical heads; its `remote` parameter
  selects the push namespace, not a peer's branch to check out.
- Patch `latest_revision` identifies the patch-author revision described by
  `base`/`head`/`description`. Use it for review/merge of that revision;
  `version` counts all revision slots (including redactions), not a list index.
- Repository canonical-ref writes do not silently sign afterward;
  `sign_refs(signer)` is explicit. **Typed COB mutations sign upstream**.
- Patch `merge` records an upstream COB merge operation: it does not merge
  Git, move a branch, or clean up refs. The commit must already be in storage.
  Target-branch/quorum evaluation may leave the patch open even if the call
  succeeds; inspect `patch.reload().state`.
- No generic Git write API: callers use Git for commits/imports/branch movement.
- To open a patch from a working copy, use `git push rad HEAD:refs/patches`;
  to update an existing patch, use
  `git push rad HEAD:refs/heads/patches/<id>` (needs `git-remote-rad`).
  `Patches.create`, `draft` and `update` are for commits already in storage
  and need an explicit `base`; see the [Python API](docs/API.md) for how they
  differ from the push path.
- Deferred: node control/network fetch/announce, identity governance/private
  access administration, address-store management, routing writes, inline
  review machinery, reactions, attachments and edit/redaction history APIs.

## Documentation

- [`node/README.md`](./node/README.md): TypeScript installation, runnable example and public contract.
- [`node/CHANGELOG.md`](./node/CHANGELOG.md): TypeScript alpha changes.
- [`examples/README.md`](./examples/README.md): cross-SDK programs and usage.
- [`docs/API.md`](./docs/API.md): Python public API contract.
- [`CHANGELOG.md`](./CHANGELOG.md): Python release changes.
- [`docs/README.md`](./docs/README.md): documentation index, including archived
  plans, reviews and implementation findings in `docs/archive/`.
- `spike/` and `wheel-probe/`: historical feasibility probes, not the SDK.

## Development and validation

Python (from the repository root):

```sh
uv sync --dev --locked
uv run maturin develop --uv --features test-helpers
uv run pytest -q
uv run python -m mypy.stubtest radiclepy --allowlist tests/stubtest-allowlist.txt
uv run mypy --strict tests/typing/usage.py
uv run pyright tests/typing/usage.py
uv run pyright --verifytypes radiclepy --ignoreexternal
cargo clippy --all-targets --locked --features test-helpers -- -D warnings
cargo clippy --all-targets --locked -- -D warnings

# Public-only build: helper-free workflows still run; legacy fixtures skip.
uv run maturin develop --uv
uv run pytest -q
uv run python tests/workflow_smoke.py
```

TypeScript and cross-SDK examples (after the Python setup above):

```sh
nvm use
npm ci --ignore-scripts
npm run build
npm run check
npm test
RADICLE_REQUIRE_CROSS_SDK=1 uv run pytest -q tests/test_examples.py
```

The final command requires both SDK builds: missing TypeScript output fails
rather than skipping conformance checks. See the [TypeScript guide](node/README.md#repository-development-and-validation)
for native Rust checks and exact-commit npm/pnpm installation gates.

The Python README quick start is executed directly by pytest; the TypeScript
suite and Git-install gates execute its actual README quick-start fence.
Python stubtest checks the installed exports/signatures, with two documented
extension-introspection
exceptions; behavioral tests check Literal domains and snapshot semantics.
Mypy/pyright check consumer examples; `--verifytypes` checks annotation
completeness, not semantic correctness. Stubs mark non-subclassable extension
classes as final and keep identifier aliases compatible with Python 3.9.

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
