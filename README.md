# Radicle SDKs — Python and TypeScript alpha

Typed Python and Node.js/TypeScript bindings for the
[`radicle`](https://crates.io/crates/radicle) crate, pinned to **`=0.25.1`**.
Both SDKs support **local signed project, issue and patch workflows**:
profile/signer setup, project init/fork/checkout, storage and Git inspection,
issue discussions and common writes, and patch revisions/reviews/lifecycle.

| SDK | Package / import | Runtime | Documentation |
| --- | --- | --- | --- |
| Python | `radicle-py` / `radiclepy` | CPython ≥ 3.9 (abi3) | [Agent quick reference](python/AGENT.md), [guide](python/README.md), [API](python/API.md), [changelog](python/CHANGELOG.md) |
| TypeScript | `radicle-sdk` | Node.js ≥ 26.5.0 | [Agent quick reference](node/AGENT.md), [guide](node/README.md), [API](node/API.md), [changelog](node/CHANGELOG.md) |

## Agent onboarding

Start with the language-specific capability router:
[Python](python/AGENT.md) or [TypeScript](node/AGENT.md). Each maps common user
intents to the smallest relevant API and identifies when the task belongs to Git
or the `rad` CLI instead.

The shared mental model is:

```text
Profile -> Storage -> Repository -> Issues / Patches / Raw
```

Keep four rules in context before automating writes:

1. The SDKs operate on local Radicle storage; they do not fetch or announce.
2. Git creates/imports commits and owns working-copy branches, diffs and merges.
3. Signed issue/patch writes are local until announcement is requested separately.
4. Inspect persisted or returned state after a mutation; recorded operations,
   errors and cancellation are not rollback receipts.

Use the language API references only when exact signatures or edge cases are
needed. The quick references cover signer acquisition, read/write handles,
working-copy discovery, patch transport boundaries and mutation safety.

## Scope and non-goals

Both SDKs do one job: **local, signed project, issue and patch workflows** on a
Radicle home you already have. The other documents link here rather than
restating these points.

- **Alpha, published to PyPI and npm with a Radicle receipt.** Both packages
  are version `0.1.2` and APIs may change. Releases are built on the
  maintainers' machines, recorded as a signed release object on this
  repository through rad-artifact (tag, manifest, digests), and published
  as `radicle-py` on PyPI (from 0.1.1) and `radicle-sdk` on npm (from
  0.1.2); see
  [Releasing](docs/release.md). Git installs at a reviewed commit remain the
  development path.
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
  is not a goal; [Python capability parity](docs/node-parity.md) is implemented
  with explicit language differences and recorded validation boundaries.
  Both SDKs expose local policy management, routing reads and generic raw COB
  inspection, with intentional language-specific representations.
- **Platforms.** Releases from 0.1.1 ship prebuilt artifacts for Linux x86_64
  (glibc 2.28 or newer) and macOS arm64 (macOS 11.0 or newer for the Python
  wheel, 13.5 for the Node addon, Node 26's own minimum); 0.1.0 was Linux
  only. Building from Git needs
  **Rust ≥ 1.91**, Git and a C toolchain, and a cold build takes minutes;
  Python source builds also work on macOS (Windows is unverified). Each guide
  lists its own prerequisites.

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
- [GitHub mirror](docs/github-mirror.md): canonical-ref mirroring policy and script.
- [Documentation index](docs/README.md): guides and preserved historical findings.

The architecture migration is tracked in Radicle issue `edcbbd7`. Current code
layout and proposed follow-ups are distinguished in the architecture guide.

## License

Dual **MIT OR Apache-2.0**, matching upstream. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
