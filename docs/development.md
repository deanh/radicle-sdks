# Development and validation

Run commands from repository root. Both bindings are members of the virtual
Cargo workspace: `python/native/` and `node/native/`. Python-only tests live in
`python/tests/`; root `tests/` owns cross-SDK tests and shared fixture support.
See [architecture](architecture.md) for ownership and remaining follow-ups.

Both packages are `0.1.0`. Pin the full reviewed Git SHA for candidate installs;
Git installs do not include uncommitted worktree changes. No automated publisher,
PyPI upload or npm publication is part of this alpha flow.

## Python development

Requires CPython >=3.9, Rust >=1.91, Git and a C toolchain. Install development
tools through the committed uv lock:

```sh
uv sync --dev --locked
uv run maturin develop --uv --features test-helpers
RADICLE_REQUIRE_TEST_HELPERS=1 uv run --no-sync pytest -q -rs
uv run --no-sync python -m mypy.stubtest radiclepy --allowlist python/tests/stubtest-allowlist.txt
uv run --no-sync mypy --strict python/tests/typing/usage.py
uv run --no-sync pyright python/tests/typing/usage.py
uv run --no-sync pyright --verifytypes radiclepy --ignoreexternal
cargo clippy -p radicle-py --all-targets --locked --features test-helpers -- -D warnings
cargo clippy -p radicle-py --all-targets --locked -- -D warnings
```

`test-helpers` enables deterministic legacy fixtures. Never enable it in release
artifacts. Pass `--no-sync` to every `uv run` after the helper build: a plain
`uv run` syncs the project first and, whenever uv considers the installed package
stale, rebuilds the extension *without* the feature. The deterministic fixtures
would then skip and the run would still pass. `RADICLE_REQUIRE_TEST_HELPERS=1`
turns that skip into a failure; CI sets it. Helper-free builds still execute
public workflow tests; fixtures that require deterministic seeding intentionally
skip:

```sh
uv run maturin develop --uv
uv run pytest -q
uv run python python/tests/workflow_smoke.py
```

