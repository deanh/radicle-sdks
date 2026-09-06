# radicle-py

Python bindings for the [`radicle`](https://crates.io/crates/radicle) crate (`=0.25.1`, crates.io authoritative).

**Status: Phase 1 complete (33 tests) → Phase 2 (storage & read-only repository) next.** The `wheels.yml` manylinux gate closes on first CI run.

- [`PLAN.md`](./PLAN.md) — the design (v3.2), reviewed three times. Canonical; `../radicle-py-plan.md` is a symlink here.
- [`API.md`](./API.md) — the public API contract (Phase 1 surface).
- [`FINDINGS-0A.md`](./FINDINGS-0A.md) — feasibility evidence from Phase 0A spikes.
- `spike/`, `wheel-probe/` — throwaway Phase 0A probes. NOT the binding.

## Try it

```sh
uv sync --dev
uv run maturin develop --uv --features test-helpers   # test build (deterministic fixtures)
uv run pytest -q
```

Release wheels build **without** `test-helpers`; the deterministic seed
helper (`Profile._from_seed`) is absent there by construction — CI's wheel
smoke step asserts this.

```python
import radiclepy as rad

profile = rad.Profile.init("/tmp/rad-demo", "alice")  # secure random key
print(profile.did, profile.node_id)
profile.signer().node_id  # matches profile.node_id
```

Phase 0A verdict: green, with the manylinux leg gated on the first CI run — deterministic
profiles, public `rad::init`, hand conversions with bytes-first UTF-8 policy, raw COB history
traversing `entry.parents` only (identity/related are links, never walked), and an importable
abi3 wheel with libgit2+SQLite statically bundled. `wheels.yml` closes the glibc-manylinux gate.
