// Read-only issue/comment observer spike. See watch_issues.md, not an SDK API.
import { Profile, Issues, parseRepoId, parseObjectId, parseOid } from 'radicle-sdk';
import { lstat, realpath, readFile, open, rename, unlink } from 'node:fs/promises';
import { resolve, dirname, basename, relative, isAbsolute, join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';

async function stat(path) {
  try {
    return await lstat(path);
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
}
async function checkpointPath(profile, filename) {
  const absolute = resolve(filename);
  if ((await stat(absolute))?.isSymbolicLink()) throw new Error('checkpoint must not be a symlink');
  const path = join(await realpath(dirname(absolute)), basename(absolute));
  for (const root of [profile.home.path, profile.storage.path]) {
    const rel = relative(await realpath(root), path);
    if (rel === '' || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith('../')))
      throw new Error('checkpoint must be outside Radicle home and storage');
  }
  const info = await stat(path);
  if (info && !info.isFile()) throw new Error('checkpoint must be a regular file');
  return path;
}
function record(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
async function loadCheckpoint(path, storage, rid) {
  let text;
  try {
    text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(await readFile(path));
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
  const state = JSON.parse(text);
  if (
    !record(state) ||
    Object.keys(state).sort().join(',') !== 'rid,seen,storage,version' ||
    state.version !== 1 ||
    state.storage !== storage ||
    state.rid !== rid ||
    !record(state.seen)
  )
    throw new Error('invalid checkpoint version, scope or shape');
  for (const [id, comments] of Object.entries(state.seen)) {
    if (parseObjectId(id) !== id || !Array.isArray(comments))
      throw new Error('invalid checkpoint issue/comments');
    if (comments.some((cid) => typeof cid !== 'string' || parseOid(cid) !== cid))
      throw new Error('invalid checkpoint comment ID');
    if (new Set(comments).size !== comments.length)
      throw new Error('duplicate checkpoint comment ID');
  }
  return state;
}
async function saveCheckpoint(path, state) {
  // Same-directory atomic replace, file fsync, single writer. Not directory-fsync
  // durability, a concurrent-writer lock, or an acknowledgement transaction.
  const temporary = join(dirname(path), `.${basename(path)}.${randomUUID()}`);
  let file,
    created = false;
  try {
    file = await open(temporary, 'wx', 0o600);
    created = true;
    await file.writeFile(JSON.stringify(state) + '\n');
    await file.sync();
    await file.close();
    file = undefined;
    await rename(temporary, path);
  } finally {
    if (file) await file.close();
    if (created) {
      try {
        await unlink(temporary);
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
      }
    }
  }
}

/** Materialize one read batch, await every handler, then checkpoint. */
export async function pollOnce(profile, rid, filename, handle, { replay = false } = {}) {
  if (typeof handle !== 'function') throw new TypeError('handle must be a function');
  rid = parseRepoId(rid);
  const path = await checkpointPath(profile, filename),
    storage = await realpath(profile.storage.path);
  const previous = await loadCheckpoint(path, storage, rid);
  const seen = new Map(
    Object.entries(previous?.seen ?? {}).map(([id, comments]) => [id, new Set(comments)]),
  );
  // All SDK reads finish before application code; read failures deliver nothing.
  const issues = [...(await Issues.open(profile.storage.repository(rid)).all())].sort((a, b) =>
    a.id < b.id ? -1 : a.id > b.id ? 1 : 0,
  );
  let events = [];
  for (const issue of issues) {
    let known = seen.get(issue.id);
    if (!known) {
      known = new Set();
      events.push({
        id: `${rid}/issue/${issue.id}`,
        type: 'issue.discovered',
        rid,
        issueId: issue.id,
        title: issue.title,
        author: issue.author,
      });
    }
    const root = issue.comments[0]?.id;
    for (const comment of issue.comments) {
      if (!known.has(comment.id) && comment.id !== root)
        events.push({
          id: `${rid}/issue/${issue.id}/comment/${comment.id}`,
          type: 'comment.observed',
          rid,
          issueId: issue.id,
          title: issue.title,
          commentId: comment.id,
          author: comment.author,
          body: comment.body,
        });
      known.add(comment.id);
    }
    seen.set(issue.id, known);
  }
  const state = {
    version: 1,
    storage,
    rid,
    seen: Object.fromEntries(
      [...seen]
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([id, comments]) => [id, [...comments].sort()]),
    ),
  };
  if (previous === null && !replay) events = [];
  for (const event of events) await handle(event);
  // Compare parsed state, not the other language's JSON whitespace/key order.
  const unchanged =
    previous !== null &&
    Object.keys(previous.seen).length === seen.size &&
    [...seen].every(
      ([id, comments]) =>
        Object.hasOwn(previous.seen, id) &&
        comments.size === previous.seen[id].length &&
        previous.seen[id].every((cid) => comments.has(cid)),
    );
  if (!unchanged) await saveCheckpoint(path, state);
  return events.length;
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      state: { type: 'string' },
      once: { type: 'boolean' },
      replay: { type: 'boolean' },
      interval: { type: 'string', default: '2' },
      help: { type: 'boolean' },
    },
    allowPositionals: true,
  });
  if (values.help) {
    console.log('node watch-issues.mjs --state FILE [--once] [--replay] [--interval SECONDS] RID');
    return;
  }
  const interval = Number(values.interval);
  if (
    positionals.length !== 1 ||
    !values.state ||
    !Number.isFinite(interval) ||
    interval <= 0 ||
    interval > 86400
  )
    throw new Error('require one RID, --state FILE and finite interval > 0 and <= 86400 seconds');
  const profile = Profile.load(),
    stop = new AbortController();
  const shutdown = () => stop.abort();
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
  const outputError = () => {}; // write callback below propagates stream errors.
  process.stdout.on('error', outputError);
  try {
    do {
      await pollOnce(
        profile,
        positionals[0],
        values.state,
        (event) =>
          new Promise((done, fail) =>
            process.stdout.write(
              JSON.stringify(event).replace(
                /[\u007f-\uffff]/g,
                (char) => `\\u${char.charCodeAt(0).toString(16).padStart(4, '0')}`,
              ) + '\n',
              (error) => (error ? fail(error) : done()),
            ),
          ),
        { replay: values.replay },
      );
      if (values.once || stop.signal.aborted) break;
      try {
        await delay(interval * 1000, undefined, { signal: stop.signal });
      } catch (error) {
        if (error.name !== 'AbortError') throw error;
      }
    } while (!stop.signal.aborted);
  } finally {
    process.removeListener('SIGINT', shutdown);
    process.removeListener('SIGTERM', shutdown);
    process.stdout.removeListener('error', outputError);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    await main();
  } catch (error) {
    console.error(`observer failed: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
