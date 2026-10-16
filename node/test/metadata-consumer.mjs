// Isolated OS environment lets the upstream writer sign arbitrary COB dates,
// without mutating the test runner's environment or adding a SDK timestamp API.
import assert from 'node:assert/strict';
import { Profile, Issues, Patches } from '../dist/index.js';
const profile = Profile.load();
const repo = profile.storage.repository(process.argv[2]),
  signer = profile.signer();
if (process.argv[3] === 'patch') {
  const base = process.argv[4];
  const patch = await Patches.openAs(repo, signer).create({
    title: 'Peer date',
    description: 'Peer revision',
    base,
    head: base,
  });
  const revision = (await patch.reload()).latestRevision;
  await patch.comment(revision, 'Peer discussion date');
  const change = await patch.review(revision, { verdict: 'accept', summary: 'Peer review date' });
  const r = change.snapshot.revisions[0];
  for (const value of [r, ...r.comments, ...r.reviews]) {
    assert.ok(Object.hasOwn(value, 'timestamp'));
    assert.equal(value.timestamp, null);
  }
  console.log(patch.id);
} else {
  const issue = await Issues.openAs(repo, signer).create({
    title: 'Peer date',
    description: 'Root with unrepresentable date',
  });
  const change = await issue.comment('Peer discussion date');
  assert.equal(change.snapshot.comments.length, 2);
  for (const comment of change.snapshot.comments) {
    assert.ok(Object.hasOwn(comment, 'timestamp'));
    assert.equal(comment.timestamp, null);
  }
  console.log(issue.id);
}
