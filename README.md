# radicle-py

Python bindings for the [`radicle`](https://crates.io/crates/radicle) crate (`=0.25.1`, crates.io authoritative).

**Status: Phase 0A complete → Phase 1 (identity/profile) next.**

- [`PLAN.md`](./PLAN.md) — the design (v3.1), reviewed twice. Master copy: `../radicle-py-plan.md`.
- [`FINDINGS-0A.md`](./FINDINGS-0A.md) — feasibility evidence from Phase 0A spikes.
- `spike/` — throwaway feasibility spikes (Rust; `cargo run`). NOT the binding.
- `wheel-probe/` — throwaway pyo3/maturin wheel feasibility probe. NOT the binding.

Phase 0A verdict: all green — deterministic profiles, public `rad::init`, hand conversions with
bytes-first UTF-8 policy, raw COB inspection via `entry.contents()`, and an importable abi3 wheel
with libgit2+SQLite statically bundled. One CI follow-up recorded: manylinux rebuild + auditwheel.
