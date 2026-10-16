import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { Profile, initProject } from '../dist/index.js';

export function git(work, ...args) {
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
      '-c',
      'tag.gpgsign=false',
      ...args,
    ],
    {
      env: { ...process.env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' },
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  ).trim();
}
export async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), 'radicle-sdk-test-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const home = join(root, 'home'),
    work = join(root, 'work');
  await mkdir(work);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'README.md'), '# Fixture\n');
  git(work, 'add', '.');
  git(work, 'commit', '-m', 'initial');
  const profile = await Profile.init(home, 'alice');
  const signer = profile.signer();
  const storage = profile.storage;
  const rid = await initProject(storage, signer, work, {
    name: 'fixture',
    description: 'T0 workflow',
    defaultBranch: 'main',
  });
  return { root, home, work, profile, signer, storage, rid, repo: storage.repository(rid) };
}
