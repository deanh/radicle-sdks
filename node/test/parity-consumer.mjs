// Also executed outside the checkout against the packed public package.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import {
  Profile,
  Home,
  Storage,
  Issues,
  Patches,
  initProject,
  checkoutProject,
  parseRepoId,
  parseDid,
  parseNodeId,
  parseOid,
  parseObjectId,
} from 'radicle-sdk';

const root = await mkdtemp(join(tmpdir(), 'sdk-parity-consumer-'));
const git = (path, ...args) =>
  execFileSync(
    'git',
    [
      '-C',
      path,
      '-c',
      'user.name=SDK',
      '-c',
      'user.email=sdk@example.invalid',
      '-c',
      'commit.gpgsign=false',
      '-c',
      'tag.gpgsign=false',
      ...args,
    ],
    { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
  ).trim();
try {
  const p = await Profile.init(join(root, 'home'), 'consumer');
  const signer = p.signer(),
    storage = p.storage,
    work = join(root, 'work');
  await mkdir(work);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'readme'), 'fixture\n');
  git(work, 'add', '.');
  git(work, 'commit', '-m', 'initial');
  const rid = await initProject(storage, signer, work, {
    name: 'consumer',
    description: 'parity smoke',
    defaultBranch: 'main',
  });
  assert.equal((await Home.load(p.home.path)).path, p.home.path);
  assert.equal((await Home.create(join(root, 'new-home'))).path, join(root, 'new-home'));
  const reopened = await Storage.open(storage.path, 'consumer', p.nodeId);
  assert.equal(await reopened.contains(rid), true);
  assert.equal(signer.did, p.did);
  assert.equal(await p.aliases.get(p.nodeId), 'consumer');
  assert.deepEqual(await p.aliases.reverseLookup('consumer'), { consumer: [p.nodeId] });
  const repo = storage.repository(rid),
    doc = await repo.identity();
  assert.equal(doc.threshold, 1);
  assert.ok(Number.isInteger(doc.version));
  assert.match(doc.blob, /^[0-9a-f]{40}$/);
  assert.equal(await repo.description(), 'parity smoke');
  assert.equal(await repo.setIdentityHead(), doc.commit);
  assert.equal(await repo.setRemoteIdentityRoot(p.nodeId), doc.commit);
  const head = (await repo.head()).oid;
  assert.equal(await repo.revParse('HEAD'), head);
  assert.equal((await repo.setDefaultBranchToCanonicalHead()).isUpdated, false);
  assert.deepEqual(await repo.tags(), []);
  assert.ok((await repo.remotes()).some((r) => r.fetcher === p.nodeId));
  assert.equal(parseRepoId(rid), rid);
  assert.equal(parseDid(p.did), p.did);
  assert.equal(parseNodeId(p.nodeId), p.nodeId);
  assert.equal(parseOid(head.toUpperCase()), head);
  assert.equal(parseObjectId(head.toUpperCase()), head);
  const bare = join(root, 'bare');
  await checkoutProject(storage, rid, p.nodeId, bare, { bare: true });
  assert.equal(git(bare, 'rev-parse', '--is-bare-repository'), 'true');
  const issues = Issues.openAs(repo, signer),
    patches = Patches.openAs(repo, signer);
  assert.equal(issues.rid, rid);
  assert.equal(patches.rid, rid);
  assert.equal(await issues.isEmpty(), true);
  assert.equal(await patches.count(), 0);
  const issue = await issues.create({ title: 'test', description: 'root' });
  const issueValue = await issue.reload();
  assert.equal(issueValue.commentCount, 1);
  assert.deepEqual(
    issueValue.commentIds,
    issueValue.comments.map((c) => c.id),
  );
  assert.equal(await issues.count(), 1);
  const patch = await patches.create({ title: 'patch', description: 'd', base: head, head });
  assert.equal((await patch.reload()).revisionCount, 1);
  assert.equal(await patches.isEmpty(), false);
  console.log('Parity public consumer OK');
} finally {
  await rm(root, { recursive: true, force: true });
}
