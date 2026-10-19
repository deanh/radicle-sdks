// Operation-feed experiment; see events_experiment.md. Not an SDK API.
import {
  Raw,
  Profile,
  parseRepoId,
  parseOid,
  parseObjectId,
  parseNodeId,
  parseDid,
  RadicleError,
} from 'radicle-sdk';
import { checkpointPath, loadState, saveState } from './watch-issues.mjs';
import { pathToFileURL } from 'node:url';

const TYPES = ['xyz.radicle.issue', 'xyz.radicle.patch'];
const domain = (type) => (type === 'xyz.radicle.issue' ? 'issue' : 'patch');
export class ResyncRequired extends Error {}

// JSON wire preview only; exact bytes stay in rawActions.
const preview = (value) => {
  if (typeof value === 'bigint') return value.toString();
  if (Array.isArray(value)) return value.map(preview);
  if (value !== null && typeof value === 'object')
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, preview(v)]));
  return value;
};

function eventFor(rid, typename, entity, op) {
  let payload = null,
    actionTypes = [],
    decodeError = false;
  try {
    payload = preview(op.actionsJson());
    actionTypes = payload.map((a) =>
      a !== null && typeof a === 'object' && typeof a.type === 'string' ? a.type : null,
    );
  } catch (error) {
    if (!(error instanceof RadicleError)) throw error;
    decodeError = true;
  }
  const kinds = actionTypes.map((t) =>
    t ? `${domain(typename)}.${t}Recorded` : `${domain(typename)}.unknownAction`,
  );
  return {
    id: op.id,
    rid,
    cob: { type: typename, id: entity },
    kind:
      op.id === entity
        ? `${domain(typename)}.opened`
        : kinds.length === 1
          ? kinds[0]
          : `${domain(typename)}.operation`,
    actor: parseDid(`did:key:${op.actor}`),
    timestamp: op.timestamp.toString(),
    parents: [...op.parents],
    actionKinds: kinds,
    payload,
    decodeError,
    evaluation: 'not-evaluated',
    rawActions: op.actions.map((a) => Buffer.from(a).toString('base64')),
  };
}

class Reader {
  constructor(repo) {
    this.repo = repo;
    this.raw = Raw.open(repo);
    this.calls = {};
    this.historyEntries = this.sigrefsBytes = this.discoveryRefs = 0;
  }
  async call(name, ...args) {
    const [owner, method] = name.split('.');
    this.calls[name] = (this.calls[name] ?? 0) + 1;
    return (owner === 'raw' ? this.raw : this.repo)[method](...args);
  }
  async scan() {
    const result = {};
    for (const typename of TYPES)
      for (const entity of await this.call('raw.entities', typename)) {
        const tips = [
          ...new Set((await this.call('raw.refs', typename, entity)).map((r) => r.target)),
        ].sort();
        if (tips.length) result[`${typename}/${entity}`] = tips;
      }
    return result;
  }
  async signatures() {
    const remotes = await this.call('repo.remotes');
    this.discoveryRefs = remotes.reduce((n, r) => n + r.heads.length, 0);
    const out = {};
    for (const remote of remotes)
      out[remote.fetcher] = await this.call(
        'repo.revParse',
        `refs/namespaces/${remote.fetcher}/refs/rad/sigrefs`,
      );
    return out;
  }
  async signedFrontier(signatures) {
    const result = {};
    for (const oid of Object.values(signatures)) {
      if (!this.blobs) this.blobs = {};
      if (!(oid in this.blobs)) {
        const commit = await this.call('repo.commit', oid);
        const tree = await this.call('repo.tree', commit.tree);
        const entry = tree.entries.find((e) => e.nameStr === 'refs');
        if (!entry || entry.kind !== 'blob') throw new ResyncRequired('sigrefs refs blob missing');
        const data = await this.call('repo.blob', entry.oid);
        this.sigrefsBytes += data.byteLength;
        const refs = {};
        for (const line of new TextDecoder('utf-8', { fatal: true, ignoreBOM: true })
          .decode(data)
          .split('\n')) {
          if (!line) continue;
          const [target, name] = line.split(' ');
          const parts = name.split('/');
          if (
            parts.length === 4 &&
            parts[0] === 'refs' &&
            parts[1] === 'cobs' &&
            TYPES.includes(parts[2])
          ) {
            const entity = parseObjectId(parts[3]);
            if (entity === parts[3]) refs[`${parts[2]}/${entity}`] = parseOid(target);
          }
        }
        this.blobs[oid] = refs;
      }
      for (const [key, target] of Object.entries(this.blobs[oid])) {
        (result[key] ??= new Set()).add(target);
      }
    }
    return Object.fromEntries(Object.entries(result).map(([k, tips]) => [k, [...tips].sort()]));
  }
}