The Python guide's actual fenced quick start is extracted by
`python/tests/test_readme.py`. Use `uv run pytest -q python/tests` for the
Python-only suite; plain pytest collects both configured test directories.
API tests check exported names/kinds, not all prose.
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
bash node/scripts/tarball-install-smoke.sh
cargo +1.91.0 test --release -p radicle-sdk-native --locked
cargo +1.91.0 clippy -p radicle-sdk-native --all-targets --locked -- -D warnings
cargo +stable fmt -p radicle-sdk-native -- --check
```

Build clears `node/dist/` before producing optimized native output, generated
native declarations and the wrapper. Do not hand-edit generated files. Keep the
explicit inherited `indexmap/std` in both adapters; a combined build must not
hide missing features in either package. Do not refresh the root lock casually.

A negative build gate must reject abort mode with
`radicle-sdk requires panic=unwind`:

```sh
cargo +1.91.0 rustc --release -p radicle-sdk-native --locked --lib -- -C panic=abort
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
/tmp/radicle-python-consumer/bin/python python/tests/wheel_smoke.py
/tmp/radicle-python-consumer/bin/python python/tests/workflow_smoke.py
```

Also extract and execute the Python guide's actual fence with that interpreter;
check installed metadata contains the Python guide, not the two-SDK overview.
Test a clean `maturin sdist` by unpacking/installing it outside the checkout. It
must contain the manifests, sources, path dependencies and Python metadata inputs
needed to build, including its guide. Registry dependencies may be downloaded or
pre-cached; self-contained source does not mean all dependencies are vendored.

Git preparation builds Node in a full checkout before packing. Tarball consumers
load `node/dist`; no `install`/`postinstall` lifecycle builds Rust there. Native
sources/manifests are not in the npm payload: it ships built output, language
guides/API/changelog and licenses, not a workspace consumer rebuild contract.
The tarball smoke script checks contents and installed runtime/types/actual
README with invalid Rust tool paths during installation, independently of Git
installs. Host-built tarballs are validation artifacts, not portable prebuilt
platform packages or authorization to publish.

## CI evidence and platforms

Current workflows are `.github/workflows/python.yml` and
`.github/workflows/node.yml`. The Node path filters include both binding
sources and shared workspace/fixture inputs; package selection remains explicit.

Python workflows define minimum/stable Rust checks, helper/release feature
checks, typing, source distribution installation, and manylinux_2_28
x86_64/aarch64 and macOS universal2 artifact jobs. Installed workflow/purity
smokes run against the resulting native artifacts. An abi3 tag alone does not
establish a platform claim. A locally built wheel can require a newer glibc than
CI's repaired wheel; do not substitute it as manylinux_2_28 evidence.

Node workflow gates cover release output, native tests/lints, abort rejection,
consumer types, packed runtime/docs/types, required digest parity and exact-commit
npm/pnpm installation.
Live agent variants, other platforms and transports require their own evidence.
Report local checks separately from successful remote/native CI jobs.

## Architecture migration gates

Issue `edcbbd7` tracks docs, workspace/test layout, then module alignment.
All three structural steps are implemented here. Shared core/history extraction
remains a separate follow-up. Dependency changes and validation are
recorded in [layout findings](archive/FINDINGS-WORKSPACE-LAYOUT.md).

### Workspace feasibility first

The initial isolated throwaway experiment used the virtual workspace and
Python-crate move only, before changing tests/workflows. Repeat this gate for
future workspace/source-packaging changes. Build an sdist, unpack it outside
the checkout and build using pre-cached dependencies with Cargo offline. Prevent
access to original checkout sources. This establishes workspace-member/target
closure, not vendoring every registry crate. Inspect how maturin includes or
rewrites workspace manifests; do not assume it trims unused Node members.
The validated maturin 1.15.0 path trims Node membership and Cargo prunes unused
Node lock records without adding selections. Source builds may perform that
pruning offline; Git/workspace build gates continue enforcing `--locked`.

If the sdist cannot build independently, decide a fallback before expanding the
patch. Excluding Node requires explicit workspace exclusion and retaining its
own lock and dependency constraints; it is not equivalent to the single-lock
end state.

### Dependency and fixture audit

Use resolver 3 and explicit package selection. Seed the merged lock from existing
selections and record per-package `cargo tree` versions/features before and after.
MSRV-aware resolution can choose different versions; unavoidable changes must
be reviewed, not hidden in lock regeneration. The alignment commit adopts the
existing newer Node selections for 31 shared package names, retaining Python-only
selections. Verify each adapter's features independently, including explicit
`indexmap/std`, and keep exact Radicle 0.25.1/Rust 1.91.

Shared public-API fixture construction lives in `tests/sdk_fixture.py`; root
cross-SDK tests import it directly, without Python-private workflow tests or
conftest fixtures. Python's workflow smoke imports it through an explicit
repository-relative support path for direct script execution. Pytest paths are
configured in `pyproject.toml`; verify each suite independently as well as together.

For the layout patch run all language tests/typing, required digest parity,
actual guide examples, Rust minimum/stable checks, fresh optimized panic gates,
exact-commit Python/npm 11/npm 12/pnpm installs, tarball consumer tests, clean
sdist installation and wheel purity/typing smokes. Update CI paths/caches and
manifest/version tests. No dependency upgrade or behavior normalization belongs
in the mechanical migration.

### Module alignment map

The module-alignment patch implements the following ownership under
`python/native/src/` (the first column records pre-alignment paths):

| Previous paths | Current module ownership |
| --- | --- |
| `storage.rs`, `write.rs` | `repository/mod.rs`, `write.rs`, `values.rs`: storage and repository reads/writes/snapshots |
| `cob_typed.rs` | `issues/` and `patches/`: operations in `mod.rs`, snapshots/materialization in domain `values.rs` |
| `values.rs`, `cob_values.rs` | domain `values.rs` under repository/project/issues/patches/policy/routing; common `comment.rs`; `hashing.rs` holds only snapshot hashing |
| `cob_raw.rs` | `raw.rs` (Python-only) |
| `identity.rs` validators | `ids.rs`, separate from error presentation |
| `profile.rs`, `project.rs`, `policy.rs`, `routing.rs`, `error.rs`, `testhelpers.rs` | unchanged responsibilities; project/policy/routing have domain subdirectories for snapshots |
| `lib.rs` | registration/module declarations only |

Node's former mixed `lib.rs` now separates profile, project, repository, issues,
errors and argument parsers; patch operations live in `patches/mod.rs`.
Repository, issue and patch snapshots live in their domain `values.rs`, and
`comment.rs` holds the small common DTO.
`tasks.rs`, `locks.rs`, `timestamps.rs` and `probes.rs` remain binding-specific.
Native free functions are re-exported at root; class registration stays with napi
macros. Match operation ownership, not an artificial identical type hierarchy.
Public declarations, error codes, validation order, lock order and snapshot timing
are unchanged. No shared core, scheduler, DTO hierarchy or dependency change is
part of module alignment.

Remove obsolete probes only after checking references and recording exact
last-containing commits, in a separate cleanup commit. Do not silently modernize
historical findings into claims about the current SDK.
