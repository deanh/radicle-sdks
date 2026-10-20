// Application reconciliation experiment, not an SDK feed API. See reconciliation.md.
import { open, readFile, rename, unlink } from 'node:fs/promises';
import { dirname, basename, join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { parseArgs } from 'node:util';
import { pathToFileURL } from 'node:url';
import { Profile, parseRepoId, parseNodeId, parseObjectId } from 'radicle-sdk';
import { checkpointPath } from './watch-issues.mjs';

const HEADER = '# radicle-events v1 ';
const refPattern = /^refs\/namespaces\/([^/]+)\/refs\/rad\/sigrefs$/;
const compare = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

export function parseCursor(text, rid) {
  parseRepoId(rid);
  const lines = text.split('\n');
  if (lines[0] !== HEADER + rid)
    throw new Error('cursor line 1: wrong version, repository or header');
  if (lines.at(-1) !== '') throw new Error(`cursor line ${lines.length}: missing final LF`);
  const tips = new Map();
  let previous = '';
  for (let i = 1; i < lines.length - 1; i++) {
    const parts = lines[i].split(' ');
    if (parts.length !== 2) throw new Error(`cursor line ${i + 1}: expected oid and refname`);
    const [oid, name] = parts;
    const match = refPattern.exec(name);
    if (
      oid.length !== 40 ||
      !/^[0-9a-f]{40}$/.test(oid) ||
      oid === '0'.repeat(40) ||
      !match ||
      match[0] !== name
    )
      throw new Error(`cursor line ${i + 1}: invalid oid or refname`);
    const nid = match[1];
    try {
      if (parseNodeId(nid) !== nid) throw new Error('noncanonical node id');
    } catch {
      throw new Error(`cursor line ${i + 1}: invalid node id`);
    }
    if (name <= previous) throw new Error(`cursor line ${i + 1}: unsorted or duplicate refname`);
    previous = name;
    tips.set(nid, oid);
  }
  return tips;
}

export function emitCursor(rid, tips) {
  const text =
    HEADER +
    rid +
    '\n' +
    [...tips]
      .sort(([a], [b]) => compare(a, b))
      .map(([nid, oid]) => `${oid} refs/namespaces/${nid}/refs/rad/sigrefs\n`)
      .join('');
  parseCursor(text, rid);
  return text;
}

class Reader {
  constructor(repository) {
    this.repo = repository;
    this.stats = { namespace_tips: 0, signed_refs_at: 0, commit: 0, ref_names: 0 };
    this.sets = new Map();
    this.parents = new Map();
  }
  async tips() {
    this.stats.namespace_tips++;
    return new Map((await this.repo.namespaceTips()).map((tip) => [tip.nodeId, tip.target]));
  }
  async signed(nid, oid) {
    const key = `${nid}/${oid}`;
    if (!this.sets.has(key)) {
      this.stats.signed_refs_at++;
      const refs = (await this.repo.signedRefsAt(nid, oid)).refs;
      this.sets.set(key, refs);
      this.stats.ref_names += Object.keys(refs).length;
    }
    return this.sets.get(key);
  }
  async ancestor(old, current) {
    const pending = [current],
      seen = new Set();
    while (pending.length) {
      const oid = pending.pop();
      if (oid === old) return true;
      if (seen.has(oid)) continue;
      seen.add(oid);
      if (!this.parents.has(oid)) {
        this.stats.commit++;
        this.parents.set(oid, (await this.repo.commit(oid)).parents);
      }
      pending.push(...this.parents.get(oid));
    }
    return false;
  }
}

function frontier(sets) {
  const result = new Map();
  for (const refs of sets.values()) {
    for (const [name, target] of Object.entries(refs)) {
      const parts = name.split('/');
      if (parts.length !== 4 || parts[0] !== 'refs' || parts[1] !== 'cobs') continue;
      try {
        parseObjectId(parts[3]);
      } catch (cause) {
        const error = new Error(`invalid signed COB ref: ${name}`, { cause });
        error.code = 'GIT';
        throw error;
      }
      const key = `${parts[2]}/${parts[3]}`;
      if (!result.has(key)) result.set(key, new Set());
      result.get(key).add(target);
    }
  }
  return result;
}

export async function since(repository, cursor, { fullScan = false } = {}) {
  const rid = repository.rid;
  const old = parseCursor(cursor, rid),
    reader = new Reader(repository);
  const current = await reader.tips();
  const page = {
    rid,
    cursor,
    next: emitCursor(rid, current),
    namespaces: [],
    incomplete: [],
    cobs: [],
  };
  const unchanged =
    old.size === current.size && [...old].every(([nid, oid]) => current.get(nid) === oid);
  if (unchanged && !fullScan) return { page, stats: reader.stats };
  const beforeSets = new Map(),
    afterSets = new Map();
  for (const [nid, oid] of [...current].sort(([a], [b]) => compare(a, b)))
    afterSets.set(nid, await reader.signed(nid, oid));
  for (const [nid, oid] of [...old].sort(([a], [b]) => compare(a, b))) {
    try {
      beforeSets.set(nid, await reader.signed(nid, oid));
    } catch (error) {
      if (error.code !== 'GIT') throw error;
      page.incomplete.push(nid);
      page.namespaces.push({
        node_id: nid,
        kind: 'history-unavailable',
        old: oid,
        new: current.get(nid) ?? null,
        error: error.message,
      });
    }
  }
  for (const nid of [...new Set([...old.keys(), ...current.keys()])].sort()) {
    if (old.get(nid) === current.get(nid) || page.incomplete.includes(nid)) continue;
    const kind = !old.has(nid)
      ? 'created'
      : !current.has(nid)
        ? 'removed'
        : (await reader.ancestor(old.get(nid), current.get(nid)))
          ? 'updated'
          : 'forced';
    page.namespaces.push({
      node_id: nid,
      kind,
      old: old.get(nid) ?? null,
      new: current.get(nid) ?? null,
    });
  }
  page.namespaces.sort((a, b) => compare(a.node_id, b.node_id));
  const before = frontier(beforeSets),
    after = frontier(afterSets);
  for (const key of [...new Set([...before.keys(), ...after.keys()])].sort((a, b) => {
    const [aType, aId] = a.split('/');
    const [bType, bId] = b.split('/');
    return (
      Buffer.compare(Buffer.from(aType), Buffer.from(bType)) ||
      Buffer.compare(Buffer.from(aId), Buffer.from(bId))
    );
  })) {
    const previous = [...(before.get(key) ?? [])].sort(),
      now = [...(after.get(key) ?? [])].sort();
    if (previous.length === now.length && previous.every((oid, i) => oid === now[i])) continue;
    const [typename, id] = key.split('/');
    page.cobs.push({
      typename,
      id,
      kind: !previous.length ? 'created' : !now.length ? 'deleted' : 'changed',
      old_tips: previous,
      new_tips: now,
      old_complete: !page.incomplete.length,
    });
  }
  return { page, stats: reader.stats };
}

export async function saveCursor(path, text) {
  const temporary = join(dirname(path), `.${basename(path)}.${randomUUID()}`);
  let file;
  let created = false;
  try {
    file = await open(temporary, 'wx', 0o600);
    created = true;
    await file.writeFile(text, 'utf8');
    await file.sync();
    await file.close();
    file = undefined;
    await rename(temporary, path);
  } finally {
    if (file) await file.close(); // FileHandle.close is idempotent after a failed close.
    if (created)
      await unlink(temporary).catch((error) => {
        if (error.code !== 'ENOENT') throw error;
      });
  }
}

export async function pollOnce(profile, rid, filename, handle, { replay = false } = {}) {
  if (typeof handle !== 'function') throw new TypeError('handle must be callable');
  rid = parseRepoId(rid);
  const path = await checkpointPath(profile, filename);
  const repo = profile.storage.repository(rid);
  let cursor;
  try {
    cursor = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(
      await readFile(path),
    );
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    if (!replay) {
      await saveCursor(
        path,
        emitCursor(rid, new Map((await repo.namespaceTips()).map((t) => [t.nodeId, t.target]))),
      );
      return null;
    }
    cursor = emitCursor(rid, new Map());
  }
  const { page } = await since(repo, cursor);
  if (page.namespaces.length || page.cobs.length) await handle(page);
  await saveCursor(path, page.next);
  return page;
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      state: { type: 'string' },
      replay: { type: 'boolean', default: false },
    },
    allowPositionals: true,
  });
  if (positionals.length !== 1 || !values.state)
    throw new Error('usage: reconcile-refs.mjs --state FILE [--replay] RID');
  await pollOnce(
    Profile.load(),
    positionals[0],
    values.state,
    (page) =>
      new Promise((resolve, reject) => {
        const line = JSON.stringify(page).replace(
          /[\u007f-\uffff]/g,
          (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`,
        );
        process.stdout.write(line + '\n', (error) => (error ? reject(error) : resolve()));
      }),
    { replay: values.replay },
  );
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