async function loadCursor(path, storage, rid, source) {
  let state;
  try {
    state = JSON.parse(
      new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(
        await (await import('node:fs/promises')).readFile(path),
      ),
    );
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
    return null;
  }
  const keys = Object.keys(state).sort().join(',');
  if (
    keys !== 'frontier,model,rid,source,storage,version' ||
    state.version !== 1 ||
    state.model !== 'operations' ||
    state.source !== source ||
    state.storage !== storage ||
    state.rid !== rid ||
    state.frontier === null ||
    typeof state.frontier !== 'object' ||
    Array.isArray(state.frontier)
  )
    throw new Error('invalid operation cursor version/model/source/scope/shape');
  for (const [key, value] of Object.entries(state.frontier)) {
    if (source === 'sigrefs') {
      if (parseNodeId(key) !== key || typeof value !== 'string' || parseOid(value) !== value)
        throw new Error('invalid sigrefs cursor');
    } else {
      const [type, id] = key.split('/');
      if (
        !TYPES.includes(type) ||
        parseObjectId(id) !== id ||
        !Array.isArray(value) ||
        value.length === 0 ||
        value.some((v) => typeof v !== 'string' || parseOid(v) !== v) ||
        new Set(value).size !== value.length
      )
        throw new Error('invalid ref frontier cursor');
    }
  }
  return state;
}

function reachable(tips, history) {
  const parents = new Map(history.map((op) => [op.id, op.parents]));
  const seen = new Set(),
    stack = [...tips];
  while (stack.length) {
    const id = stack.pop();
    if (seen.has(id)) continue;
    if (!parents.has(id))
      throw new ResyncRequired(
        'frontier tip/ancestor unavailable through current Raw.history; cursor retained',
      );
    seen.add(id);
    stack.push(...parents.get(id));
  }
  return seen;
}

/** One operation per event; cursor saved after the whole delivered batch. */
export async function pollOperations(
  profile,
  rid,
  filename,
  handle,
  { source = 'scan', replay = false } = {},
) {
  if (
    !['scan', 'sigrefs'].includes(source) ||
    typeof handle !== 'function' ||
    typeof source !== 'string'
  )
    throw new Error(
      `require scan/sigrefs source and callback function (got source=${JSON.stringify(source)})`,
    );
  const started = performance.now();
  rid = parseRepoId(rid);
  const path = await checkpointPath(profile, filename);
  const storage = await realpathStorage(profile);
  const previous = await loadCursor(path, storage, rid, source);
  const reader = new Reader(profile.storage.repository(rid));
  const frontier = source === 'scan' ? await reader.scan() : await reader.signatures();
  const cursor = { version: 1, model: 'operations', source, storage, rid, frontier };
  let events = [];
  if (previous !== null && JSON.stringify(previous.frontier) === JSON.stringify(frontier)) {
    // unchanged
  } else if (previous === null && !replay) {
    // silent baseline
  } else {
    let old = previous === null ? {} : previous.frontier;
    const fresh = frontier;
    if (source === 'sigrefs') {
      old = await reader.signedFrontier(old);
    }
    const newer = source === 'sigrefs' ? await reader.signedFrontier(fresh) : fresh;
    for (const key of [...new Set([...Object.keys(old), ...Object.keys(newer)])].sort()) {
      const a = old[key] ?? [],
        b = newer[key] ?? [];
      if (JSON.stringify(a) === JSON.stringify(b)) continue;
      if (!(key in newer))
        throw new ResyncRequired(
          'COB refs removed; cursor retained, choose an explicit recovery policy',
        );
      const [typename, entity] = key.split('/');
      const history = await reader.call('raw.history', typename, entity);
      reader.historyEntries += history.length;
      const ops = new Map(history.map((op) => [op.id, op]));
      const before = reachable(a, history);
      const after = reachable(b, history);
      for (const id of before)
        if (!after.has(id)) throw new ResyncRequired('non-fast-forward frontier; cursor retained');
      events.push(
        ...history
          .filter((op) => after.has(op.id) && !before.has(op.id))
          .map((op) => eventFor(rid, typename, entity, op)),
      );
    }
  }
  for (const event of events) await handle(event);
  const unchanged =
    previous !== null &&
    Object.keys(previous.frontier).length === Object.keys(cursor.frontier).length &&
    Object.entries(cursor.frontier).every(
      ([k, v]) => JSON.stringify(previous.frontier[k]) === JSON.stringify(v),
    );
  if (!unchanged) await saveState(path, cursor);
  return {
    events: events.length,
    calls: reader.calls,
    historyEntries: reader.historyEntries,
    sigrefsBytes: reader.sigrefsBytes,
    discoveryRefs: reader.discoveryRefs,
    cursorBytes: Buffer.byteLength(JSON.stringify(cursor)),
    seconds: (performance.now() - started) / 1000,
  };
}

async function realpathStorage(profile) {
  const { realpath } = await import('node:fs/promises');
  return realpath(profile.storage.path);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const argv = process.argv.slice(2);
    const flag = (name) => argv.includes(`--${name}`);
    const value = (name) => argv[argv.indexOf(`--${name}`) + 1];
    const rid = argv.find((a) => !a.startsWith('--'));
    const state = value('state');
    const source = flag('source') ? value('source') : 'scan';
    const replay = flag('replay');
    if (!rid || !state)
      throw new Error(
        'usage: operation-events.mjs --state FILE [--source scan|sigrefs] [--replay] RID',
      );
    const stats = await pollOperations(
      Profile.load(),
      rid,
      state,
      (event) => process.stdout.write(JSON.stringify(event) + '\n'),
      { source, replay },
    );
    process.stderr.write(JSON.stringify(stats) + '\n');
  } catch (error) {
    process.stderr.write(
      `operation experiment failed: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    process.exitCode = 1;
  }
}
