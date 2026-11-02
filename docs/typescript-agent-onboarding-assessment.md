# TypeScript SDK: agent-onboarding assessment

## Summary

The TypeScript SDK has a strong machine-readable surface and a precise installed
API contract, but—like the Python SDK—it is optimized for **contract
verification**, not **task discovery**. An agent can infer correct automation
from the declarations and documentation, but it must first navigate installation
detail, a large all-in-one quick start, a 580-line API contract, and shared
workflow semantics.

The highest-impact improvement is a short, task-oriented front door that explains:

1. which Radicle automation tasks are supported;
2. the handle path from profile to repository to issues/patches;
3. which calls are synchronous versus asynchronous;
4. when the task belongs to Git rather than the SDK; and
5. which returned snapshot proves the mutation's effective result.

## What is already strong

- The package ships `node/README.md`, `node/API.md`, and generated declarations,
  so installed-package documentation is substantially self-contained.
- `node/dist/index.d.ts` gives agents a concise public class/method inventory;
  generated DTOs provide readonly fields and useful literal/template-literal
  domains such as patch states and `branch:${string}`.
- The result-envelope design is automation-friendly. Most mutations return the
  post-operation `snapshot`, avoiding an unnecessary second read.
- `RadicleError.code` and Node-style argument error codes provide a clear error
  classification model.
- Private constructors and factory methods make valid handle creation paths
  visible in both declarations and runtime behavior.
- Async native work is explicit, with `loadAsync()` and `signerAsync()` available
  for interactive applications.
- Examples and the README quick start are executed by tests, including from an
  installed package.
- The core path is coherent once learned:
  `Profile -> Storage -> Repository -> Issues/Patches/Raw`.

These are strong foundations for coding agents and editor-assisted discovery.

## Evidence from the `rad-pi` integration

The SDK-first spike in `../rad-pi` sharpens this assessment because it is a real
agent-facing consumer rather than a hypothetical onboarding exercise.

### What demonstrably worked

- **Typed snapshots replaced terminal parsing.** The integration reports that
  issue/patch tools became simpler and more reliable once they consumed evaluated
  SDK values instead of CLI tables or ad-hoc JSON.
- **Mutation envelopes improved truthfulness.** Tools expose `entryId` together
  with the observed `snapshot`; patch review explicitly returns both the requested
  verdict and the review that remained effective. This prevented an agent from
  treating “transaction recorded” as “requested state applied.”
- **The detailed semantic documentation was actionable.** The consumer correctly
  implemented full-set label/assignment replacement, an application-level
  read/modify/write queue, no blind retry, lazy signer acquisition, explicit
  announcement, and Git transport for working-copy patches.
- **Verified project metadata improved automation decisions.** The integration
  stopped assuming `main` and used the SDK identity's default branch.
- **Generated types helped integration quality.** The spike records that prior
  TypeScript inference problems disappeared after switching to structured SDK
  results.

### What the skill shows about good onboarding

`packages/core/skills/radicle/SKILL.md` is close to the missing SDK front door.
It succeeds by being a short **capability router**, not an API reference:

1. a task-to-tool table;
2. a separate table for deliberate CLI/Git bridges;
3. a few high-risk semantic rules;
4. unsupported operations; and
5. error/cancellation guidance.

This suggests `node/AGENT.md` should follow that shape and remain shorter than a
recipe catalogue. Detailed examples and signatures can stay in linked references.
The strongest phrases are concrete operational distinctions: “signed locally
only,” “announce separately,” “read `.snapshot`,” “recorded does not mean
applied,” and “a COB merge record is not a Git merge.”

### Friction confirmed by implementation code

- Issue creation returns a mutable handle while later mutations return envelopes;
  `rad-pi` immediately calls `reload()` to manufacture `{ issueId, snapshot }`.
  This is a genuine small consistency cost worth documenting or revisiting.
