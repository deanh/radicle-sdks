# Development and validation

Commands below describe the **current checkout**, before the Rust/test-layout
migration. Run them from repository root. Python owns its guide/reference under
`python/`, but its Rust crate and tests still live at root. Node remains a
standalone native crate. See [architecture](architecture.md) for planned paths.

Both packages are `0.1.0`. Pin the full reviewed Git SHA for candidate installs;
Git installs do not include uncommitted worktree changes. No automated publisher,
PyPI upload or npm publication is part of this alpha flow.

## Python development

Requires CPython >=3.9, Rust >=1.91, Git and a C toolchain. Install development
tools through the committed uv lock:

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
```

`test-helpers` enables deterministic legacy fixtures. Never enable it in release
artifacts. Helper-free builds still execute public workflow tests; fixtures that
require deterministic seeding intentionally skip:

```sh
uv run maturin develop --uv
uv run pytest -q
uv run python tests/workflow_smoke.py
```

The Python guide's actual fenced quick start is extracted by
`tests/test_readme.py`. API tests check exported names/kinds, not all prose.
Stubtest checks installed exports/signatures, with documented introspection
exceptions; mypy/pyright check consumer examples. `--verifytypes` establishes
annotation completeness, not semantic correctness. Behavioral tests establish
finite domains, snapshot equality/hash, PathLike support and workflow semantics.

## TypeScript development

Use Node >=26.12.0; the validated native platform is Linux x86_64 GNU/glibc.

```sh
nvm use
npm ci --ignore-scripts
RUSTUP_TOOLCHAIN=1.91.0 npm run build
npm run check
npm test
cargo +1.91.0 test --release --manifest-path node/native/Cargo.toml --locked
cargo +1.91.0 clippy --manifest-path node/native/Cargo.toml --all-targets --locked -- -D warnings
cargo +stable fmt --manifest-path node/native/Cargo.toml -- --check
```

Build clears `node/dist/` before producing optimized native output, generated
native declarations and the wrapper. Do not hand-edit generated files. Keep the
standalone graph's explicit `indexmap/std`; Python's features must not accidentally
make a Node-only build pass. Do not refresh Cargo locks casually.

A negative build gate must reject abort mode with
`radicle-sdk requires panic=unwind`:

```sh
cargo +1.91.0 rustc --release --manifest-path node/native/Cargo.toml --locked --lib -- -C panic=abort
```

That command is expected to fail. Runtime tests exercise worker and synchronous
panic containment, poison behavior and persisted side effects. Neither a panic
error nor an optimized build provides rollback, process isolation or a latency
promise. Fresh optimized artifacts—not recycled debug evidence—are required for
Rust/build changes.

## Cross-SDK tests and documentation

After building both SDKs:

```sh
RADICLE_REQUIRE_CROSS_SDK=1 uv run pytest -q
```

Required mode fails if the TypeScript build is absent. Optional Python-only runs
may skip parity checks. Digest tests create one multi-actor fixture, execute the
actual programs, check content and compare byte-identical output, including
Unicode ordering and latest-revision review coverage. These are not whole-API
parity tests.

Docs tests validate repository-local links and heading anchors in both language
guides/references/changelogs and shared/archive Markdown. Historical references
to sibling developer checkouts are not repository dependencies. Runnable examples
must execute through package exports, not private native imports or copied tests.

## Installed artifacts and exact commits

After committing a candidate, test pinned Node package managers:

```sh
bash node/scripts/git-install-smoke.sh npm11 FULL_COMMIT_SHA
bash node/scripts/git-install-smoke.sh npm12 FULL_COMMIT_SHA
bash node/scripts/git-install-smoke.sh pnpm FULL_COMMIT_SHA
# Bun Git prepare remains best effort, not a release gate:
bash node/scripts/git-install-smoke.sh bun FULL_COMMIT_SHA
```

The script uses fresh package-manager caches/stores and a full-SHA root Git
source. It defaults to `git+file://...`; `SDK_GIT_URL` selects another transport.
Cargo dependencies may be warm, so elapsed times are not cold-install estimates.
Each gate runs installed workflows, extracts and runs the actual installed guide,
and checks positive/negative constructor typing with the installed declarations.
`npm install /path/to/checkout` is not a substitute.

For a Python candidate:

```sh
python -m venv /tmp/radicle-python-consumer
/tmp/radicle-python-consumer/bin/python -m pip install 'git+file:///ABSOLUTE/CHECKOUT@FULL_COMMIT_SHA'
/tmp/radicle-python-consumer/bin/python tests/wheel_smoke.py
/tmp/radicle-python-consumer/bin/python tests/workflow_smoke.py
```

