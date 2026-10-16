# TypeScript alpha release polish: Patch B

Issue `fdcfc02`, based on merged Patch A at `6ec6555`. This patch is limited to
non-public handle construction, private initialization and executable user docs.
Package version remains 0.1.0; the exact Git commit identifies each candidate.

## Public handles

All eight handle classes have explicit private constructors in emitted consumer
`.d.ts` files, including the five previously exposing native arguments. No
`stripInternal` trick: deleting a constructor declaration would allow implicit
public zero-argument construction. Native conversion stays in module-private
closures initialized by class static blocks; no public `fromNative` helpers.
A module-private symbol also rejects direct JavaScript construction before any
native handle is stored. This is API hygiene, not a hostile-JavaScript sandbox.

- Positive factory/handle assignment and negative zero-argument/native-looking
  argument tests compile against emitted declarations via the package export.
- A subclass attempt and an invented native factory are rejected by the compiler.
- Runtime tests reject direct construction for every handle, confirm factories
  still work, and check declarations retain all eight private constructors with
  no `raw.Native*` references or leaked conversion helpers.
- Git-install gates repeat runtime checks and compile the same consumer type
  fixture against the actually installed package using TypeScript 5.9.3.

## Private initialization

`InitOptions.visibility?: 'public' | 'private'` defaults to public. Native code
maps private to upstream `Visibility::private([])`; it does not add ACL editing,
encryption, networking or policy APIs. Invalid strings reject `ERR_INVALID_ARG_VALUE`;
wrong types reject `ERR_INVALID_ARG_TYPE`. The public wrapper explicitly rejects
null, which napi otherwise treats as an absent optional field; it must not silently
make an invalid visibility public. Validation precedes initialization side effects.

Tests cover default/explicit public and private identity snapshots, local signing,
fork/checkout, invalid values with unchanged storage/remotes, successful retry,
and preserving an existing private project's remote/identity on reinit failure.
Bare checkout and broader Python parity stay deferred.

## Executable user docs

The README's actual `## Runnable local quick start` JavaScript fence creates a
temporary profile and committed working copy, initializes privately, forks/checks
out, discusses/closes/reopens an issue, then imports Git commits locally and
creates/updates/comments/reviews a patch. It signs refs explicitly after direct
Git writes and cleans up its demo data. No external working copy, daemon or node
network is required. The root README links the TypeScript guide for discovery.

`readme-consumer.mjs` resolves the README through the package's installed entry,
extracts the actual fence without rewriting it, and executes it beneath the
consumer so bare imports resolve to that installation. Local Node tests and all
npm 11/npm 12/pnpm exact-commit gates run it. Separate workflow smoke coverage is
retained. User docs remove dated Node Current/LTS wording, explain cold release
compilation can take minutes, and distinguish warm-cache gate timings.

## Local optimized-build validation

Linux x86_64 GNU/glibc, Node 26.12.0, Rust 1.91.0; fresh isolated Cargo target:

- 30 Node tests pass, including the actual README and panic/worker regressions.
- 4 native Rust tests pass with `--release --locked`.
- Consumer typing, Rust 1.91/stable Clippy, and native Rust fmt pass.
- Exact-commit gates: `node/scripts/git-install-smoke.sh npm11|npm12|pnpm FULL_SHA`;
  these use isolated package-manager caches/stores and shared Cargo dependencies.

No dependencies, Python implementation, publication, supported platforms,
scheduler, networking or shared-core design are changed. These are local checks,
not remote-CI evidence or a claim about Bun's still-unverified Git prepare path.
