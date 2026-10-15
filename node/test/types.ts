// Compile-only consumer checks against generated Rust DTOs and TS wrapper declarations.
import { Profile, Signer, Issues, initProject, forkProject, checkoutProject, type Issue, type Commit, type IssueChange, type CloseIssueOptions } from '../../node/dist/index.js';

const loadProfile: () => Profile = Profile.load;
void loadProfile;

async function usage(home: string): Promise<void> {
  const profile = await Profile.init(home, 'alice');
  const signer: Signer = profile.signer();
  const rid = await initProject(profile.storage, signer, '/work', { name: 'demo', description: 'Demo', defaultBranch: 'main' });
  const repo = profile.storage.repository(rid);
  const head = await repo.head();
  const doc = await repo.identity();
  const visibility: 'public' | 'private' = doc.visibility;
  console.log(visibility);
  const commit: Commit = await repo.commit(head.oid);
  const ms: number = commit.authorTime;
  new Date(ms);
  const bytes: Uint8Array = commit.message;
  bytes[0] = 1; // copied byte contents intentionally remain mutable
  // @ts-expect-error snapshot properties cannot be reassigned
  commit.message = new Uint8Array();
  // @ts-expect-error delegates are readonly collections
  doc.delegates.push('another');
  const labels = ['bug'] as const;
  const assignees = [profile.did] as const;
  const store = Issues.openAs(repo, signer);
  const issue = await store.create({ title: 'title', description: 'body', labels, assignees });
  const listed: ReadonlyArray<Issue> = await Issues.open(repo).all();
  // @ts-expect-error eager listing is asynchronous
  const directList: ReadonlyArray<Issue> = store.all();
  // @ts-expect-error immutable listing
  listed.push(listed[0]);
  void directList;
  const closedOptions: CloseIssueOptions = { reason: 'solved' };
  const mutations: ReadonlyArray<IssueChange> = await Promise.all([
    issue.edit('new title'), issue.editDescription('new root'), issue.label(labels), issue.assign(assignees),
    issue.close(closedOptions), issue.close(), issue.reopen(),
  ]);
  const reason: 'solved' | 'unspecified' | undefined = mutations[0].snapshot.closeReason;
  const author: string = mutations[0].snapshot.author;
  console.log(reason, author);
  // @ts-expect-error invalid close reason
  await issue.close({ reason: 'done' });
  const result: IssueChange = await issue.comment('reply');
  const snapshot: Issue = result.snapshot;
  const state: 'open' | 'closed' = snapshot.state;
  console.log(state, result.entryId);
  // @ts-expect-error readonly labels
  snapshot.labels.push('bad');
  // @ts-expect-error readonly assignees
  snapshot.assignees[0] = profile.did;
  // @ts-expect-error readonly close reason
  snapshot.closeReason = 'solved';
  // @ts-expect-error closed reason does not widen to arbitrary string
  const badReason: 'other' = snapshot.closeReason;
  void badReason;
  // @ts-expect-error readonly envelope
  result.entryId = 'another';
  // @ts-expect-error readonly nested snapshot
  snapshot.comments[0].body = 'changed';
  // @ts-expect-error finite union cannot widen
  const bad: 'pending' = snapshot.state;
  console.log(bad);
  // @ts-expect-error I/O is asynchronous
  const direct: Commit = repo.commit(head.oid);
  console.log(direct);
  await forkProject(profile.storage, signer, rid);
  await checkoutProject(profile.storage, rid, signer.nodeId, '/checkout');
}
void usage; // never execute this consumer example