Also extract and execute the Python guide's actual fence with that interpreter;
check installed metadata contains the Python guide, not the two-SDK overview.
Test a clean `maturin sdist` by unpacking/installing it outside the checkout. It
must contain the manifests, sources, path dependencies and Python metadata inputs
needed to build, including its guide. Registry dependencies may be downloaded or
pre-cached; self-contained source does not mean all dependencies are vendored.

Git preparation builds Node in a full checkout before packing. Tarball consumers
load `node/dist`; no `install`/`postinstall` lifecycle builds Rust there. Native
sources are currently included in the npm file list but are not a supported
consumer rebuild path. The layout patch will remove them rather than export a
workspace source-build contract. Validate the unpacked/installed tarball's runtime,
consumer types, actual README and API/changelog presence independently of Git
installs. Host-built tarballs are validation artifacts, not portable prebuilt
platform packages or authorization to publish.

## CI evidence and platforms

Current workflows are `.github/workflows/wheels.yml` and
`.github/workflows/ts-sdk.yml`; the layout patch will rename them `python.yml`
and `node.yml` without dropping gates.

Python workflows define minimum/stable Rust checks, helper/release feature
checks, typing, source distribution installation, and manylinux_2_28
x86_64/aarch64 and macOS universal2 artifact jobs. Installed workflow/purity
smokes run against the resulting native artifacts. An abi3 tag alone does not
establish a platform claim. A locally built wheel can require a newer glibc than
CI's repaired wheel; do not substitute it as manylinux_2_28 evidence.

Node workflow gates cover release output, native tests/lints, abort rejection,
consumer types, required digest parity and exact-commit npm/pnpm installation.
Live agent variants, other platforms and transports require their own evidence.
Report local checks separately from successful remote/native CI jobs.

## Architecture migration gates

Issue `edcbbd7` tracks three structural patches: docs, workspace/test layout,
then module alignment. Core/history sharing remains a separate follow-up.

### Workspace feasibility first

Before moving tests or workflows, use an isolated throwaway worktree for the
virtual workspace and Python-crate move only. Build an sdist, unpack it outside
the checkout and build using pre-cached dependencies with Cargo offline. Prevent
access to original checkout sources. This establishes workspace-member/target
closure, not vendoring every registry crate. Inspect how maturin includes or
rewrites workspace manifests; do not assume it trims unused Node members.

If the sdist cannot build independently, decide a fallback before expanding the
patch. Excluding Node requires explicit workspace exclusion and retaining its
own lock and dependency constraints; it is not equivalent to the single-lock
end state.

### Dependency and fixture audit

Use resolver 3 and explicit package selection. Seed the merged lock from existing
selections and record per-package `cargo tree` versions/features before and after.
MSRV-aware resolution can choose different versions; any unavoidable changes
must be reviewed, not hidden in a lock regeneration. Verify Node-only features
independently, including `indexmap/std`, and keep exact Radicle 0.25.1/Rust 1.91.

Move Python-only fixtures, typing and smokes with their tests. Root cross-SDK
tests currently import `workflow_smoke` helpers: explicitly place shared fixture
support where both suites can discover it, rather than relying on an incidental
`conftest.py` ancestor or pytest import path. Verify both suites are collected.

For the layout patch run all language tests/typing, required digest parity,
actual guide examples, Rust minimum/stable checks, fresh optimized panic gates,
exact-commit Python/npm 11/npm 12/pnpm installs, tarball consumer tests, clean
sdist installation and wheel purity/typing smokes. Update CI paths/caches and
manifest/version tests. No dependency upgrade or behavior normalization belongs
in the mechanical migration.

### Module alignment map

The following mapping is for the later mechanical module patch, not current paths:

| Current Python `src/` | Target `python/native/src/` |
| --- | --- |
| `storage.rs`, `write.rs` | `repository.rs` or `repository/`: storage plus repository reads/writes |
| `cob_typed.rs` | `issues.rs`, `patches.rs`, with materialization near the corresponding domain |
| `values.rs`, `cob_values.rs` | snapshot `values` modules, divided by domain if needed rather than one oversized file |
| `cob_raw.rs` | `raw.rs` (Python-only) |
| `identity.rs` validators | `ids.rs`, separate from error presentation |
| `profile.rs`, `project.rs`, `policy.rs`, `routing.rs`, `error.rs`, `testhelpers.rs` | unchanged domain names |
| `lib.rs` | registration/module declarations only |

Node's mixed `lib.rs` similarly separates profile, project, repository, issues
and errors; existing patch operations stay in `patches`. Tasks, locks and Date
conversion remain binding-specific modules. Match operation ownership, not an
artificial identical type hierarchy. Preserve public declarations, error codes,
validation order, lock order and snapshot timing.

Remove obsolete probes only after checking references and recording exact
last-containing commits, in a separate cleanup commit. Do not silently modernize
historical findings into claims about the current SDK.
