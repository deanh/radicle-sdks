import { type Repository, type Storage, type Signer, type Tree, type TreeEntry,
  type HistoryPage, type HistoryOptions, type SignedRefs } from '../dist/index.js';
async function usage(storage: Storage, repo: Repository, signer: Signer): Promise<void> {
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
  console.log(present, branches, branch, parentIds, kind, name, blob, feature, parent, time, immediate);
}
void usage;
