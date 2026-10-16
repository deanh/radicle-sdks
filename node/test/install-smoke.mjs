// Copied into a fresh consumer project by scripts/git-install-smoke.sh.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { Profile, Issues, initProject, forkProject, checkoutProject, RadicleError } from 'radicle-sdk';

const root = await mkdtemp(join(tmpdir(), 'radicle-sdk-installed-'));
function git(work, ...args) {
  return execFileSync('git', ['-C', work, '-c', 'user.name=Alice', '-c', 'user.email=alice@example.invalid',
    '-c', 'commit.gpgsign=false', ...args], {
    env: { ...process.env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' }, encoding: 'utf8',
  }).trim();
}
try {
  const work = join(root, 'work'); await mkdir(work);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'README.md'), '# Installed SDK\n');
  git(work, 'add', '.'); git(work, 'commit', '-m', 'initial');
  const profile = await Profile.init(join(root, 'home'), 'alice');
  const signer = profile.signer();
  const rid = await initProject(profile.storage, signer, work, { name: 'installed', description: 'Source install', defaultBranch: 'main' });
  const repo = profile.storage.repository(rid);
  const head = await repo.head();
  assert.equal((await repo.identity()).visibility, 'public');
  assert.ok((await repo.commit(head.oid)).message instanceof Uint8Array);
  const issue = await Issues.openAs(repo, signer).create({ title: 'Installed', description: 'Root' });
  const result = await issue.comment('reply');
  assert.equal(result.snapshot.comments.length, 2);
  assert.ok(Object.isFrozen(result.snapshot.comments));
  assert.ok(result.snapshot.comments.some(c => c.id === result.entryId));
  await assert.rejects(Issues.open(repo).create({ title: 'no', description: '' }), e => e instanceof RadicleError && e.code === 'READ_ONLY');
  await forkProject(profile.storage, signer, rid);
  const checkout = join(root, 'checkout');
  await checkoutProject(profile.storage, rid, signer.nodeId, checkout);
  assert.equal(git(checkout, 'rev-parse', 'HEAD'), head.oid);
  console.log(`Installed SDK workflow OK (${process.version}${globalThis.Bun ? ', Bun ' + Bun.version : ''})`);
} finally { await rm(root, { recursive: true, force: true }); }
