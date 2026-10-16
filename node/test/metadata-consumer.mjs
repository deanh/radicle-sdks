// Isolated OS environment lets the upstream writer sign arbitrary COB dates,
// without mutating the test runner's environment or adding a SDK timestamp API.
import assert from 'node:assert/strict';
import { Profile, Issues } from '../dist/index.js';
const profile = Profile.load();
const issue = await Issues.openAs(profile.storage.repository(process.argv[2]), profile.signer())
  .create({ title: 'Peer date', description: 'Root with unrepresentable date' });
const change = await issue.comment('Peer discussion date');
assert.equal(change.snapshot.comments.length, 2);
for (const comment of change.snapshot.comments) {
  assert.ok(Object.hasOwn(comment, 'timestamp'));
  assert.equal(comment.timestamp, null);
}
console.log(issue.id);
