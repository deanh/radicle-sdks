"""Ref-only namespace discovery through both installed public SDK exports."""

import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent


def node_tips(profile, rid, *, error=False):
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK unavailable")
        pytest.skip("Node SDK unavailable")
    script = """
import assert from 'node:assert/strict';
import { Profile } from 'radicle-sdk';
const repo = Profile.load().storage.repository(process.argv[1]);
if (process.argv[2] === 'error') {
  await assert.rejects(repo.namespaceTips(), e => e.code === 'GIT');
} else {
  const tips = await repo.namespaceTips();
  assert.ok(Object.isFrozen(tips));
  for (const tip of tips) assert.ok(Object.isFrozen(tip));
  process.stdout.write(JSON.stringify(tips));
}
"""
    result = subprocess.run(
        ["node", "--input-type=module", "-e", script, rid, "error" if error else "ok"],
        cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path),
        capture_output=True, text=True, timeout=60,
    )
    assert result.returncode == 0, result.stderr
    return None if error else json.loads(result.stdout)


def test_namespace_tips_sorted_ref_only_and_cross_sdk(tmp_path):
    profile, peer, _, repo, _, stored, _ = project_fixture(tmp_path)
    repo.sign_refs(peer.signer())
    before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    tips = repo.namespace_tips()
    expected = [
        {"nodeId": tip.node_id, "target": tip.target}
        for tip in tips
    ]
    assert [tip.node_id for tip in tips] == sorted(remote.fetcher for remote in repo.remotes())
    for tip in tips:
        assert isinstance(tip, rad.NamespaceTip)
        assert tip.target == git(stored, "rev-parse", f"refs/namespaces/{tip.node_id}/refs/rad/sigrefs")
        with pytest.raises(AttributeError):
            tip.target = "0" * 40
    assert node_tips(profile, repo.rid) == expected
    assert before == git(stored, "for-each-ref", "--format=%(refname) %(objectname)")

    # A non-sigrefs object (blob) is listed without trying to load a signed set.
    blob = git(stored, "rev-parse", "HEAD:hello.txt")
    ref = f"refs/namespaces/{profile.node_id}/refs/rad/sigrefs"
    git(stored, "update-ref", ref, blob)
    assert any(tip.target == blob for tip in repo.namespace_tips())
    assert node_tips(profile, repo.rid) == [
        {"nodeId": tip.node_id, "target": tip.target} for tip in repo.namespace_tips()
    ]
    with pytest.raises(rad.GitError):
        repo.remotes()

    # Even an absent target object is listable: no object lookup is needed.
    loose = Path(stored) / ref
    loose.write_text("1" * 40 + "\n")
    assert any(tip.target == "1" * 40 for tip in repo.namespace_tips())
    assert node_tips(profile, repo.rid) == [
        {"nodeId": tip.node_id, "target": tip.target} for tip in repo.namespace_tips()
    ]
    with pytest.raises(rad.GitError):
        repo.remotes()


def test_namespace_tips_symbolic_empty_and_invalid_names(tmp_path):
    profile, _, _, repo, _, stored, _ = project_fixture(tmp_path)
    ref = f"refs/namespaces/{profile.node_id}/refs/rad/sigrefs"
    original = git(stored, "rev-parse", ref)
    git(stored, "update-ref", "refs/test/sigrefs-target", original)
    git(stored, "symbolic-ref", ref, "refs/test/sigrefs-target")
    assert next(t.target for t in repo.namespace_tips() if t.node_id == profile.node_id) == original
    assert node_tips(profile, repo.rid) == [
        {"nodeId": tip.node_id, "target": tip.target} for tip in repo.namespace_tips()
    ]
    git(stored, "symbolic-ref", ref, "refs/test/missing")
    with pytest.raises(rad.GitError):
        repo.namespace_tips()
    node_tips(profile, repo.rid, error=True)
    git(stored, "symbolic-ref", "--delete", ref)
    for tip in repo.namespace_tips():
        git(stored, "update-ref", "-d", f"refs/namespaces/{tip.node_id}/refs/rad/sigrefs")
    assert repo.namespace_tips() == []
    assert node_tips(profile, repo.rid) == []
    git(stored, "update-ref", "refs/namespaces/not-a-node/refs/rad/sigrefs", original)
    with pytest.raises(rad.GitError):
        repo.namespace_tips()
    node_tips(profile, repo.rid, error=True)
