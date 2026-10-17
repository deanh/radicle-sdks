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
        "ids": [rad.parse_repo_id(repo.rid), rad.parse_did(profile.did),
                rad.parse_node_id(profile.node_id), rad.parse_oid(base.upper()),
                rad.parse_object_id(base.upper())],
    }
    script = """
import { Profile, parseRepoId, parseDid, parseNodeId, parseOid, parseObjectId } from 'radicle-sdk';
const p = Profile.load();
const repo = p.storage.repository(process.argv[1]);
const oid = (await repo.head()).oid.toUpperCase();
process.stdout.write(JSON.stringify({
  description: await repo.description(), identity: await repo.identity(),
  tags: await repo.tags(), remotes: await repo.remotes(),
  resolved: await repo.revParse('refs/tags/annotated^{}'),
  ids: [parseRepoId(repo.rid), parseDid(p.did), parseNodeId(p.nodeId), parseOid(oid), parseObjectId(oid)]
}));
"""
    result = subprocess.run(["node", "--input-type=module", "-e", script, repo.rid],
                            cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path),
                            text=True, capture_output=True, timeout=60)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == expected
