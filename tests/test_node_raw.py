"""Generic raw COB conformance through installed exports and one Git storage."""
import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest
import radiclepy as rad
from sdk_fixture import project_fixture, git

ROOT = Path(__file__).resolve().parent.parent
ISSUE = 'xyz.radicle.issue'


def node(profile, rid, typename, entity, *, decode=True):
    if shutil.which('node') is None or not (ROOT / 'node/dist/index.js').exists():
        if os.environ.get('RADICLE_REQUIRE_CROSS_SDK') == '1':
            pytest.fail('required Node SDK unavailable')
        pytest.skip('Node SDK unavailable')
    script = """
import {Profile, Raw} from 'radicle-sdk';
import assert from 'node:assert/strict';
const [rid,type,id,decode] = process.argv.slice(1), raw = Raw.open(Profile.load().storage.repository(rid));
const ops = await raw.history(type,id);
if (decode==='no') for (const op of ops) assert.throws(()=>op.actionsJson(), e=>e.code==='COB');
process.stdout.write(JSON.stringify({types:await raw.typenames(),entities:await raw.entities(type),refs:await raw.refs(type,id),
 ops:ops.map(o=>({id:o.id,actor:o.actor,timestamp:o.timestamp.toString(),actions:o.actions.map(a=>Buffer.from(a).toString('hex')),
 parents:o.parents,related:o.related,identity:o.identity??null,manifest:{typeName:o.manifest.typeName,version:o.manifest.version.toString()},
 json:decode==='yes'?o.actionsJson():undefined}))}, (key,value)=>typeof value==='bigint'?value.toString():value));
"""
    result = subprocess.run(['node', '--input-type=module', '-e', script, rid, typename, entity, 'yes' if decode else 'no'], cwd=ROOT,
                            env=dict(os.environ, RAD_HOME=profile.home.path), text=True, capture_output=True, timeout=90)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def python(raw, typename, entity, *, decode=True):
    return {'types':raw.typenames(), 'entities':raw.entities(typename),
            'refs':[{'name':r.name, 'target':r.target} for r in raw.refs(typename,entity)],
            'ops':[{'id':o.id,'actor':o.actor,'timestamp':str(o.timestamp),'actions':[a.hex() for a in o.actions],
                    'parents':list(o.parents),'related':list(o.related),'identity':o.identity,
                    'manifest':{'typeName':o.manifest.type_name,'version':str(o.manifest.version)},
                    **({'json':list(o.actions_json())} if decode else {})}
                   for o in raw.history(typename,entity)]}


def test_raw_multiref_dag_order_links_bytes_and_strict_enumeration(tmp_path):
    p, peer, _, repo, _, stored, base = project_fixture(tmp_path)
    issue = rad.Issues.open_as(repo,p.signer()).create('dag','root')
    ref = f'refs/namespaces/{p.node_id}/refs/cobs/{ISSUE}/{issue.id}'
    a = issue.comment('alice')
    git(stored,'update-ref',ref,issue.id)
    b = rad.Issues.open_as(repo,peer.signer()).get_mut(issue.id).comment('bob')
    git(stored,'update-ref',ref,a)
    patch = rad.Patches.open_as(repo,p.signer()).create('patch','related',base=base,head=base)
    for name in [f'refs/tags/xyz.fake.type/{base}', f'refs/heads/xyz.fake.type/{base}',
                 f'refs/cobs/{ISSUE}/bad-id', f'refs/namespaces/{p.node_id}/refs/cobs/bad_type/{base}']:
        git(stored,'update-ref',name,base)
    raw = rad.Raw.open(repo)
    before = git(stored,'for-each-ref','--format=%(refname) %(objectname)')
    expected = python(raw,ISSUE,issue.id)
    assert [op['id'] for op in expected['ops']] == [issue.id, *sorted([a,b])]
    assert node(p,repo.rid,ISSUE,issue.id) == expected
    assert node(p,repo.rid,'xyz.radicle.patch',patch.id) == python(raw,'xyz.radicle.patch',patch.id)
    assert node(p,repo.rid,ISSUE,'0'*40) == python(raw,ISSUE,'0'*40)
    assert git(stored,'for-each-ref','--format=%(refname) %(objectname)') == before


def hash_object(stored, kind, contents):
    return subprocess.run(['git','-C',str(stored),'hash-object','--literally','-t',kind,'-w','--stdin'],
                          input=contents,capture_output=True,check=True).stdout.decode().strip()


def test_raw_large_metadata_and_invalid_json_do_not_hide_bytes(tmp_path):
    p, _, _, repo, _, stored, _ = project_fixture(tmp_path)
    issue = rad.Issues.open_as(repo,p.signer()).create('raw','root')
    commit = subprocess.run(['git','-C',str(stored),'cat-file','commit',issue.id],capture_output=True,check=True).stdout
    rows = git(stored,'ls-tree',f'{issue.id}^{{tree}}').splitlines()
    rewritten = []
    for row in rows:
        mode, kind, rest = row.split(' ',2)
        oid, name = rest.split('\t',1)
        if name.isdigit():
            # Fixture-only tampering: raw loading parses signature metadata but
            # does not authenticate it. No claim that copied signatures verify.
            oid = hash_object(stored,'blob',b'{invalid json')
        else:
            manifest = json.loads(git(stored,'cat-file','blob',oid))
            manifest['version'] = 18446744073709551615
            oid = hash_object(stored,'blob',json.dumps(manifest).encode())
        rewritten.append(f'{mode} {kind} {oid}\t{name}\n')
    tree = subprocess.run(['git','-C',str(stored),'mktree'],input=''.join(rewritten),text=True,capture_output=True,check=True).stdout.strip()
    commit = b'tree '+tree.encode()+b'\n'+commit.split(b'\n',1)[1]
    # Preserve all other signature/trailer metadata; maximum positive i64 seconds.
    import re
    commit = re.sub(rb'(committer .*?> )\d+ ([+-]\d{4})',rb'\g<1>9223372036854775807 \2',commit)
    oid = hash_object(stored,'commit',commit)
    git(stored,'update-ref',f'refs/namespaces/{p.node_id}/refs/cobs/{ISSUE}/{issue.id}',oid)
    raw = rad.Raw.open(repo)
    op = raw.history(ISSUE,issue.id)[0]
    assert op.timestamp == 9223372036854775807
    assert op.manifest.version == 18446744073709551615
    with pytest.raises(rad.CobError): op.actions_json()
    assert node(p,repo.rid,ISSUE,issue.id,decode=False) == python(raw,ISSUE,issue.id,decode=False)