- Prefix resolution is consumer glue built from `Raw.entities()`. Full IDs are
  safest, but agent interfaces often receive short IDs, so ambiguity behavior
  belongs in onboarding guidance and may merit a future SDK helper.
- `all()` plus consumer filtering is workable and clear, but eager for polling.
- Commit transport, diff generation, current-worktree patch checkout, and network
  sync remain intentional Git/CLI boundaries—not evidence that the local SDK API
  or its documentation failed.

Two historical gaps in the spike have since improved in the current SDK:
`projectAt()` now supplies bounded working-copy discovery, and
`Profile.loadAsync()` / `signerAsync()` address blocking setup. This is evidence
that consumer-oriented findings are useful API-shaping inputs; the quick guide
should make these newer preferred paths obvious.

## Where agents lose time

### 1. No task-to-API map

The guide describes scope and the API reference inventories capabilities, but
neither starts from user intent. An agent asked to “review the latest revision”,
“open an issue”, or “find the project for this checkout” must search and compose
the workflow itself.

### 2. Installation dominates the front of the guide

The first half of `node/README.md` covers platforms, registry/source installs,
npm versions, pnpm approval, Bun limitations, native compilation, and release
behavior. This is important for operators, but delays API understanding for an
agent working in a project where `radicle-sdk` is already installed.

### 3. The quick start is a validation program, not a quick orientation

The runnable example correctly exercises project init, policy/routing, issues,
raw COBs, Git import, signing, patches, revisions, comments, and reviews. Its
breadth makes it excellent release evidence, but too large as the first API
example. Most automation begins with an existing profile and working copy.

### 4. The mixed sync/async model requires synthesis

The API intentionally mixes:

- synchronous metadata/factories such as `profile.storage`,
  `storage.repository()`, `Issues.open()`, and `Raw.open()`;
- optional blocking setup via `Profile.load()` and `profile.signer()`; and
- Promise-returning I/O and mutation methods.

This is correctly documented, but an agent lacks a compact rule such as “prefer
`loadAsync`/`signerAsync`; factories are often synchronous; operations are
async.” Missing or unnecessary `await` is an avoidable onboarding failure mode.

### 5. Critical workflow rules are distributed

The rules most likely to cause incorrect automation are spread across the API
contract and shared semantics:

- the SDK is local and does not fetch, announce, or control a node;
- Git creates/imports commits and moves working-copy branches;
- SDK patch creation/update requires commits already in storage;
- issue/patch labels and assignees replace the complete set;
- snapshots are detached and frozen, except copied `Uint8Array` fields;
- patch `merge()` records a COB decision, not a Git merge;
- reviews retain the first review per actor/revision;
- mutation success must be judged from the returned snapshot, not only an entry
  ID or `changed` boolean.

These should appear before write recipes.

### 6. Type declarations are precise but split by implementation boundary

`index.d.ts` is a concise public map, while many DTO definitions come from the
generated `native.d.cts`. Editors resolve this correctly, but an agent reading
files directly must follow the import and distinguish public wrapper types from
unsupported native exports. The API reference explains this, but there is no
single compact public-surface document.

### 7. Small read examples are not the main entry point

`release/demo/1-read.mjs` is much closer to agent-sized onboarding than the main
quick start, but it is not linked as the primary “existing profile” path and uses
a repository-specific default.

## Recommended changes, in priority order

### P0: Add one TypeScript agent quick reference

Add `node/AGENT.md` and link it near the top of `node/README.md`. Keep it around
100–150 lines so it can be placed directly in an agent context. Include:

- a one-paragraph scope statement;
- the handle/object model;
- a task-to-API table;
- a sync/async rule table;
- an “SDK or Git?” decision table;
- five minimal recipes;
- mutation postconditions and hazards;
- links to `API.md` and shared semantics.

Suggested task table:

