"""Node async setup reads/decrypts the same profile as Python's sync API."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest
import radiclepy as rad
from sdk_fixture import project_fixture

ROOT = Path(__file__).resolve().parent.parent


def test_async_node_setup_and_python_use_same_encrypted_identity(tmp_path):
    if shutil.which('node') is None or not (ROOT / 'node/dist/index.js').exists():
        if os.environ.get('RADICLE_REQUIRE_CROSS_SDK') == '1':
            pytest.fail('required Node SDK unavailable')
        pytest.skip('Node SDK unavailable')
    _, _, _, repo, _, stored, _ = project_fixture(tmp_path)
    encrypted = rad.Profile.init(tmp_path / 'encrypted', 'encrypted', 'fixture-only')
    shutil.copytree(stored, Path(encrypted.storage.path) / repo.rid[4:])
    env = dict(os.environ, RAD_HOME=encrypted.home.path, RAD_PASSPHRASE='fixture-only', UV_THREADPOOL_SIZE='1')
    env.pop('SSH_AUTH_SOCK', None)
    result = subprocess.run(['node', str(ROOT / 'node/test/async-profile-worker.mjs'), str(tmp_path), repo.rid],
        cwd=ROOT, env=env, capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout)
    assert data['did'] == encrypted.did
    assert data['nodeId'] == encrypted.node_id
    assert data['queued']
    assert rad.Issues.open(encrypted.storage.repository(repo.rid)).get(data['issueId']).author == encrypted.did
    result = subprocess.run([sys.executable, '-c', '''
import json, radiclepy as r
p = r.Profile.load()
s = p.signer()
print(json.dumps(dict(did=p.did, nodeId=s.node_id)))
'''], cwd=ROOT, env=env, capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == dict(did=data['did'], nodeId=data['nodeId'])
