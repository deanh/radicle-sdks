# Historical feasibility probe source

The obsolete `spike/` and `wheel-probe/` packages were removed in a separate
cleanup commit after the workspace move. Neither was part of current SDK builds
or CI. The archived findings remain; their original measurements are not current
platform guarantees.

Last-containing repository commit (both directories):
**`b37bd4abe678c7a1bb67580be9b9b1e4349f0d9c`**.

Inspect or recover the exact historical sources without restoring them as
maintained workspace members:

```sh
git show b37bd4abe678c7a1bb67580be9b9b1e4349f0d9c:spike/src/bin/phase2.rs
git show b37bd4abe678c7a1bb67580be9b9b1e4349f0d9c:wheel-probe/src/lib.rs
mkdir -p /tmp/radicle-historical-probes
git archive b37bd4abe678c7a1bb67580be9b9b1e4349f0d9c spike wheel-probe | tar -x -C /tmp/radicle-historical-probes
```

Each recovered directory contains its original manifest and lock; the wheel
probe also contains its original Python build manifest. Historical dependencies,
platform assumptions and build instructions have not been modernized. This is
source retrieval, not a supported build/install path for either SDK.
