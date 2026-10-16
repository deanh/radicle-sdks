# TypeScript T0 findings

Status: **local T0 feasibility demonstrated** on `ts-t0`; release limitations
remain below. This is not TypeScript v0 or authorization for shared-core
extraction. Native CI has been added but has not been run remotely.

## Environment and layout

- Linux x86_64, Node 26.12.0, npm 11.20.0, Rust 1.97.1.
- `radicle = "=0.25.1"`; napi 3.14.2, napi-derive 3.6.12, CLI 3.10.8.
- TypeScript 5.9.3, @types/node 26.6.5; pnpm 12.11.2 and Bun 1.4.3 probes.
- Root `package.json` (`radicle-sdk`, private prerelease) with source-build
  `prepare`; Rust package/lockfile isolated in `node/native`. Python manifests,
  crate and source layout are untouched.
- T0 builds a debug addon; timings are diagnostics, not release benchmarks.
- No npm publication or prebuilt platform distribution.

## Demonstrated so far

- Workflow 1: profile init → project init → head/identity → fork → checkout.
- Commit reads return copied Uint8Array fields and exact millisecond timestamps.
- Init preflight preserves the existing `rad` remote/configuration.
- Issue create/get/getMut/reload/comment; read-only and argument errors.
- Ordinary DTO containers recursively frozen; byte copies remain mutable and
  detached. Result envelopes pair entry IDs with snapshots from the transaction.
- Same-handle/separate-handle concurrent comments, failure recovery, and three
  `worker_threads` opening their own profile/repository/signer handles.
- Twelve cross-worker writes produce twelve unique serialized snapshots and no
  missing comments. A read queued after all workers submit their bursts reveals
  pool contention: example debug run ~1578ms for writes, ~1040ms read latency,
  default libuv pool of four. An earlier read submitted before the write burst
  finished in ~7ms, which did not exercise saturation. No latency guarantee.
- Generated Rust `.d.cts` provides readonly fields/collections and finite
  state/visibility domains. A thin TS wrapper compiles to its own declarations;
  compile-only consumer tests check readonly errors, literal domains, bytes and
  Promise signatures. DTO shapes are not maintained twice.
- Worker-task errors are converted into real RadicleError instances with stable
  codes; native converter errors become TypeError. Internal timestamp probes
  exercise RangeError translation without adding a public timestamp-input API.
- Checked timestamp arithmetic and Date boundaries, signed multiplication
  overflow and unsigned upstream maximum rejection are tested in Rust/JS.
- Reproducible Send/Sync assertions are in the Rust test module: Signer Send but
  not Sync; Profile/Storage Send+Sync; Repository Send but not Sync.
- Current validation: eight Node tests, two Rust unit tests plus compile assertions,
  TypeScript consumer checks, Clippy, and all 192 existing Python tests pass.

## Material findings

### Standalone feature graph differs from Python

A fresh standalone dependency resolution failed compiling radicle's IndexMap/
IndexSet uses because their std-only default hasher was unavailable. Enable
`indexmap/std` explicitly in the spike. Python's dependency feature graph enables
it incidentally. The committed standalone Cargo lockfile and `--locked` builds
make this reproducible; copying the Python lockfile is not the solution.

### Rust panic containment (follow-up)

Worker closures now catch unwinding panics and reject `INTERNAL` instead of
unwinding through napi-rs's C worker callback and aborting Node. Native synchronous
entry points use napi-rs `catch_unwind` guards too. Worker errors omit panic
payloads; the default Rust panic hook can still print diagnostics. A subprocess
regression covers worker and synchronous panics, then a successful timestamp task
and profile initialization. A Rust test confirms locks remain poisoned and jobs
are consumed, not retried. This does not roll back writes or contain abort/OOM,
native faults or double panics during unwinding. Abort-mode builds are rejected.

### Synchronous upstream profile/signer setup, matching Python (follow-up)

