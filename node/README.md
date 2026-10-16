# radicle-sdk 0.1.0: TypeScript alpha

A Node.js/TypeScript SDK for **local signed Radicle projects, issues and patches**,
plus storage/repository inspection. It is not a node or a network client.

**Experimental, source-installed alpha.** APIs may change: version `0.1.0` is the
package version; pin the exact reviewed Git commit to identify a candidate.
See the [API reference](API.md) for the implemented surface, conversions, errors,
concurrency and environment contracts, and the [changelog](CHANGELOG.md) for
package history.

## Requirements

The initial alpha supports **Linux x86_64 (GNU/glibc) only**, tested locally.
macOS, Windows, other architectures and musl-based Linux are not supported or
validated yet. Support claims will expand only after source-install and workflow
validation on those platforms; no multi-platform binary distribution is promised.

Requires Node **26.12.0+** (tested baseline: 26.12.0), **Rust ≥ 1.91**, a C
compiler, and Git. The pinned radicle 0.25.1 declares Rust 1.88.0 but doesn't
compile on 1.88–1.90. There is no Node upper-version cap.
npm publication and prebuilt platform packages are deferred.
The working package name is `radicle-sdk`; registry availability isn't assumed.
Browsers and Deno are unsupported; Bun is best effort, as described below.

## Alpha source installation

Set `SDK_GIT_URL` to a clone URL supported by your Git client. The package is at
repository root; do not add an npm subdirectory fragment. Replace `<full-commit>`
with a full reviewed commit SHA:

```sh
# npm 12+ requires explicit approval for root Git dependencies:
npm install --allow-git=root "git+${SDK_GIT_URL}#<full-commit>"
# npm 11 (does not have the Git opt-in policy):
npm install "git+${SDK_GIT_URL}#<full-commit>"
# pnpm 12: first approve the exact Git source in pnpm-workspace.yaml (below).
pnpm add "git+${SDK_GIT_URL}#<full-commit>"
# Bun Git installs are best effort; see the observed limitation below.
bun add --trust "git+${SDK_GIT_URL}#<full-commit>"
```

For pnpm 12, add an exact-source entry to the consuming project's
`pnpm-workspace.yaml`, replacing the URL and commit with literal values:

```yaml
allowBuilds:
  "radicle-sdk@git+https://YOUR-HOST/YOUR-REPOSITORY.git#FULL_COMMIT_SHA": true
```

`--allow-build radicle-sdk` alone did not approve Git `prepare` in the tested
pnpm release. The smoke script writes the exact-source approval automatically.
Only trust build scripts from reviewed commits. Rust/C/Git must be available to
the installing process; installing a JS dependency compiles native code.
A first install compiles Radicle and its dependencies in release mode and can take
minutes. The smoke checks reuse a warm Cargo dependency cache; their compile times
are not first-install estimates.
The package has `private: true` to prevent accidental registry publication;
this does not prevent Git installs.

Observed Bun 1.4.3 limitation: Git `prepare` runs without installing this
package's development build tools, so the command above fails with `napi: command
not found`. npm/pnpm are the validated source-install paths. Bun can load and run
the Node-built addon; that is not a claim that Bun's Git build path works.

Git dependency `prepare` uses the committed Cargo lockfile to compile the native
addon in optimized **release** mode, generate native declarations and compile the
TypeScript wrapper. It clears generated `node/dist` output first, so legacy
addons cannot leak into a package. Unwinding panic containment is retained;
abort-mode builds are rejected. Optimization is not a latency or portable-binary
guarantee. Installed consumers load the built addon; the package has no native
install/rebuild lifecycle script.

## Runnable local quick start

After installing the package above, save this as `quickstart.mjs` in your consumer
project and run `node quickstart.mjs`. It creates a temporary profile and Git
repository, exercises project/issue/patch workflows without a node or network,
and removes all demo data on exit. The tests execute this exact fenced block,
including against the README shipped by exact-commit Git installs.

```js
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { Profile, initProject, forkProject, checkoutProject, Issues, Patches } from 'radicle-sdk';

function git(work, ...args) {
  return execFileSync('git', ['-C', work, '-c', 'user.name=Alice',
    '-c', 'user.email=alice@example.invalid', '-c', 'commit.gpgsign=false', ...args], {
    env: { ...process.env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' },
    encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'],
  }).trim();
}
const root = await mkdtemp(join(tmpdir(), 'radicle-sdk-quickstart-'));
try {
  const home = join(root, 'home'), work = join(root, 'work');
  await mkdir(work);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'README.md'), '# Hello Radicle\n');
  git(work, 'add', '.'); git(work, 'commit', '-m', 'Initial commit');
  const profile = await Profile.init(home, 'alice');
  const signer = profile.signer(), storage = profile.storage;
  const rid = await initProject(storage, signer, work, {
    name: 'hello', description: 'Local SDK demo', defaultBranch: 'main', visibility: 'private',
  });
  const repo = storage.repository(rid);
  assert.equal((await repo.identity()).visibility, 'private');
  const base = (await repo.head()).oid;
  await forkProject(storage, signer, rid);
  const checkout = join(root, 'checkout');
  await checkoutProject(storage, rid, signer.nodeId, checkout);
  assert.equal(git(checkout, 'rev-parse', 'HEAD'), base);

  const issues = Issues.openAs(repo, signer);
  const issue = await issues.create({ title: 'First issue', description: 'Root', labels: ['demo'] });
  const comment = await issue.comment('A reply');
  assert.equal(comment.snapshot.comments.length, 2);
  await issue.edit('Clarified title');
  const closed = await issue.close({ reason: 'solved' });
  assert.equal(closed.snapshot.closeReason, 'solved');
  assert.equal(comment.snapshot.state, 'open'); // prior snapshots stay detached
  await issue.reopen();
  assert.equal((await Issues.open(repo).all()).length, 1);

  // Git imports commits; the SDK takes explicit existing storage commit OIDs.
  const stored = join(home, 'storage', rid.slice(4));
  async function importCommit(text) {
    await writeFile(join(work, 'README.md'), text);
    git(work, 'commit', '-am', text.trim());
    git(work, 'push', stored, `HEAD:refs/namespaces/${signer.nodeId}/refs/heads/main`);
    await repo.signRefs(signer); // direct Git writes do not sign Radicle refs
    return git(work, 'rev-parse', 'HEAD');
  }
  const head = await importCommit('First change\n');
  const patches = Patches.openAs(repo, signer);
  const patch = await patches.draft({ title: 'Demo patch', description: 'First', base, head });
  assert.equal((await patch.ready()).snapshot.state, 'open');
  const nextHead = await importCommit('Second change\n');
  const revision = await patch.update({ description: 'Second', base, head: nextHead });
  await patch.comment(revision.revisionId, 'Review this revision');
  const reviewed = await patch.review(revision.revisionId, { verdict: 'accept', summary: 'Looks good' });
  assert.equal(reviewed.snapshot.head, nextHead);
  assert.equal((await Patches.open(repo).all()).length, 1);
  console.log('README quick start OK', rid, issue.id, patch.id);
} finally {
  await rm(root, { recursive: true, force: true });
}
```

## Contract and development

The [API reference](API.md) describes handles, repository/issue/patch operations,
snapshots, timestamps, errors, concurrency, environment setup and panic limits.

In a source checkout, `docs/semantics.md` and `docs/sdk-differences.md` explain
cross-SDK behavior. Repository build/test commands and exact-commit npm/pnpm
validation live in `docs/development.md`. These are repository documents, not
additional package exports. The installed guide and API reference remain under
`node/` in the package.
