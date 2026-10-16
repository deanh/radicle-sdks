// Copied into a fresh consumer project by scripts/git-install-smoke.sh.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import {
  Profile,
  Signer,
  Storage,
  Repository,
  Issues,
  IssueMut,
  Patches,
  PatchMut,
  initProject,
  forkProject,
  checkoutProject,
  RadicleError,
} from 'radicle-sdk';

const installed = JSON.parse(
  await readFile(new URL('../../package.json', import.meta.resolve('radicle-sdk')), 'utf8'),
);
assert.equal(installed.version, '0.1.0');
assert.equal(installed.napi.binaryName, 'radicle-sdk');
assert.match(installed.scripts['build:native'], /\bnapi build --release\b/);
for (const Handle of [Profile, Signer, Storage, Repository, Issues, IssueMut, Patches, PatchMut]) {
  assert.throws(() => Reflect.construct(Handle, []), TypeError);
  assert.throws(() => Reflect.construct(Handle, [{}]), TypeError);
  assert.equal(Handle.fromNative, undefined);
}
const root = await mkdtemp(join(tmpdir(), 'radicle-sdk-installed-'));
function git(work, ...args) {
  return execFileSync(
    'git',
    [
      '-C',
      work,
      '-c',
      'user.name=Alice',
      '-c',
      'user.email=alice@example.invalid',
      '-c',
      'commit.gpgsign=false',
      ...args,
    ],
    {
      env: { ...process.env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' },
      encoding: 'utf8',
    },
  ).trim();
}
try {
  const work = join(root, 'work');
  await mkdir(work);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'README.md'), '# Installed SDK\n');
  git(work, 'add', '.');
  git(work, 'commit', '-m', 'initial');
  const profile = await Profile.init(join(root, 'home'), 'alice');
  const signer = profile.signer();
  const rid = await initProject(profile.storage, signer, work, {
    name: 'installed',
    description: 'Source install',
    defaultBranch: 'main',
  });
  const repo = profile.storage.repository(rid);
  const head = await repo.head();
  assert.equal(await profile.storage.contains(rid), true);
  assert.ok((await profile.storage.repositories()).includes(rid));
  assert.deepEqual(await repo.branches(), ['main']);
  assert.equal(await repo.branchHead('main'), head.oid);
  assert.equal(await repo.branchHead('missing'), null);
  assert.equal((await repo.identity()).visibility, 'public');
  const initialCommit = await repo.commit(head.oid);
  assert.ok(initialCommit.message instanceof Uint8Array);
  assert.equal(initialCommit.authorNameStr, 'Alice');
  assert.ok(initialCommit.authorEmail instanceof Uint8Array);
  assert.deepEqual(initialCommit.parents, []);
  const tree = await repo.tree(initialCommit.tree);
  assert.ok(Object.isFrozen(tree.entries));
  const readme = tree.entries.find((e) => e.nameStr === 'README.md');
  assert.equal(Buffer.from(await repo.blob(readme.oid)).toString(), '# Installed SDK\n');
  assert.ok(Object.isFrozen(readme));
  assert.deepEqual((await repo.history(head.oid)).oids, [head.oid]);
  const issue = await Issues.openAs(repo, signer).create({
    title: 'Installed',
    description: 'Root',
    labels: ['alpha'],
    assignees: [profile.did],
  });
  const result = await issue.comment('reply');
  assert.equal(result.snapshot.comments.length, 2);
  assert.ok(Object.isFrozen(result.snapshot.comments));
  assert.ok(result.snapshot.comments.some((c) => c.id === result.entryId));
  assert.equal((await issue.edit('Edited')).snapshot.title, 'Edited');
  assert.equal((await issue.editDescription('Edited root')).snapshot.description, 'Edited root');
  assert.deepEqual((await issue.label(['installed'])).snapshot.labels, ['installed']);
  assert.deepEqual((await issue.assign([])).snapshot.assignees, []);
  assert.equal((await issue.close({ reason: 'solved' })).snapshot.closeReason, 'solved');
  const reopened = await issue.reopen();
  assert.equal(reopened.snapshot.state, 'open');
  assert.equal(reopened.snapshot.closeReason, undefined);
  assert.equal(reopened.snapshot.author, profile.did);
  const listed = await Issues.open(repo).all();
  assert.deepEqual(listed, [reopened.snapshot]);
  assert.ok(Object.isFrozen(listed));
  assert.deepEqual(result.snapshot.labels, ['alpha']); // prior snapshots stay detached
  await assert.rejects(
    Issues.open(repo).create({ title: 'no', description: '' }),
    (e) => e instanceof RadicleError && e.code === 'READ_ONLY',
  );
  await forkProject(profile.storage, signer, rid);
  const checkout = join(root, 'checkout');
  await checkoutProject(profile.storage, rid, signer.nodeId, checkout);
  assert.equal(git(checkout, 'rev-parse', 'HEAD'), head.oid);
  // Programmatic patches take explicit bounds and existing storage commits.
  const stored = join(root, 'home', 'storage', rid.slice(4));
  const addCommit = async (text) => {
    await writeFile(join(work, 'README.md'), text);
    git(work, 'commit', '-am', text.trim());
    const oid = git(work, 'rev-parse', 'HEAD');
    git(work, 'push', stored, `HEAD:refs/namespaces/${signer.nodeId}/refs/heads/main`);
    return oid;
  };
  const first = await addCommit('Installed patch\n');
  const signed = await repo.signRefs(signer);
  assert.equal(signed.nodeId, signer.nodeId);
  assert.equal(signed.refs['refs/heads/main'], first);
  assert.ok(Object.isFrozen(signed.refs));
  const history = await repo.history(first, { limit: 1 });
  assert.deepEqual(history.oids, [first]);
  assert.equal(history.hasMore, true);
  assert.deepEqual((await repo.history(first, { offset: 1 })).oids, [head.oid]);
  const patches = Patches.openAs(repo, signer);
  const patch = await patches.draft({
    title: 'Installed patch',
    description: 'First',
    base: head.oid,
    head: first,
    labels: ['alpha'],
  });
  const initial = await patch.reload();
  assert.equal(initial.state, 'draft');
  assert.equal(initial.target, 'delegates');
  assert.equal((await patch.ready()).changed, true);
  assert.equal((await patch.ready()).changed, false);
  assert.equal((await patch.unready()).snapshot.state, 'draft');
  await patch.ready();
  assert.equal(
    (await patch.edit({ title: 'Edited patch', target: 'branch:main' })).snapshot.target,
    'branch:main',
  );
  await patch.edit({ title: 'Edited patch', target: 'delegates' });
  assert.deepEqual((await patch.label(['installed'])).snapshot.labels, ['installed']);
  assert.deepEqual((await patch.assign([profile.did])).snapshot.assignees, [profile.did]);
  assert.equal((await patch.archive()).snapshot.state, 'archived');
  assert.equal((await patch.unarchive()).changed, true);
  assert.equal((await patch.unarchive()).changed, false);
  const second = await addCommit('Installed revision two\n');
  const update = await patch.update({ description: 'Second', base: head.oid, head: second });
  assert.equal(update.snapshot.latestRevision, update.revisionId);
  assert.equal(update.snapshot.head, second);
  const other = await patches.getMut(patch.id);
  const comment = await other.comment(update.revisionId, 'Installed comment');
  const reply = await patch.comment(update.revisionId, 'Installed reply', {
    replyTo: comment.entryId,
  });
  const revision = reply.snapshot.revisions.find((r) => r.id === update.revisionId);
  assert.equal(revision.comments[1].replyTo, comment.entryId);
  const review = await other.review(update.revisionId, { verdict: 'accept', summary: 'Approved' });
  assert.equal(
    review.snapshot.revisions.find((r) => r.id === update.revisionId).reviews[0].id,
    review.entryId,
  );
  const canonical = git(stored, 'rev-parse', 'refs/heads/main');
  const merge = await patch.merge(update.revisionId, second);
  assert.equal(merge.snapshot.state, 'merged');
  assert.equal(merge.snapshot.mergedRevision, update.revisionId);
  assert.equal(merge.snapshot.mergedCommit, second);
  assert.equal(git(stored, 'rev-parse', 'refs/heads/main'), canonical);
  assert.equal(git(work, 'rev-parse', 'HEAD'), second);
  assert.deepEqual(await Patches.open(repo).get(patch.id), merge.snapshot);
  assert.deepEqual(await patch.reload(), merge.snapshot);
  const empty = await patches.create({
    title: 'Empty allowed',
    description: '',
    base: head.oid,
    head: head.oid,
  });
  const patchList = await Patches.open(repo).all();
  assert.deepEqual(
    patchList.map((p) => p.id),
    [patch.id, empty.id].sort(),
  );
  assert.ok(Object.isFrozen(patchList));
  assert.ok(Object.isFrozen(review.snapshot.revisions[1].reviews[0]));
  assert.ok(Object.isFrozen(merge));
  assert.equal(initial.revisions.length, 1);
  assert.equal(initial.state, 'draft');
  assert.equal(await Patches.open(repo).get('0'.repeat(40)), null);
  await assert.rejects(
    Patches.open(repo).getMut(patch.id),
    (e) => e instanceof RadicleError && e.code === 'READ_ONLY',
  );
  console.log(
    `Installed SDK workflow OK (${process.version}${globalThis.Bun ? ', Bun ' + Bun.version : ''})`,
  );
} finally {
  await rm(root, { recursive: true, force: true });
}
