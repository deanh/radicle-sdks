import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

test('the actual README quick-start fence runs through package exports', { timeout: 70000 }, async () => {
  const { stdout } = await promisify(execFile)(process.execPath, [
    fileURLToPath(new URL('./readme-consumer.mjs', import.meta.url)),
  ], { timeout: 65000 });
  assert.match(stdout, /README quick start OK rad:/);
});
