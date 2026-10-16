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
cargo fmt -p radicle-py -- --check
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
npm run format:check
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

## Architecture migration history

The workspace, test-layout and module-alignment migration tracked in issue
`edcbbd7` is complete. The gates it was validated against, the dependency
audit and the pre-alignment module map are preserved in
[migration gates](archive/MIGRATION-GATES.md); repeat the sdist feasibility
gate there before any future workspace or source-packaging change.
