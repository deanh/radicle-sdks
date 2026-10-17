# Intentional SDK differences

Both SDKs bind Radicle **`=0.25.1`** for local signed workflows. Shared
[workflow semantics](semantics.md) do not require identical public interfaces or
full parity. The [Python API](../python/API.md) and
[TypeScript API](../node/API.md) remain the language-specific contracts.

## Surface and setup

| Concern | Python | TypeScript / Node |
| --- | --- | --- |
| Package/import | `radicle-py` / `radiclepy` | `radicle-sdk` |
| Runtime | CPython >=3.9, abi3 | Node >=26.5.0, no upper cap |
| Platform boundary | Linux/macOS targets; Windows unsupported/unverified | Linux x86_64 GNU/glibc validated; other platforms unverified/unsupported |
| Execution | synchronous | repository/COB/project work async; `Profile.load()` and `signer()` synchronous |
| Repository handle creation | validates existence immediately | lazy; absence fails on first I/O |
| Typed COB store creation | `open/open_as`, lazy | `open/openAs`, lazy |
| Missing issue/patch/branch lookup | `None` | `null` |
| Filesystem input | `str` or `os.PathLike[str]`; no bytes paths | string paths |
| Extra surfaces | policy management, routing reads, raw COB inspection, more repository metadata/ref maintenance, bare checkout | those additions deferred; checkout non-bare |
| Aliases and ID validators | public lookup facade and `parse_*` functions | no equivalent public convenience surface |

Neither SDK loads a custom profile environment adapter. Upstream resolves home,
passphrase and agent settings from the native process environment. Configure them
before concurrent work. Node worker-local environment copies do not multiplex
native profiles/signers; synchronous setup can block the calling JS thread.

## Timestamp table

Values are epoch-based; offsets are separate metadata, not adjustments to the
stored timestamp. Python integers preserve the underlying stored integer; they
are not automatically converted into Python datetime objects.

| Source | Python | TypeScript |
| --- | --- | --- |
| Commit author/committer time | `int`, seconds | required `number \| null`, milliseconds |
| Typed comment/revision/review time | `int`, milliseconds | required `number \| null`, milliseconds |
| Raw COB operation time | `int`, seconds | raw API not exposed |
| Routing last-seen time | `int \| None`, milliseconds | routing API not exposed |

Node checks overflow and the ECMAScript Date range (absolute milliseconds at
most 8,640,000,000,000,000). Stored times outside that range become `null`, not
rounded numbers, and do not hide an object or reject a whole listing. Genuine
store/decoding errors still reject. No raw/BigInt timestamp API is promised.

## Values, mutations and errors

| Concern | Python | TypeScript |
| --- | --- | --- |
| Git bytes | immutable `bytes`, lossy `*_str` accessors | independent mutable `Uint8Array` copies, lossy `...Str` views |
| Snapshot containers | immutable value objects; collection getters return copies | recursively frozen ordinary objects/arrays; byte arrays remain mutable copies |
| Value comparison | structural equality/hash for immutable snapshots; handles use identity | normal JS object identity; no structural equality/hash API |
| Mutable handles | cache `.issue`/`.patch`; operations refresh that snapshot; `reload` observes other writes | cache no snapshots; writes return operation/result envelopes with detached snapshots |
| Mutation return | operation/revision ID or lifecycle bool, depending on method | `{entryId,snapshot}`, `{revisionId,snapshot}`, or `{changed,snapshot}` |
| Input failures | validators raise `ValueError`; argument conversion can raise `TypeError`/`OverflowError` | `TypeError` with `ERR_INVALID_ARG_TYPE`/`ERR_INVALID_ARG_VALUE` |
| Operational failures | `radiclepy.Error` subclasses | `RadicleError` with stable binding error codes |

Both expose IDs as strings and finite domains in typing declarations, but those
annotations do not replace runtime validation. Both SDKs accept patch targets
`delegates`, `branch:main` and `branch:refs/heads/main`; the branch forms
canonicalize to `branch:main` in snapshots. The `branch:` prefix is required
for branch targets in both SDKs.

## Concurrency and failure handling

Python signers, mutable COB stores/handles and policy stores are thread-affine.
There is no async/thread-pool contract; project init/checkout retain the GIL,
including while upstream invokes Git. Repository handles hold no live Git
handles and reopen per operation; that is not an external-writer coordination
mechanism.

Node reads stay parallel. Writes using the same addon share a repository mutex
keyed by canonical storage path and RID, followed by the signer guard. Reload,
transaction, signing and result materialization happen under the write lock.
Await a write for read-your-write ordering. There is no JS write queue, fairness,
read-latency or cross-addon-copy/process coordination guarantee. Mutex waiters can
occupy libuv worker threads. Init/checkouts have destination-specific concurrency
limitations described in the TypeScript reference.

Node worker/synchronous entry points contain unwinding Rust panics, without
returning panic payloads; abort builds are rejected. This is not process
isolation or permanent repository quarantine. Weak repository locks can expire;
signer poisoning persists for handles sharing that signer. Python does not claim
Node's panic/error protocol. Neither SDK adds rollback or automatic retry.

## Distribution and evidence

Both SDKs install from Git at an exact commit; publication and platform status
are in the root README's [scope and non-goals](../README.md#scope-and-non-goals).
Python Git installation builds its extension, with the
[Python guide](../python/README.md) as package description. Node Git preparation
builds the addon in the full checkout, and installed consumers load that output.

Python wheel/sdist workflows and npm 11/npm 12/pnpm exact-commit consumer tests
establish different packaging guarantees. Bun Git preparation remains best
effort; browsers and Deno are unsupported. See [development](development.md)
for what each validation gate proves and what remains unverified.
