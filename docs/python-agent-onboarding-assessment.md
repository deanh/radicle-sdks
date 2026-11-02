# Python SDK: agent-onboarding assessment

## Summary

The Python SDK is unusually well specified for an alpha, but it is optimized for
**contract verification**, not for **task discovery**. An agent can eventually
produce correct automation from the current docs, yet it must first synthesize a
164-line guide, a 900-line API reference, shared semantics, and examples.

The fastest improvement is not a smaller API. It is a short, task-oriented front
door that tells an agent:

1. what the SDK can and cannot do;
2. which object path to start from;
3. whether a task belongs to the SDK or to Git;
4. the critical mutation postconditions; and
5. where to look for exact signatures.

## What is already strong

- `python/radiclepy/__init__.pyi` is a complete, typed public inventory with
  useful `Literal` states and concise semantic warnings.
- `python/API.md` is precise about validation, errors, side effects, ordering,
  timestamps, snapshots, and concurrency.
- `python/README.md` has a runnable end-to-end sandbox and clearly states major
  non-goals.
- Examples are executable and tested. The digest and reconciliation programs
  demonstrate realistic automation rather than isolated calls.
- The handle model is coherent once learned:
  `Profile -> Storage -> Repository -> Issues/Patches/Raw`.
- Read and write modes are explicit: `open()` versus `open_as(..., signer)`.

These are strong correctness foundations for agents.

## Evidence from the `rad-pi` integration

`../rad-pi` is a TypeScript consumer, so it does not directly validate Python
syntax or thread-affinity ergonomics. It does, however, provide strong evidence
for the documentation structure proposed here.

Its Radicle skill works as a short **capability router**: one task table for local
structured operations, one table for deliberate Git/CLI bridges, then a compact
set of mutation, unsupported-operation, and failure rules. Detailed CLI material
is moved to a referenced file and loaded only when needed. That is a better model
for `python/AGENT.md` than compressing the entire API reference into another
summary.

The spike also confirms that several existing SDK semantics are valuable rather
than incidental detail:

- typed evaluated snapshots remove terminal-output parsing;
- local signing and network announcement must remain visibly separate;
- verified project metadata prevents hard-coded default branches;
- replacement-set writes require a consumer-owned read/modify/write critical
  section;
- unknown write outcomes must be inspected, not blindly retried; and
- Git transport/diff/current-worktree operations are deliberate boundaries, not
  necessarily missing local SDK methods.

The strongest onboarding language is operational and short: “signed locally
only,” “announce separately,” “recorded does not mean applied,” and “a COB merge
record is not a Git merge.” Python should express the equivalent postcondition
through `reload()` and the refreshed mutable-handle snapshot rather than copying
the TypeScript envelope wording.

## Where agents lose time

### 1. No task map

The front door lists capabilities, but does not map user intents to APIs. An
agent asked “review the latest patch”, “open an issue”, or “inspect the current
working copy” must search the long reference and infer the object path.

### 2. The first example is comprehensive rather than minimal

The README quick start creates Git history, a profile, a project, an issue, a
fork, and a checkout. That proves the lifecycle, but most automation starts from
an existing `RAD_HOME` and repository. A 6–10 line existing-profile example
would provide a much faster default path.

### 3. Critical rules are distributed and repeated

The most important automation rules appear across `python/README.md`,
`python/API.md`, and `docs/semantics.md`:

- the SDK is local and does not fetch or announce;
- Git creates/imports commits and moves working-copy branches;
- patch SDK writes require commits already in storage;
- labels and assignees replace the complete set;
- snapshots do not auto-refresh;
- patch `merge()` records a COB operation, not a Git merge;
- successful mutation calls still require reloading and checking resulting state.

An agent needs these rules before choosing an implementation, not after reading
the full reference.

### 4. Discoverability after installation is weaker than in the repository

The wheel ships type stubs, which is valuable, but not `API.md`. Runtime
introspection is uneven: some native methods have useful signatures/docstrings,
while important entry points such as `Profile` and `init_project` have no
runtime docstring. An agent working only in a consumer project is likely to read
the 827-line stub or search externally.

### 5. Good small demos are not the documented entry point

`release/demo/1-read.py` and `release/demo/2-write-and-detect.py` are closer to
agent-sized onboarding than the main quick start, but they are not linked from
the Python guide and contain repository-specific defaults.

## Recommended changes, in priority order

### P0: Add one agent quick reference

Add `python/AGENT.md` (and link it near the top of `python/README.md`). Keep it
roughly 100–150 lines and stable enough to place directly in an agent context.
It should contain:

- a one-paragraph scope statement;
- the object model;
- a task-to-API table;
- an “SDK or Git?” decision table;
- five minimal recipes;
- a short list of mutation hazards;
- links to the full API and semantics.

Suggested task table:

| User intent | Start here |
|---|---|
| Find the project for the current checkout | `project_at(path)` |
| List local projects | `Profile.load().storage.repositories()` |
| Inspect Git/project metadata | `storage.repository(rid)` |
| Read issues or patches | `Issues.open(repo)`, `Patches.open(repo)` |
| Create/update an issue | `Issues.open_as(repo, profile.signer())` |
| Review/comment on a patch | `Patches.open_as(repo, signer).get_mut(id)` |
| Inspect unknown COB types | `Raw.open(repo)` |
| Follow/seed locally | `profile.policy_store()` |
| Fetch, announce, or control a node | Not supported |
| Create commits or push a working copy | Use Git / `git-remote-rad` |

### P0: Lead with an existing-profile recipe

The first copy/paste example should be:

```python
import radiclepy as rad

profile = rad.Profile.load()
location = rad.project_at(".")
if location is None:
    raise RuntimeError("not a Radicle working copy")
repo = profile.storage.repository(location.rid)
for issue in rad.Issues.open(repo).all():
    print(issue.id, issue.state, issue.title)
```

Then show the write delta separately:

```python
issues = rad.Issues.open_as(repo, profile.signer())
created = issues.create("Title", "Description", labels=["automation"])
print(created.reload().id)
```

Document preconditions immediately beside each recipe.

### P1: Add a compact automation safety checklist

Put this before write recipes:

- Validate/discover the RID; do not guess it.
- Load a signer only for writes.
- Treat snapshots as stale after external or concurrent writes.
- Serialize full read/modify/write sequences for labels and assignees.
- Reload and inspect state after reviews, lifecycle changes, and merges.
- Do not retry a failed write blindly; inspect persisted state first.
- Never describe `PatchMut.merge()` as performing a Git merge.

### P1: Make installed-package discovery self-contained

- Include `AGENT.md` and `API.md` in wheels, not only sdists/repository docs.
- Add a package-level docstring that points to the object model and task map.
- Fill missing runtime docstrings for top-level workflow functions/classes.
- Optionally expose a tiny `radiclepy.capabilities()` or static packaged JSON
  inventory, though the typed stub plus `AGENT.md` is probably sufficient.

### P2: Organize the full API around navigation, not more prose

Keep the detailed reference, but add a top-level table of contents and “common
workflows” links. Avoid expanding the main README further; move edge-case detail
downward and keep the front door task-oriented.

## Target onboarding outcome

An agent should be able to answer these within one minute and one document:

1. Can this task be done locally by the SDK?
2. Do I need Git, a signer, or a running node?
3. Which 3–5 calls form the happy path?
4. What state must I reload or verify afterward?
5. Which exact reference section covers edge cases?

The current SDK already contains the information needed. The main opportunity is
to compress and reorder it around user intent.