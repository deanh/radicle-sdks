"""Capability parity fixtures use installed public exports, not private bindings."""
import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent


def require_node():
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK is not built/available")
        pytest.skip("Node SDK not built/available")


def test_repository_metadata_and_validators_match(tmp_path):
    require_node()
    profile, _, _, repo, _, stored, base = project_fixture(tmp_path)
    git(stored, "tag", "light", base)
    git(stored, "-c", "tag.gpgsign=false", "tag", "-a", "annotated", base, "-m", "tag")
    repo.sign_refs(profile.signer())
    doc = repo.identity()
    expected = {
        "description": repo.description(),
        "identity": {key: getattr(doc, attr) for key, attr in [
            ("name", "name"), ("description", "description"),
            ("defaultBranch", "default_branch"), ("version", "version"),
            ("delegates", "delegates"), ("threshold", "threshold"),
            ("visibility", "visibility"), ("commit", "commit"), ("blob", "blob"),
        ]},
        "tags": [{"name": t.name, "target": t.target, "kind": t.kind,
                  "peeled": t.peeled, "peeledKind": t.peeled_kind} for t in repo.tags()],
        "remotes": [{"fetcher": r.fetcher, "heads": r.heads} for r in repo.remotes()],
        "resolved": repo.rev_parse("refs/tags/annotated^{}"),
        "utilities": {"home": profile.home.path, "storage": profile.storage.path,
                      "alias": profile.aliases.get(profile.node_id),
                      "reverse": profile.aliases.reverse_lookup("alice"),
                      "did": profile.signer().did},
        "stores": {"issues": rad.Issues.open(repo).count(),
                   "patches": rad.Patches.open(repo).count(),
                   "issueEmpty": rad.Issues.open(repo).is_empty(),
                   "patchEmpty": rad.Patches.open(repo).is_empty()},
        "ids": [rad.parse_repo_id(repo.rid), rad.parse_did(profile.did),
                rad.parse_node_id(profile.node_id), rad.parse_oid(base.upper()),
                rad.parse_object_id(base.upper())],
    }
    script = """
import { Profile, Storage, Home, Issues, Patches, parseRepoId, parseDid, parseNodeId, parseOid, parseObjectId } from 'radicle-sdk';
const p = Profile.load();
const repo = p.storage.repository(process.argv[1]);
const oid = (await repo.head()).oid.toUpperCase();
process.stdout.write(JSON.stringify({
  description: await repo.description(), identity: await repo.identity(),
  tags: await repo.tags(), remotes: await repo.remotes(),
  resolved: await repo.revParse('refs/tags/annotated^{}'),
  utilities: { home: (await Home.load(p.home.path)).path,
    storage: (await Storage.open(p.storage.path, 'alice', p.nodeId)).path,
    alias: await p.aliases.get(p.nodeId), reverse: await p.aliases.reverseLookup('alice'), did: p.signer().did },
  stores: { issues: await Issues.open(repo).count(), patches: await Patches.open(repo).count(),
    issueEmpty: await Issues.open(repo).isEmpty(), patchEmpty: await Patches.open(repo).isEmpty() },

  ids: [parseRepoId(repo.rid), parseDid(p.did), parseNodeId(p.nodeId), parseOid(oid), parseObjectId(oid)]
}));
"""
    result = subprocess.run(["node", "--input-type=module", "-e", script, repo.rid],
                            cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path),
                            text=True, capture_output=True, timeout=60)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == expected


def test_store_conveniences_observe_both_sdk_writers(tmp_path):
    require_node()
    profile, _, _, repo, _, _, base = project_fixture(tmp_path)
    issues = rad.Issues.open_as(repo, profile.signer())
    issue = issues.create("python", "root")
    issue.comment("python reply")
    patches = rad.Patches.open_as(repo, profile.signer())
    patch = patches.draft("python patch", "root", base=base, head=base)
    patch.update("second", base=base, head=base)
    script = """
import assert from 'node:assert/strict';
import { Profile, Issues, Patches } from 'radicle-sdk';
const p = Profile.load(), repo = p.storage.repository(process.argv[1]);
const issues = Issues.openAs(repo, p.signer()), patches = Patches.openAs(repo, p.signer());
assert.equal(issues.rid, repo.rid); assert.equal(patches.rid, repo.rid);
assert.equal(await issues.count(), 1); assert.equal(await patches.count(), 1);
assert.equal(await issues.isEmpty(), false); assert.equal(await patches.isEmpty(), false);
const issue = await issues.get(process.argv[2]), patch = await patches.get(process.argv[3]);
assert.equal(issue.commentCount, 2);
assert.deepEqual(issue.commentIds, issue.comments.map(c => c.id));
assert.equal(patch.revisionCount, 2);
assert.equal(patch.revisions.length, 2);
const reply = await (await issues.getMut(issue.id)).comment('node reply');
assert.equal(reply.snapshot.commentCount, 3);
await issues.create({title: 'node', description: 'root'});
await (await patches.getMut(patch.id)).update({description: 'third', base: patch.base, head: patch.head});
process.stdout.write(JSON.stringify(reply.snapshot.commentIds));
"""
    result = subprocess.run(["node", "--input-type=module", "-e", script, repo.rid, issue.id, patch.id],
                            cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path),
                            text=True, capture_output=True, timeout=60)
    assert result.returncode == 0, result.stderr
    assert issues.count() == 2
    assert issue.issue.comment_count == 2  # Python cached snapshot is detached
    assert issue.reload().comment_ids == json.loads(result.stdout)
    assert issue.issue.comment_count == 3
    assert patches.count() == 1
    assert patch.reload().revision_count == 3
