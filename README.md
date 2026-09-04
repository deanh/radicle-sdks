# radicle-py

Python bindings for the [`radicle`](https://crates.io/crates/radicle) crate (`=0.25.1`, crates.io authoritative).

**Status: Phase 0A complete (packaging gate → first `wheels.yml` run) → Phase 1 (identity/profile) next.**

- [`PLAN.md`](./PLAN.md) — the design (v3.2), reviewed three times. Canonical; `../radicle-py-plan.md` is a symlink here.
- [`FINDINGS-0A.md`](./FINDINGS-0A.md) — feasibility evidence from Phase 0A spikes.
- `spike/` — throwaway feasibility spikes (Rust; `cargo run`). NOT the binding.
- `wheel-probe/` — throwaway pyo3/maturin wheel feasibility probe. NOT the binding.

Phase 0A verdict: green, with the manylinux leg gated on the first CI run — deterministic
profiles, public `rad::init`, hand conversions with bytes-first UTF-8 policy, raw COB history
traversing `entry.parents` only (identity/related are links, never walked), and an importable
abi3 wheel with libgit2+SQLite statically bundled. `wheels.yml` closes the glibc-manylinux gate.
