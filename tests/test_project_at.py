"""Profile-free explicit-path discovery through both public SDK exports."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest
import radiclepy as rad

from sdk_fixture import git

ROOT = Path(__file__).resolve().parent.parent
RID = "rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg"
NID = "z6MkgNk7k9nXpHP8vgyqCNTgSh8nAfia21QaSFyhLCSntynx"
URL = "rad://" + RID[4:]


def both(path, remote="rad", *, error=False, env=None):
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK unavailable")
        pytest.skip("Node SDK unavailable")
    node = shutil.which("node")
    scripts = [
        ([sys.executable, "-c"], """
import json, os, sys
import radiclepy as r
cwd = os.getcwd()
try:
    value = r.project_at(sys.argv[1], remote_name=sys.argv[2])
except r.GitError:
    assert sys.argv[3] == 'error'
    print('null')
else:
    assert sys.argv[3] != 'error'
    print(json.dumps(None if value is None else dict(rid=value.rid, remoteName=value.remote_name, namespace=value.namespace)))
assert os.getcwd() == cwd
"""),
        ([node, "--input-type=module", "-e"], """
import assert from 'node:assert/strict';
import {projectAt} from 'radicle-sdk';
const cwd = process.cwd();
if (process.argv[3] === 'error') {
  await assert.rejects(projectAt(process.argv[1], {remoteName:process.argv[2]}), e => e.code === 'GIT');
  console.log('null');
} else {
  const value = await projectAt(process.argv[1], {remoteName:process.argv[2]});
  assert.ok(value === null || Object.isFrozen(value));
  console.log(JSON.stringify(value === null ? null : {...value, namespace:value.namespace ?? null}));
}
assert.equal(process.cwd(), cwd);
"""),
    ]
    values = []
    for command, script in scripts:
        result = subprocess.run(command + [script, str(path), remote, "error" if error else "ok"],
            cwd=ROOT, env=dict(os.environ, RAD_HOME=str(path.parent / "no-profile"), PATH="", **(env or {})),
            capture_output=True, text=True, timeout=60)
        assert result.returncode == 0, result.stderr
        values.append(json.loads(result.stdout))
    assert values[0] == values[1]
    return values[0]


def worktree(tmp_path):
    work = tmp_path / "work"
    work.mkdir()
    git(work, "init", "--initial-branch=main")
    (work / "file").write_text("initial\n")
    git(work, "add", ".")
    git(work, "-c", "commit.gpgsign=false", "commit", "-m", "initial")
    return work


def test_project_at_paths_worktrees_remote_selection_and_no_writes(tmp_path):
    work = worktree(tmp_path)
    git(work, "remote", "add", "rad", URL)
    git(work, "remote", "add", "peer", URL + "/" + NID)
    git(work, "remote", "set-url", "--push", "rad", "not-a-fetch-url")
    nested = work / "nested" / "child"
    nested.mkdir(parents=True)
    linked = tmp_path / "linked λ"
    git(work, "worktree", "add", "-b", "linked", str(linked))
    config = (work / ".git/config").read_bytes()
    refs = git(work, "for-each-ref", "--format=%(refname) %(objectname)")
    cwd = os.getcwd()
    expected = dict(rid=RID, remoteName="rad", namespace=None)
    for path in (work, nested, linked):
        assert both(path) == expected
        value = rad.project_at(path)
        assert isinstance(value, rad.ProjectLocation)
        assert value == rad.project_at(str(path))
        assert {value: "value"}[rad.project_at(path)] == "value"
        assert repr(value) == f"ProjectLocation({RID}, rad)"
        with pytest.raises(AttributeError):
            value.rid = "invalid"
    assert both(nested, "peer") == dict(rid=RID, remoteName="peer", namespace=NID)
    assert both(nested, env={"GIT_DIR": str(tmp_path / "not-a-repository")}) == expected
    assert os.getcwd() == cwd
    assert (work / ".git/config").read_bytes() == config
    assert git(work, "for-each-ref", "--format=%(refname) %(objectname)") == refs
    git(work, "remote", "remove", "rad")
    assert rad.project_at(work) is None  # never guess another remote
    assert both(work) is None
    assert both(work, "peer")["namespace"] == NID
    bare = tmp_path / "bare"
    bare.mkdir()
    git(bare, "init", "--bare")
    git(bare, "remote", "add", "rad", URL)
    assert both(bare) == expected


def test_project_at_absence_configuration_errors_and_argument_errors(tmp_path):
    empty = tmp_path / "empty"
    empty.mkdir()
    assert both(empty) is None
    work = worktree(tmp_path)
    assert both(work) is None
    both(tmp_path / "absent", error=True)
    git(work, "remote", "add", "rad", URL)
    for invalid in ("https://example.invalid/repo", "rad://invalid", URL + "/invalid", URL + "/", URL + "/" + NID + "/extra"):
        git(work, "remote", "set-url", "rad", invalid)
        both(work, error=True)
    git(work, "config", "--unset", "remote.rad.url")
    both(work, error=True)
    git(work, "config", "remote.rad.url", URL)
    config = work / ".git/config"
    valid = config.read_bytes()
    config.write_bytes(valid + b'[remote "bad-utf8"]\nurl = \xff\n')
    both(work, "bad-utf8", error=True)
    config.write_bytes(valid + b'[broken config\n')
    both(work, error=True)
    config.write_bytes(valid)
    for name in ("", "bad name", "bad..name"):
        with pytest.raises(ValueError):
            rad.project_at(work, remote_name=name)
    for path in (None, 12, b"bytes"):
        with pytest.raises(TypeError):
            rad.project_at(path)
    broken = tmp_path / "broken"
    broken.mkdir()
    (broken / ".git").write_text("not a valid gitfile\n")
    both(broken, error=True)
