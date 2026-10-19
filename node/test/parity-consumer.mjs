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
  PolicyStore,
  RoutingStore,
  Raw,
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
  const policy = await p.policyStore(),
    reader = await PolicyStore.reader(p);
  assert.equal(await policy.follow(p.nodeId, { alias: 'friend' }), true);
  assert.equal(await reader.isFollowing(p.nodeId), true);
  assert.deepEqual(await reader.nodesByAlias('RIE'), [[p.nodeId, 'friend']]);
  assert.equal((await reader.followPolicies())[0].policy, 'allow');
  assert.equal(await policy.seed(rid, { scope: 'followed' }), true);
  assert.equal((await reader.seedPolicies())[0].scope, 'followed');
  assert.equal(await policy.setFollowPolicy(p.nodeId, 'block'), true);
  assert.equal(await reader.isBlocked(p.nodeId), true);
  assert.equal((await reader.followPolicy(p.nodeId)).policy, 'block');
  assert.equal(await policy.unblockNid(p.nodeId), true);
  assert.equal(await policy.setSeedPolicy(rid, 'block'), true);
  assert.equal((await reader.seedPolicy(rid)).scope, undefined);
  assert.equal(await policy.unblockRid(rid), true);
  assert.equal(await policy.isSeeding(rid), false);
  assert.equal(await policy.unfollow(p.nodeId), false);
  assert.equal(await policy.unseed(rid), false);
  await assert.rejects(reader.follow(p.nodeId), (e) => e.code === 'READ_ONLY');
  const routes = await RoutingStore.fromProfile(p);
  assert.deepEqual(await routes.entries(), []);
  assert.deepEqual(await routes.get(rid), []);
  assert.deepEqual(await routes.getInventory(p.nodeId), []);
  assert.equal(await routes.entry(rid, p.nodeId), null);
  assert.equal(await routes.count(rid), 0);
  assert.equal(await routes.total(), 0);
  assert.equal(await routes.isEmpty(), true);
  assert.equal(await (await PolicyStore.open(policy.path)).isFollowing(p.nodeId), false);
  const raw = Raw.open(repo),
    ops = await raw.history('xyz.radicle.issue', issue.id);
  assert.equal(raw.rid, rid);
  assert.ok((await raw.typenames()).includes('xyz.radicle.issue'));
  assert.ok((await raw.entities('xyz.radicle.issue')).includes(issue.id));
  assert.ok((await raw.refs('xyz.radicle.issue', issue.id)).length > 0);
  assert.equal(typeof ops[0].timestamp, 'bigint');
  assert.equal(ops[0].manifest.version, 1n);
  assert.ok(ops[0].actionsJson().length > 0);
  ops[0].actions[0].fill(32);
  ops[0].actions[0].set(new TextEncoder().encode('9007199254740993'));
  assert.equal(ops[0].actionsJson()[0], 9007199254740993n);
  assert.equal(
    typeof (await raw.history('xyz.radicle.issue', issue.id))[0].actionsJson()[0],
    'object',
  );
  console.log('Parity public consumer OK');
} finally {
  await rm(root, { recursive: true, force: true });
}
