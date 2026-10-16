// Compile-only consumer against the generated patch DTOs and public wrapper.
import { Patches, type Repository, type Signer, type Patch, type Revision, type Review,
  type PatchChange, type PatchRevisionChange, type PatchStateChange } from '../dist/index.js';

async function usage(repo: Repository, signer: Signer, base: string, head: string): Promise<void> {
  const store = Patches.openAs(repo, signer);
  const labels = ['bug'] as const;
  const patch = await store.draft({ title: 'Patch', description: 'First', base, head, labels });
  await store.create({ title: 'Open', description: '', base, head, target: 'branch:main' });
  // @ts-expect-error explicit head bound is required
  await store.create({ title: 'No head', description: '', base });
  // @ts-expect-error finite target domain
  await store.create({ title: 'Bad', description: '', base, head, target: 'main' });
  const snapshot: Patch = await patch.reload();
  const state: 'draft' | 'open' | 'archived' | 'merged' = snapshot.state;
  const target: 'delegates' | `branch:${string}` = snapshot.target;
  const revision: Revision = snapshot.revisions.find(r => r.id === snapshot.latestRevision)!;
  const changed: PatchRevisionChange = await patch.update({ description: 'Next', base, head });
  const revisionId: string = changed.revisionId;
  // @ts-expect-error updates return revisionId, not entryId
  console.log(changed.entryId);
  const transition: PatchStateChange = await patch.ready();
  const flag: boolean = transition.changed;
  // @ts-expect-error state transitions do not invent an entryId
  console.log(transition.entryId);
  const entry: PatchChange = await patch.comment(revisionId, 'Comment');
  await patch.comment(revisionId, 'Reply', { replyTo: entry.entryId });
  await patch.review(revisionId, { verdict: 'accept', summary: 'Approved' });
  await patch.review(revisionId, { summary: 'Comment-only review' });
  // @ts-expect-error finite verdict domain
  await patch.review(revisionId, { verdict: 'approve' });
  await patch.label(labels);
  await patch.assign([snapshot.author] as const);
  await patch.edit({ title: 'Title', target: 'delegates' });
  // @ts-expect-error editing requires an explicit target, not an implicit reset
  await patch.edit({ title: 'Title' });
  await patch.unready(); await patch.archive(); await patch.unarchive();
  const merged: PatchChange = await patch.merge(revision.id, head);
  const mergedRevision: string | undefined = merged.snapshot.mergedRevision;
  const mergedCommit: string | undefined = merged.snapshot.mergedCommit;
  const review: Review = revision.reviews[0];
  const verdict: 'accept' | 'reject' | undefined = review.verdict;
  const time: number | null = review.timestamp;
  const revisionTime: number | null = revision.timestamp;
  if (time !== null) new Date(time);
  // @ts-expect-error metadata timestamps may be null
  const requiredTime: number = revision.timestamp;
  // @ts-expect-error detached snapshots are readonly
  revision.comments[0].body = 'changed';
  // @ts-expect-error nested collections are readonly
  revision.reviews.push(review);
  // @ts-expect-error review properties are readonly
  review.verdict = 'reject';
  // @ts-expect-error snapshot properties are readonly
  snapshot.latestRevision = revision.id;
  // @ts-expect-error no mutable snapshot cache on handles
  console.log(patch.patch);
  const list: ReadonlyArray<Patch> = await Patches.open(repo).all();
  // @ts-expect-error listings are frozen/readonly arrays
  list.push(snapshot);
  const missing: Patch | null = await store.get('id');
  console.log(state, target, flag, mergedRevision, mergedCommit, verdict, revisionTime, requiredTime, missing);
}
void usage;