The original spike ran upstream `Profile::load()`/`signer()` on libuv workers.
An initial fix captured JS environment values and mirrored upstream loading and
agent selection in a custom adapter. Review rejected that duplication. The
adapter and its direct `ssh-agent-lib` dependency have been removed.

`Profile.load(): Profile` and `profile.signer(): Signer` now call upstream
synchronously on the calling JS thread, matching the Python SDK. They return
handles immediately and throw translated errors synchronously. Filesystem I/O,
key decryption and agent communication can block that thread; this is an explicit
setup trade-off, not async work disguised behind a Promise. `Profile.init()` and
repository workflows remain async. The native calls retain panic guards.

Environment selection follows upstream native OS semantics: `RAD_HOME`/`HOME`,
`RAD_PASSPHRASE` and `SSH_AUTH_SOCK`. Node worker-local `env` copies do not change
these native values. Configure environment before OS process startup; concurrent
process-global environment mutation/`SHARE_ENV` is not supported. No custom
loader, signer implementation, environment write or passphrase cache remains.

Fresh-process tests exercise HOME fallback/RAD_HOME precedence, encrypted keys,
plaintext-key precedence, missing homes, wrong passwords, and empty-passphrase/
missing-agent rejection. Tests assert immediate handles and synchronous typed
throws. A worker with conflicting JS env values still uses the OS profile and
signer settings. Another worker uses a throwaway Unix SSH agent to initialize a
project and create/comment on an issue. No test registers keys with the user's
agent or mutates the main process environment.

### Locking works across workers; not every source of concurrency is covered

The registry uses canonical storage path + RID and weak Arc entries, discarding
dead keys. The repository mutex encloses reload, transaction, signing and
snapshot materialization. The signer mutex follows it and stays held for the
upstream WriteAs transaction. Metadata getters never wait on the signer lock.

No JS queue was added. Mutex waiters consume libuv workers, so pool contention
remains possible. Different processes, external Git/CLI writes and independently
loaded addon copies/versions are not coordinated. Init has no RID yet; concurrent
init on the same work directory and checkouts to the same destination are outside
the tested contract.

### Declared Rust 1.88 does not compile the pinned upstream

`cargo +1.88.0 check --manifest-path node/native/Cargo.toml --locked` fails in
radicle 0.25.1 `src/cob/identity.rs:506–507` with E0382: the `action @ (...)`
pattern moves `action` and also uses the revision binding. Rust 1.97.1 compiles
it. This is an observed upstream compatibility blocker, not an addon error.

Do not claim alpha installation works with Rust 1.88 on this evidence. No
upstream source patch, pin bump or Python MSRV change was made. CI reproduces
this explicitly as a non-blocking known failure, alongside the passing stable
checks. A release decision must resolve or document the actual supported MSRV.

**Resolved after review: minimum Rust is 1.91.0.** Rust 1.88.0, 1.89.0 and
1.90.0 fail with this E0382; 1.91.0 builds. The Python crate fails identically,
since the error is in radicle's own source, so radicle-py's documented 1.88 was
also wrong and is being raised to 1.91 (Radicle patch `dc3e969`). No newer
radicle release exists (0.25.1 is latest, still declaring 1.88.0). Upstream main
has rewritten the match without the `action @` binding (a minimal reproduction
of the new form builds on 1.88), but main pins a 1.98.0 toolchain, so the
declared minimum isn't exercised upstream. This crate now declares
`rust-version = "1.91"`, and the CI step is a blocking 1.91 check. Re-measure on
every radicle pin bump.

## Source-install gate

Exact-commit root Git installs (`git+file://...#491f7e4...`, repeated at
`2f07ca6999c3c4306c9b9c5b859012785b312773` with isolated stores) passed with
**npm 11.20.0** and **pnpm 12.11.2**. Logs showed `prepare` compiling the addon from
Git-cloned sources, generating declarations and running the installed workflow.
No development dependency is required in the final consuming application.

