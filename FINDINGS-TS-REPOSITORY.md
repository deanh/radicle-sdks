# TypeScript v0 repository increment

Scope: the remaining proposal §2 storage/repository operations, on top of merged
`main` at `dc5e209`. No Python implementation changes, dependencies, shared core,
networking, commit import, general ref writes, publication or scheduler.

## Contracts exercised

- Async storage discovery/membership; lazy repository handles and genuine errors.
- Sorted canonical branch names, symbolic resolution, absent versus corrupt refs;
  namespaced actor branches are not canonical branches. libgit2 itself skips some
  malformed loose refs during enumeration; returned iterator errors still reject.
- Tree canonical order, binary names, directory/executable/symlink/gitlink modes;
  independent mutable byte copies and frozen DTO containers.
- Commit tree/parent navigation and complete name/email/date/offset/message fields,
  including lossy text conveniences and optional declared encoding.
- History revision expressions, linear/merge ancestry, real 1000-entry clamping,
  safe-integer validation, empty pages, offset and lookahead-based `hasMore`.
- Explicit upstream ref signing; detached metadata/ref maps from its own result,
  mixed issue/patch/signing writes and independent signer handles/worker realms.
  The upstream ref map excludes structural `refs/rad/sigrefs-parent`; `parent`
  exposes that metadata. Unchanged signing may reuse the existing commit.

## History error propagation / separate Python follow-up

A direct port of Python's `walk.skip(offset)` silently discards errors encountered
in skipped ancestry. A commit with a missing parent reproduces this on Python at
`dc5e209`: offset 0 raises `GitError`, while offsets 1 and 10 return empty success.
The TS increment checks every consumed walk entry, including skipped entries, and
rejects `GIT` for all three offsets. A regression test covers this. Valid history
ordering/pagination is unchanged. Correcting the Python implementation remains a
separate follow-up; its passing existing suite does not cover this edge case.

## Local validation

Linux x86_64, Node 26.12.0, Rust 1.91.0; installed stable for additional Clippy/fmt:

- 25 Node tests, 4 native Rust tests, TypeScript consumer checks pass.
- Rust 1.91/stable Clippy (`--all-targets --locked -- -D warnings`), formatting pass.
- 192 Python helper tests pass, with Python sources unchanged.
- Installed-consumer smoke includes discovery, branch/tree/blob/history reads and
  signing after a real Git import, plus the existing three local workflows.

Exact-commit Git source-install gates use `node/scripts/git-install-smoke.sh`
(`npm`, pinned `npm12`, `pnpm`) after committing sources. Existing platform, Bun,
agent, libuv contention and external-process coordination limits still apply;
this is not evidence of remote CI or additional supported platforms.
