// Copy into a consumer project to execute its installed README, not the checkout's.
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const readme = await readFile(new URL('../README.md', import.meta.resolve('radicle-sdk')), 'utf8');
const heading = '## Runnable local quick start\n';
assert.equal(readme.split(heading).length, 2, 'one runnable quick-start section');
const section = readme.split(heading)[1].split('\n## ')[0];
const blocks = [...section.matchAll(/```js\n([\s\S]*?)\n```/g)];
assert.equal(blocks.length, 1, 'one actual JavaScript quick-start fence');
// Keep the generated module under the consumer, so bare imports resolve there.
const dir = await mkdtemp(join(dirname(fileURLToPath(import.meta.url)), '.readme-'));
try {
  const example = join(dir, 'quickstart.mjs');
  await writeFile(example, blocks[0][1]);
  const stdout = execFileSync(process.execPath, [example], {
    cwd: dir, encoding: 'utf8', timeout: 60000, stdio: ['ignore', 'pipe', 'pipe'],
  });
  assert.match(stdout, /README quick start OK rad:/);
  process.stdout.write(stdout);
} finally {
  await rm(dir, { recursive: true, force: true });
}
