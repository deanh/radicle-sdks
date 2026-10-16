# Radicle SDKs — Python and TypeScript alpha

Typed Python and Node.js/TypeScript bindings for the
[`radicle`](https://crates.io/crates/radicle) crate, pinned to **`=0.25.1`**.
Both SDKs support **local signed project, issue and patch workflows**:
profile/signer setup, project init/fork/checkout, storage and Git inspection,
issue discussions and common writes, and patch revisions/reviews/lifecycle.

| SDK | Package / import | Runtime | Documentation |
| --- | --- | --- | --- |
| Python | `radicle-py` / `radiclepy` | CPython ≥ 3.9 (abi3) | [Guide](python/README.md), [API](python/API.md), [changelog](python/CHANGELOG.md) |
| TypeScript | `radicle-sdk` | Node.js ≥ 26.12.0 | [Guide](node/README.md), [API](node/API.md), [changelog](node/CHANGELOG.md) |

These are **experimental, source-installed alphas**, not nodes or network
clients. Both package versions are `0.1.0`; the **exact reviewed Git commit**,
not the version, identifies a candidate. APIs may change. PyPI/npm registry
publication and portable prebuilt distribution are deferred.

Git installation compiles native code and requires **Rust ≥ 1.91**, Git and a C
toolchain; a cold build can take minutes. Python targets Linux/macOS; Windows is
unsupported/unverified. The TypeScript alpha is validated on **Linux x86_64
GNU/glibc only**. See each guide for installation, prerequisites and limitations.

Fetching/announcing projects, connections and daemon control are outside the
SDKs. Git handles commit creation/import and working-copy pushes. Python also
exposes local policy management, routing reads and raw COB inspection. Full API
parity is not the alpha goal, and no shared Rust core has been extracted yet.

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
