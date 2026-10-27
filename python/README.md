# radicle-py 0.1.0: Python alpha

Typed CPython bindings for local signed Radicle project, issue and patch
workflows, plus storage/repository inspection, local policy management, routing
reads and raw COB inspection. Import the package as `radiclepy`.

See the [API reference](API.md) for signatures and conversion rules, the
[changelog](CHANGELOG.md) for Python changes, and the
[project overview](../README.md) for both SDKs, including the shared
[scope and non-goals](../README.md#scope-and-non-goals). This guide is also
the Python package's long description; it is not a registry-publication
announcement.

## Python installation

From 0.1.1, releases are published to PyPI as `radicle-py`: abi3 wheels for
CPython ≥ 3.9 on Linux x86_64 (`manylinux_2_28`) and macOS arm64 (11.0+),
plus an sdist. Each release is also recorded as a signed rad-artifact release
object on the Radicle repository, whose manifest lists the digest of every
published file; see [Releasing](../docs/release.md).

```sh
python -m pip install "radicle-py==<version>"
```

Release 0.1.0 (Linux x86_64 only) predates the registry step and is available
only through rad-artifact:

```sh
rad-artifact download <tag-object-id> --cid <cid> --repo rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg
rad-artifact verify radicle_py-0.1.0-cp39-abi3-manylinux_2_28_x86_64.whl --repo rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg
python -m pip install ./radicle_py-0.1.0-cp39-abi3-manylinux_2_28_x86_64.whl
```

Installing from Git remains the development path and the only path on other
platforms. It requires Git, Rust ≥ 1.91 and a C toolchain; libgit2 and SQLite
build from bundled sources. Linux and macOS are the targets; Windows is
unsupported/unverified. The **Git commit** identifies a development build.

Set `SDK_GIT_URL` to a clone URL of the SDK repository supported by your Git
installation, then install:

```sh
python -m venv .venv
. .venv/bin/activate
SDK_GIT_URL=https://YOUR-HOST/YOUR-REPOSITORY.git
# Replace FULL_COMMIT_SHA with a full reviewed commit SHA.
python -m pip install "git+${SDK_GIT_URL}@FULL_COMMIT_SHA"
```

A local Git checkout also works, at any committed revision:

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
TypeScript equivalent, see its [runnable quick start](../node/README.md#runnable-local-quick-start).

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
  and need an explicit `base`; see the [Python API](API.md) for how they
  differ from the push path.
- Deferred: node control/network fetch/announce, identity governance/private
  access administration, address-store management, routing writes, inline
  review machinery, reactions, attachments and edit/redaction history APIs.

## Shared workflow documentation

- [Workflow semantics](../docs/semantics.md): signing, Git import, revisions,
  merge records and partial failures.
- [SDK differences](../docs/sdk-differences.md): units, snapshots and concurrency.
- [Development and validation](../docs/development.md): repository build commands,
  typing checks and clean installed-artifact tests.
