"""Whole-set replacement and recorded-vs-effective reviews across both SDKs."""
import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest
import radiclepy as rad
from sdk_fixture import project_fixture

ROOT = Path(__file__).resolve().parent.parent


def test_set_replacement_stale_inputs_and_review_receipts_cross_sdk(tmp_path):
    if shutil.which('node') is None or not (ROOT / 'node/dist/index.js').exists():
        if os.environ.get('RADICLE_REQUIRE_CROSS_SDK') == '1':
            pytest.fail('required Node SDK unavailable')
        pytest.skip('Node SDK unavailable')
    profile, peer, _, repo, _, _, base = project_fixture(tmp_path)
    signer = profile.signer()
    issue = rad.Issues.open_as(repo, signer).create('sets', 'root', labels=['one', 'two'])
    patch = rad.Patches.open_as(repo, signer).create('sets', 'root', base=base, head=base, labels=['one', 'two'])
    for handle in (issue, patch):
        handle.label(['two'])
        handle.assign([profile.did, peer.did])
        handle.assign([peer.did])
        snapshot = handle.reload()
        assert snapshot.labels == ['two']
        assert snapshot.assignees == [peer.did]
    revision = patch.reload().latest_revision
    first = patch.review(revision, verdict='accept', summary='first')
    script = '''
import assert from 'node:assert/strict';
import {Profile,Issues,Patches} from 'radicle-sdk';
const p=Profile.load(), repo=p.storage.repository(process.argv[1]), signer=p.signer();
const issue=await Issues.openAs(repo,signer).getMut(process.argv[2]);
const patch=await Patches.openAs(repo,signer).getMut(process.argv[3]);
for(const handle of [issue,patch]) {
  const before=await handle.reload();
  assert.deepEqual(before.labels,['two']);
  assert.deepEqual(before.assignees,[process.argv[4]]);
  // Two serialized writes with inputs derived from the SAME stale read.
  await handle.label([...before.labels,'first']);
  const last=await handle.label([...before.labels,'second']);
  assert.deepEqual(last.snapshot.labels,['second','two']); // not an incremental union
  assert.ok(last.entryId);
  await handle.label([]);
  const cleared=await handle.assign([]);
  assert.deepEqual(cleared.snapshot.labels,[]);
  assert.deepEqual(cleared.snapshot.assignees,[]);
}
const changed=await patch.review(process.argv[5],{verdict:'reject',summary:'second'});
assert.notEqual(changed.entryId,process.argv[6]);
const observed=changed.snapshot.revisions.find(r=>r.id===process.argv[5]).reviews.find(r=>r.author===p.did);
assert.equal(observed.id,process.argv[6]);
assert.equal(observed.verdict,'accept');
console.log(JSON.stringify({recorded:changed.entryId,observed:observed.verdict}));
'''
    result = subprocess.run(['node', '--input-type=module', '-e', script, repo.rid, issue.id, patch.id, peer.did, revision, first],
        cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path), capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)['observed'] == 'accept'
    for handle in (issue, patch):
        snapshot = handle.reload()
        assert snapshot.labels == []
        assert snapshot.assignees == []
    review = next(r for r in patch.reload().revisions[0].reviews if r.author == profile.did)
    assert review.id == first
    assert review.verdict == 'accept'
