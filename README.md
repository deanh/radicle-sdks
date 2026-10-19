# Radicle SDKs — Python and TypeScript alpha

Typed Python and Node.js/TypeScript bindings for the
[`radicle`](https://crates.io/crates/radicle) crate, pinned to **`=0.25.1`**.
Both SDKs support **local signed project, issue and patch workflows**:
profile/signer setup, project init/fork/checkout, storage and Git inspection,
issue discussions and common writes, and patch revisions/reviews/lifecycle.

| SDK | Package / import | Runtime | Documentation |
| --- | --- | --- | --- |
| Python | `radicle-py` / `radiclepy` | CPython ≥ 3.9 (abi3) | [Guide](python/README.md), [API](python/API.md), [changelog](python/CHANGELOG.md) |
| TypeScript | `radicle-sdk` | Node.js ≥ 26.5.0 | [Guide](node/README.md), [API](node/API.md), [changelog](node/CHANGELOG.md) |

## Scope and non-goals

Both SDKs do one job: **local, signed project, issue and patch workflows** on a
Radicle home you already have. The other documents link here rather than
restating these points.

- **Alpha, installed from Git.** Both packages are version `0.1.0`; the exact
  reviewed commit identifies a candidate, and APIs may change. Registry
  publication and prebuilt binaries are deferred.
- **Not a node.** Fetching, announcing, connections and daemon control are out
  of scope, and nothing in the SDKs talks to the network.
- **Git owns working-copy content.** The SDKs never create or import content
  commits and never push a working copy; use Git (and `git-remote-rad`) for
  that. They do write to local storage: project init, fork and checkout set up
  namespaces and remotes, the explicit canonical-ref writes (for example
  `set_default_branch_to_canonical_head` and `sign_refs`) move storage refs,
  and signed issue/patch writes create change commits of their own.
- **One operation at a time.** Each write is one upstream transaction. There is
  no multi-call transaction, rollback or automatic retry; after a failure,
  inspect persisted state before continuing.
- **Thin adapters, no shared core.** Each binding adapts the upstream crate
  directly; no common Rust crate has been extracted. Upstream API completeness
  is not a goal; [Python capability parity](docs/node-parity.md) is in progress.
  Both SDKs expose local policy management, routing reads and generic raw COB
  inspection, with intentional language-specific representations.
- **Platforms.** Building from Git needs **Rust ≥ 1.91**, Git and a C toolchain,
  and a cold build takes minutes. Python targets Linux and macOS (Windows is
  unverified); the TypeScript alpha is validated on Linux x86_64 GNU/glibc only.
  Each guide lists its own prerequisites.

## Try the SDKs

The runnable quick starts create temporary local data:
[Python](python/README.md#runnable-local-quick-start) and
[TypeScript](node/README.md#runnable-local-quick-start).

The [project digest](examples/README.md) is a read-only Markdown report of local
patches, latest-revision reviews, merged patches, open issues by label and items
assigned to you. [Python](examples/digest.py) and [TypeScript](examples/digest.mjs)
implement the same program, with tested byte-identical output. They use an
existing profile without loading a signer or making signed writes. A sandboxed
write-side review-bot example is planned separately, not implemented yet.

## Shared documentation

- [Architecture](docs/architecture.md): current ownership and build boundaries.
- [Workflow semantics](docs/semantics.md): Git versus COB writes, signing,
  revisions, merge records and failure limits.
- [SDK differences](docs/sdk-differences.md): intentional API, conversion and
  concurrency differences.
- [Development and validation](docs/development.md): commands and evidence gates.
- [Documentation index](docs/README.md): guides and preserved historical findings.

The architecture migration is tracked in Radicle issue `edcbbd7`. Current code
layout and proposed follow-ups are distinguished in the architecture guide.

## License

Dual **MIT OR Apache-2.0**, matching upstream. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