| User intent | Start here |
|---|---|
| Find the project for the current checkout | `await projectAt(path)` |
| List locally stored projects | `await profile.storage.repositories()` |
| Inspect Git/project metadata | `profile.storage.repository(rid)` |
| Read issues or patches | `Issues.open(repo)`, `Patches.open(repo)` |
| Create/update an issue | `Issues.openAs(repo, signer)` |
| Review/comment on a patch | `await Patches.openAs(repo, signer).getMut(id)` |
| Inspect unknown COB types | `Raw.open(repo)` |
| Follow/seed locally | `await profile.policyStore()` |
| Fetch, announce, or control a node | Not supported |
| Create commits or push a working copy | Use Git / `git-remote-rad` |

### P0: Lead with an existing-profile recipe

The first API example should be small and representative:

```ts
import { Issues, Profile, projectAt } from 'radicle-sdk';

const [profile, location] = await Promise.all([
  Profile.loadAsync(),
  projectAt('.'),
]);
if (location === null) throw new Error('not a Radicle working copy');

const repo = profile.storage.repository(location.rid);
for (const issue of await Issues.open(repo).all()) {
  console.log(issue.id, issue.state, issue.title);
}
```

Then show the write delta and the result-envelope rule:

```ts
const signer = await profile.signerAsync();
const issues = Issues.openAs(repo, signer);
const issue = await issues.create({
  title: 'Title',
  description: 'Description',
  labels: ['automation'],
});
const result = await issue.comment('Created by automation');
console.log(result.entryId, result.snapshot.comments.length);
```

State each recipe's preconditions directly above it.

### P0: Document the async model in six lines

A compact table would prevent many mistakes:

| Kind | Behavior |
|---|---|
| `Profile.load()` / `profile.signer()` | Synchronous and potentially blocking |
| `Profile.loadAsync()` / `signerAsync()` | Preferred async setup |
| Handle factories/getters | Often synchronous and perform little/no I/O |
| Repository/store reads and writes | Return Promises; `await` them |
| Mutation results | Inspect `result.snapshot` |
| Cancellation | Promises have no native in-flight cancellation |

### P1: Add an automation safety checklist

Place this before write recipes:

- Discover or validate the RID; do not guess it.
- Load a signer only for writes.
- Await every operation before consuming its result.
- For mutations, inspect the returned `snapshot`; use `reload()` only when a
  fresh read is needed after another writer or when creation returned a handle.
- Serialize complete read/compute/write sequences for replacement labels and
  assignees.
- Do not retry failed writes blindly; inspect persisted state first.
- Never describe `PatchMut.merge()` as performing a Git merge.
- Convert `bigint` explicitly before `JSON.stringify` when using raw COB data.

### P1: Improve package-level discovery

- Include `node/AGENT.md` in the published package.
- Add more JSDoc to public declarations for the highest-risk methods:
  `create/update/review/merge`, label/assign replacement, and sync setup calls.
- Add a `Documentation` URL or package export/documented path that makes
  installed `node/API.md` and the agent guide easy to locate.
- Consider generating a compact public API inventory from the wrapper
  declarations so agents do not need to inspect `native.d.cts` for DTO shapes.

### P2: Separate onboarding from exhaustive validation

Keep the current comprehensive quick start because it is valuable tested release
evidence, but label it “full local workflow example” and precede it with the
small existing-profile recipes. Add a top-level API table of contents and common
workflow links rather than expanding the README further.

## TypeScript-specific onboarding target

An agent should be able to answer these within one minute and one document:

1. Is this local task supported, or does it require Git/a node?
2. Which handles do I need and which factory creates them?
3. Which calls require `await`, and should setup use the async variant?
4. Which returned snapshot proves the requested write took effect?
5. Are any values `bigint`, mutable byte copies, or replacement sets?
6. Which exact API section covers edge cases and error codes?

The SDK already exposes a good agent-facing API. The main opportunity is to put
its usage model ahead of installation detail and exhaustive behavioral caveats.