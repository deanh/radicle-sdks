# Architecture migration gates (historical)

Validation gates, dependency audit and module map for the workspace/test-layout
and module-alignment migration tracked in Radicle issue `edcbbd7`. All three
structural steps are implemented; this file records how they were checked and
is **not** a current development checklist. Moved from `docs/development.md`.

## Gates

Issue `edcbbd7` tracks docs, workspace/test layout, then module alignment.
All three structural steps are implemented here. Shared core/history extraction
remains a separate follow-up. Dependency changes and validation are
recorded in [layout findings](FINDINGS-WORKSPACE-LAYOUT.md).

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
