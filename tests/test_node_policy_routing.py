"""Same local databases through installed Python and Node public exports."""
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess

import pytest
import radiclepy as rad
from sdk_fixture import project_fixture

ROOT = Path(__file__).resolve().parent.parent


def node(script, profile, *args):
    if shutil.which('node') is None or not (ROOT / 'node/dist/index.js').exists():
        if os.environ.get('RADICLE_REQUIRE_CROSS_SDK') == '1':
            pytest.fail('required Node SDK unavailable')
        pytest.skip('Node SDK unavailable')
    result = subprocess.run(['node', '--input-type=module', '-e', script, *args], cwd=ROOT,
                            env=dict(os.environ, RAD_HOME=profile.home.path),
                            text=True, capture_output=True, timeout=90)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def test_policy_both_sdk_writers_and_readonly_views(tmp_path):
    profile, peer, _, repo, _, _, _ = project_fixture(tmp_path)
    policy = profile.policy_store()
    reader = rad.PolicyStore.reader(profile)
    assert policy.follow(peer.node_id, alias='RemoteBob')
    assert policy.seed(repo.rid, scope='followed')
    output = node("""
import assert from 'node:assert/strict';
import {Profile, PolicyStore} from 'radicle-sdk';
const p = Profile.load(), s = await p.policyStore(), ro = await PolicyStore.reader(p);
const [nid, rid] = process.argv.slice(1);
const before = { follows: await ro.followPolicies(), seeds: await ro.seedPolicies(),
  matches: await ro.nodesByAlias('BOB'), following: await s.isFollowing(nid), seeding: await s.isSeeding(rid) };
await assert.rejects(ro.follow(nid), e => e.code === 'READ_ONLY');
assert.equal(await s.setFollowPolicy(nid, 'block'), true);
assert.equal(await ro.isBlocked(nid), true);
assert.equal(await s.setSeedPolicy(rid, 'block'), true);
assert.equal((await ro.seedPolicy(rid)).scope, undefined);
assert.equal(await s.unblockRid(rid), true);
assert.equal(await s.seed(rid), true);
process.stdout.write(JSON.stringify(before));
""", profile, peer.node_id, repo.rid)
    assert output == {
        'follows': [{'nodeId': peer.node_id, 'alias': 'RemoteBob', 'policy': 'allow'}],
        'seeds': [{'rid': repo.rid, 'policy': 'allow', 'scope': 'followed'}],
        'matches': [[peer.node_id, 'RemoteBob']], 'following': True, 'seeding': True,
    }
    assert reader.is_blocked(peer.node_id)
    assert not reader.is_following(peer.node_id)
    assert reader.seed_policy(repo.rid).scope == 'all'
    assert policy.unfollow(peer.node_id)
    assert policy.unseed(repo.rid)
    output = node("""
import {Profile} from 'radicle-sdk';
const s = await Profile.load().policyStore();
process.stdout.write(JSON.stringify({ follows: await s.followPolicies(), seeds: await s.seedPolicies() }));
""", profile)
    assert output == {'follows': [], 'seeds': []}


def test_routing_same_storage_order_times_and_missing_values(tmp_path):
    profile, peer, _, repo, _, _, _ = project_fixture(tmp_path)
    path = Path(profile.home.path) / 'node' / 'node.db'
    # Fixture-only writes. No routing mutation or network API is added to either SDK.
    with sqlite3.connect(path) as db:
        db.executemany('INSERT OR REPLACE INTO routing(repo,node,timestamp) VALUES (?,?,?)',
                       [(repo.rid, profile.node_id, 1700000000123),
                        (repo.rid, peer.node_id, 8640000000000001)])
    routes = profile.routing_store()
    expected = {
        'nodes': routes.get(repo.rid), 'inventory': routes.get_inventory(peer.node_id),
        'entries': [{'rid': e.rid, 'nodeId': e.node_id,
                     'timestamp': e.timestamp if e.timestamp <= 8640000000000000 else None}
                    for e in routes.entries()],
        'count': routes.count(repo.rid), 'total': routes.total(), 'empty': routes.is_empty(),
        'time': routes.entry(repo.rid, profile.node_id), 'unsafeTime': None,
    }
    assert routes.entry(repo.rid, peer.node_id) == 8640000000000001
    output = node("""
import {Profile, RoutingStore} from 'radicle-sdk';
const p = Profile.load(), r = await RoutingStore.fromProfile(p), [rid,nid] = process.argv.slice(1);
process.stdout.write(JSON.stringify({nodes: await r.get(rid), inventory: await r.getInventory(nid),
 entries: await r.entries(), count: await r.count(rid), total: await r.total(), empty: await r.isEmpty(),
 time: await r.entry(rid, p.nodeId), unsafeTime: await r.entry(rid,nid)}));
""", profile, repo.rid, peer.node_id)
    assert output == expected
    with sqlite3.connect(path) as db:
        assert db.execute('SELECT timestamp FROM routing WHERE repo=? AND node=?',
                          (repo.rid, peer.node_id)).fetchone()[0] == 8640000000000001
        db.execute('DELETE FROM routing')
    output = node("""
import {Profile} from 'radicle-sdk';
const r = await Profile.load().routingStore();
process.stdout.write(JSON.stringify({entries: await r.entries(), total: await r.total(), empty: await r.isEmpty()}));
""", profile)
    assert output == {'entries': [], 'total': 0, 'empty': True}
