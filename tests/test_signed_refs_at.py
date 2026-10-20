"""Explicit-tip reads delegate verification to pinned upstream in both SDKs."""

import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent


def snapshot(signed):
    return {
        "nodeId": signed.node_id,
        "at": signed.at,
        "parent": signed.parent,
        "featureLevel": signed.feature_level,
        "refs": signed.refs(),
    }


def node_read(profile, rid, nid, oid, *, code=None):
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK unavailable")
        pytest.skip("Node SDK unavailable")
    script = """
import assert from 'node:assert/strict';
import { Profile } from 'radicle-sdk';
const [rid, nid, oid, code] = process.argv.slice(1);
const repo = Profile.load().storage.repository(rid);
if (code) {
  await assert.rejects(repo.signedRefsAt(nid, oid), e => e.code === code);
} else {
  const signed = await repo.signedRefsAt(nid, oid);
  assert.ok(Object.isFrozen(signed));
  assert.ok(Object.isFrozen(signed.refs));
  process.stdout.write(JSON.stringify({ ...signed, parent: signed.parent ?? null }));
}
"""
    result = subprocess.run(
        ["node", "--input-type=module", "-e", script, rid, nid, oid, code or ""],
        cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path),
        text=True, capture_output=True, timeout=90,
    )
    assert result.returncode == 0, result.stderr
    return None if code else json.loads(result.stdout)


def reject(profile, repo, nid, oid, *, argument=False):
    with pytest.raises(ValueError if argument else rad.GitError) as caught:
        repo.signed_refs_at(nid, oid)
    if not argument:
        assert oid in str(caught.value)
    node_read(profile, repo.rid, nid, oid, code="ERR_INVALID_ARG_VALUE" if argument else "GIT")


def test_signed_refs_at_historical_current_and_without_reference(tmp_path):
    profile, _, _, repo, _, stored, _ = project_fixture(tmp_path)
    old = repo.sign_refs(profile.signer())
    issue = rad.Issues.open_as(repo, profile.signer()).create("signed", "root")
    current = repo.sign_refs(profile.signer())
    assert current.at != old.at
    assert current.parent == old.at
    before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    for signed in [old, current]:
        loaded = repo.signed_refs_at(profile.node_id, signed.at)
        # Upstream promotes a verified root's feature level to parent on read.
        expected = snapshot(signed)
        if signed.parent is None and signed.feature_level == "root":
            expected["featureLevel"] = "parent"
        assert snapshot(loaded) == expected
        assert loaded == repo.signed_refs_at(profile.node_id, signed.at)
        assert hash(loaded) == hash(repo.signed_refs_at(profile.node_id, signed.at))
        assert node_read(profile, repo.rid, profile.node_id, signed.at) == expected
        copy = loaded.refs()
        copy.clear()
        assert loaded.refs() == signed.refs()
    ref = f"refs/cobs/xyz.radicle.issue/{issue.id}"
    assert ref not in old.refs()
    assert current.refs()[ref] == issue.id
    remote = next(r for r in repo.remotes() if r.fetcher == profile.node_id)
    assert sorted(current.refs()) == remote.heads
    assert git(stored, "for-each-ref", "--format=%(refname) %(objectname)") == before
    sigref = f"refs/namespaces/{profile.node_id}/refs/rad/sigrefs"
    git(stored, "update-ref", "-d", sigref)
    assert repo.signed_refs_at(profile.node_id, old.at).refs() == old.refs()
    assert node_read(profile, repo.rid, profile.node_id, current.at) == snapshot(current)
    assert not repo.namespace_tips()
    git(stored, "reflog", "expire", "--expire=now", "--all")
    git(stored, "gc", "--prune=now")
    reject(profile, repo, profile.node_id, old.at)


def test_signed_refs_at_invalid_arguments_and_upstream_verification(tmp_path):
    profile, peer, _, repo, _, stored, base = project_fixture(tmp_path)
    signed = repo.sign_refs(profile.signer())
    reject(profile, repo, "bad-node", signed.at, argument=True)
    reject(profile, repo, profile.node_id, "bad-oid", argument=True)
    reject(profile, repo, profile.node_id, "1" * 40)
    reject(profile, repo, profile.node_id, git(stored, "rev-parse", "HEAD:hello.txt"))
    reject(profile, repo, profile.node_id, base)  # commit without signed-set tree
    reject(profile, repo, peer.node_id, signed.at)  # namespace key mismatch

    # Legacy root-level signatures can be replayed in another Git commit.
    # Upstream walks the chain and yields the original, not the requested OID.
    tree = git(stored, "rev-parse", f"{signed.at}^{{tree}}")
    replay = git(stored, "commit-tree", tree, "-p", signed.at, "-m", "replayed signature")
    loaded = repo.signed_refs_at(profile.node_id, replay)
    assert loaded.at == signed.at
    assert loaded.at != replay
    assert loaded.refs() == signed.refs()
    assert node_read(profile, repo.rid, profile.node_id, replay) == snapshot(loaded)

    # Parent-feature sets bind the parent into the signed payload and reject
    # the same tree when placed under a different parent.
    rad.Issues.open_as(repo, profile.signer()).create("parent", "root")
    modern = repo.sign_refs(profile.signer())
    modern_tree = git(stored, "rev-parse", f"{modern.at}^{{tree}}")
    wrong_parent = git(stored, "commit-tree", modern_tree, "-p", modern.at, "-m", "wrong parent")
    reject(profile, repo, profile.node_id, wrong_parent)

    # Alter the refs blob without changing the signature.
    original = git(stored, "show", f"{signed.at}:refs")
    bad_refs = Path(tmp_path) / "bad-refs"
    bad_refs.write_text(original.replace(base, "1" * 40) + "\n")
    blob = git(stored, "hash-object", "-w", str(bad_refs))
    entries = git(stored, "ls-tree", tree)
    changed = "\n".join(
        f"100644 blob {blob}\trefs" if line.endswith("\trefs") else line
        for line in entries.splitlines()
    ) + "\n"
    made = subprocess.run(["git", "-C", str(stored), "mktree"], input=changed,
                          capture_output=True, text=True, check=True).stdout.strip()
    args = ["commit-tree", made, "-m", "tampered refs"]
    if signed.parent:
        args += ["-p", signed.parent]
    tampered = git(stored, *args)
    reject(profile, repo, profile.node_id, tampered)


def test_signed_refs_at_rejects_wrong_repository_identity(tmp_path):
    profile, _, storage, repo, _, stored, _ = project_fixture(tmp_path / "one")
    # Initialize a second RID with the same signer: namespace matches but RID
    # binding must still reject the first repository's signed set.
    _, _, _, _, work, _, _ = project_fixture(tmp_path / "two")
    git(work, "remote", "remove", "rad")
    rid = rad.init_project(storage, profile.signer(), work, name="other", description="other RID", default_branch="main")
    other = storage.repository(rid)
    signed = repo.sign_refs(profile.signer())
    other_stored = Path(storage.path) / rid.removeprefix("rad:")
    git(other_stored, "fetch", str(stored), signed.at)
    reject(profile, other, profile.node_id, signed.at)