pnpm's `--allow-build radicle-sdk` alone failed with
`ERR_PNPM_GIT_DEP_PREPARE_NOT_ALLOWED`. Approving the exact reviewed Git source
(`radicle-sdk@git+URL#SHA: true`) in the consumer's `pnpm-workspace.yaml`
`allowBuilds` map succeeded. The smoke script and README use that precise opt-in,
not a global allow-all setting. npm emitted a script-approval warning but did
execute Git prepare in this tested configuration.

**Bun Git installation did not pass**: Bun 1.4.3 ran prepare without installing
the development build tools, so `napi` was missing. This remains best effort;
no workaround adding compiler tools to production dependencies was introduced.
A separate Bun runtime probe successfully ran the Node-built addon's complete
installed-workflow smoke (Bun reports process.version v26.3.0). Runtime viability
is not evidence that Bun's own source-install path works.

`node/scripts/git-install-smoke.sh` creates fresh consumers, isolates npm/Bun
caches and the pnpm store, and runs workflows using installed exports. Only Cargo
dependency artifacts are reused; no built addon is committed. Remote clone
transports and native macOS/aarch64 portability are not established by a local
Git-URL test. The workflow includes npm/pnpm gates and a non-blocking Bun probe.

## Final local validation record

At source commit `2f07ca6999c3c4306c9b9c5b859012785b312773`:

- `npm run build`, `npm run check`, `npm test`: pass (five Node test cases).
- Rust unit test plus compile-time positive/negative Send/Sync assertions: pass.
- Native Clippy with all targets and `-D warnings`, and Rust formatting: pass.
- Fresh npm and pnpm root Git consumers run the installed workflow successfully.
- `npm pack --dry-run` includes the generated declarations and native artifact,
  excludes Cargo target directories and node_modules (16 files, ~22.6 MB compressed
  for this debug build; not a prebuilt distribution or portability claim).
- Existing Python helper-build suite: 192 passed. A fresh CPython 3.9 environment
  also installs Python from this exact root Git commit and passes release-purity/
  version/workflow smoke (`tests/wheel_smoke.py`). The root npm manifest does not
  break the demonstrated Python Git-install path.
- Rust 1.88–1.90: observed pinned-upstream failure; 1.91 builds (resolved, above). Bun Git prepare: observed
  failure; Bun runtime on the Node-built addon: pass. No remote CI result claimed.

### Follow-up validation: panic and environment fixes

On Node 26.12.0, the addon built with Rust 1.91.0 passes all eight Node tests
(no skips), TypeScript consumer checks and both Rust unit tests. Native Clippy
passes on Rust 1.91.0 and stable with `-D warnings`; formatting is clean. A fresh
Python helper rebuild still passes all 192 tests. Python source/layout and its
manifest are unchanged by these fixes. No remote CI or broader agent/platform
coverage is claimed.

Before the synchronous simplification, fresh npm/pnpm root-Git consumers at
`da290ec7f98c34f3e96b3afa51c907b0ff14343d` passed the installed workflow smoke.
The first npm attempt hit a transient registry `ECONNRESET`; retry passed. pnpm's
exact-source approval requires the full 40-character SHA, not an abbreviated
revision (the abbreviated approval was rejected before preparation).

## Decision boundary

Do not start T1/T2 automatically. napi-rs is viable for the tested local
workflows, types and concurrency model, and the root Git packaging strategy works
with npm/pnpm. Recommended next step: review these findings and choose whether
to continue to the v0 surface. No evidence requires a JS scheduler yet; the
measured burst delay should inform consumers rather than trigger one by default.

The Rust MSRV is resolved: 1.91. Keep Bun source installation best effort. Shared-core extraction remains
an independent decision: this spike demonstrates another binding, not that a
broad Python rewrite pays for its packaging/compatibility risk. No language-neutral conformance framework,
network API, patch API, extra runtimes or general scheduler was introduced.
Encrypted-key worker integration and an isolated local Unix SSH agent are now
validated; other agent implementations, hardware tokens and native platforms
remain unverified. See the follow-up environment finding above.
