// Warm, read-only reconciliation cost probe through installed public exports.
import { readFile } from 'node:fs/promises';
import { parseArgs } from 'node:util';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
import assert from 'node:assert/strict';
import { Profile } from 'radicle-sdk';
import { since, emitCursor } from './reconcile-refs.mjs';

export async function measure(repository, { rounds = 5, cursor } = {}) {
  if (!Number.isSafeInteger(rounds) || rounds < 1) throw new Error('rounds must be positive');
  const tips = new Map((await repository.namespaceTips()).map((t) => [t.nodeId, t.target]));
  cursor ??= emitCursor(repository.rid, tips);
  const result = {
    rid: repository.rid,
    namespaces: tips.size,
    cursorBytes: Buffer.byteLength(cursor),
    rounds,
    models: {},
  };
  const pages = [];
  for (const [name, fullScan] of [
    ['tips', false],
    ['signed-set-scan', true],
  ]) {
    await since(repository, cursor, { fullScan });
    const elapsed = [];
    let page, stats;
    for (let i = 0; i < rounds; i++) {
      const start = performance.now();
      ({ page, stats } = await since(repository, cursor, { fullScan }));
      elapsed.push(performance.now() - start);
    }
    elapsed.sort((a, b) => a - b);
    const middle = Math.floor(rounds / 2);
    const medianMs = rounds % 2 ? elapsed[middle] : (elapsed[middle - 1] + elapsed[middle]) / 2;
    result.models[name] = {
      medianMs,
      stats,
      namespaceRecords: page.namespaces.length,
      cobRecords: page.cobs.length,
      incomplete: page.incomplete,
    };
    pages.push(page);
  }
  assert.deepEqual(pages[0], pages[1]);
  return result;
}
async function main() {
  const { values, positionals } = parseArgs({
    options: { rounds: { type: 'string', default: '5' }, since: { type: 'string' } },
    allowPositionals: true,
  });
  if (positionals.length !== 1)
    throw new Error('usage: bench-reconciliation.mjs [--rounds N] [--since CURSOR] RID');
  const cursor = values.since
    ? new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(
        await readFile(values.since),
      )
    : undefined;
  console.log(
    JSON.stringify(
      await measure(Profile.load().storage.repository(positionals[0]), {
        rounds: Number(values.rounds),
        cursor,
      }),
    ),
  );
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
