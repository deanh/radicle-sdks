import {
  type Repository,
  type Storage,
  type Signer,
  type Tree,
  type TreeEntry,
  type HistoryPage,
  type HistoryOptions,
  type SignedRefs,
  type TagInfo,
  type RemoteInfo,
  type NamespaceTip,
  type SetHead,
  checkoutProject,
  parseOid,
} from '../dist/index.js';
async function usage(storage: Storage, repo: Repository, signer: Signer): Promise<void> {
  const id: string = parseOid('oid');
  const description: string = await repo.description();
  const tags: ReadonlyArray<TagInfo> = await repo.tags();
  const remotes: ReadonlyArray<RemoteInfo> = await repo.remotes();
  const tips: ReadonlyArray<NamespaceTip> = await repo.namespaceTips();
  const nodeId: string = tips[0].nodeId;
  const target: string = tips[0].target;
  // @ts-expect-error namespace tips are immutable
  tips[0].target = id;
  console.log(nodeId, target);
  const changed: SetHead = await repo.setDefaultBranchToCanonicalHead();
  const updated: boolean = changed.isUpdated;
  const previous: string | undefined = changed.old;
  const identity = await repo.identity();
  const version: number = identity.version;
  const threshold: number = identity.threshold;
  const identityCommit: string = identity.commit;
  const identityBlob: string = identity.blob;
  await repo.revParse('HEAD');
  await repo.setIdentityHead();
  await repo.setRemoteIdentityRoot(signer.nodeId);
  await checkoutProject(storage, repo.rid, signer.nodeId, '/tmp/checkout', { bare: true });
  // @ts-expect-error bare does not coerce strings
  await checkoutProject(storage, repo.rid, signer.nodeId, '/tmp/checkout', { bare: 'true' });
  // @ts-expect-error tag snapshots are readonly
  tags[0].peeled = id;
  // @ts-expect-error metadata snapshots are readonly
  identity.threshold = 2;
  console.log(
    description,
    remotes,
    updated,
    previous,
    version,
    threshold,
    identityCommit,
    identityBlob,
  );
  const ids: ReadonlyArray<string> = await storage.repositories();
  const present: boolean = await storage.contains(repo.rid);
  const branches: ReadonlyArray<string> = await repo.branches();
  const branch: string | null = await repo.branchHead('main');
  const c = await repo.commit('oid');
  const parentIds: ReadonlyArray<string> = c.parents;
  const bytes: Uint8Array = c.authorEmail;
  bytes[0] = 0; // independent mutable copies, not immutable byte containers
  const tree: Tree = await repo.tree(c.tree);
  const e: TreeEntry = tree.entries[0];
  const kind: 'blob' | 'tree' | 'commit' | undefined = e.kind;
  const name: string = e.nameStr;
  e.name[0] = 0;
  const blob: Uint8Array = await repo.blob(e.oid);
  const options: HistoryOptions = { limit: 2, offset: 1 };
  const history: HistoryPage = await repo.history('HEAD', options);
  const loaded: SignedRefs = await repo.signedRefsAt(signer.nodeId, id);
  const at: string = loaded.at;
  const refs: Readonly<Record<string, string>> = loaded.refs;
  // @ts-expect-error signed sets are readonly
  loaded.refs['refs/heads/main'] = id;
  // @ts-expect-error explicit OID must be a string
  await repo.signedRefsAt(signer.nodeId, 123);
  console.log(at, refs);
  const signed: SignedRefs = await repo.signRefs(signer);
  const feature: 'none' | 'root' | 'parent' = signed.featureLevel;
  const parent: string | undefined = signed.parent;
  const time: number | null = c.committerTime;
  // @ts-expect-error I/O is asynchronous
  const immediate: boolean = storage.contains(repo.rid);
  // @ts-expect-error readonly collections
  ids.push(repo.rid);
  // @ts-expect-error snapshots are readonly
  tree.entries.push(e);
  // @ts-expect-error snapshot fields are readonly
  e.nameStr = 'other';
  // @ts-expect-error history contents are readonly
  history.oids.push('oid');
  // @ts-expect-error signed ref map is readonly
  signed.refs['refs/heads/main'] = 'oid';
  // @ts-expect-error options replace positional pagination arguments
  await repo.history('HEAD', 10);
  console.log(
    present,
    branches,
    branch,
    parentIds,
    kind,
    name,
    blob,
    feature,
    parent,
    time,
    immediate,
  );
}
void usage;
