"""Plain Git ancestry across bindings, including graph errors rather than false."""

import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent


def node_queries(profile, rid, queries):
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK unavailable")
        pytest.skip("Node SDK unavailable")
    script = """
import assert from 'node:assert/strict';
import {Profile} from 'radicle-sdk';
const [rid,queries]=process.argv.slice(1);
const repo=Profile.load().storage.repository(rid);
for(const [ancestor,descendant,expected] of JSON.parse(queries)) {
  if (typeof expected === 'boolean') assert.equal(await repo.isAncestor(ancestor,descendant),expected);
  else await assert.rejects(repo.isAncestor(ancestor,descendant),e=>e.code===expected);
}
"""
    result = subprocess.run(["node", "--input-type=module", "-e", script, rid, json.dumps(queries)],
        cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path), text=True, capture_output=True, timeout=60)
    assert result.returncode == 0, result.stderr


def test_plain_git_reflexive_branch_merge_and_unrelated_ancestry(tmp_path):
    profile, _, _, repo, _, stored, root = project_fixture(tmp_path)
    tree = git(stored, "rev-parse", f"{root}^{{tree}}")
    left = git(stored, "commit-tree", tree, "-p", root, "-m", "left")
    right = git(stored, "commit-tree", tree, "-p", root, "-m", "right")
    merge = git(stored, "commit-tree", tree, "-p", left, "-p", right, "-m", "merge")
    unrelated = git(stored, "commit-tree", tree, "-m", "unrelated")
    queries = [(root, root, True), (root, merge, True), (left, merge, True),
        (right, merge, True), (merge, root, False), (left, right, False),
        (right, left, False), (unrelated, merge, False), (merge, unrelated, False)]
    before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    for ancestor, descendant, expected in queries:
        assert repo.is_ancestor(ancestor, descendant) is expected
    node_queries(profile, repo.rid, queries)
    assert before == git(stored, "for-each-ref", "--format=%(refname) %(objectname)")


@pytest.mark.parametrize("commit_graph", [False, True])
def test_plain_git_missing_intermediate_fails_with_and_without_commit_graph(tmp_path, commit_graph):
    profile, _, _, repo, _, stored, root = project_fixture(tmp_path)
    tree = git(stored, "rev-parse", f"{root}^{{tree}}")
    middle = git(stored, "commit-tree", tree, "-p", root, "-m", "middle")
    tip = git(stored, "commit-tree", tree, "-p", middle, "-m", "tip")
    git(stored, "update-ref", "refs/heads/graph-query", tip)
    if commit_graph:
        git(stored, "commit-graph", "write", "--reachable")
    assert repo.is_ancestor(root, tip)
    (Path(stored) / "objects" / middle[:2] / middle[2:]).unlink()
    with pytest.raises(rad.GitError):
        repo.is_ancestor(root, tip)
    node_queries(profile, repo.rid, [(root, tip, "GIT")])


def test_plain_git_arguments_missing_endpoints_and_wrong_kinds(tmp_path):
    profile, _, _, repo, _, stored, root = project_fixture(tmp_path)
    blob = git(stored, "rev-parse", "HEAD:hello.txt")
    tree = git(stored, "rev-parse", "HEAD^{tree}")
    git(stored, "tag", "-a", "query-tag", root, "-m", "annotated tag")
    tag = git(stored, "rev-parse", "refs/tags/query-tag")
    missing = "1" * 40
    errors = [(missing, root), (root, missing), (missing, missing),
              (blob, blob), (blob, root), (root, blob), (tree, root), (root, tag)]
    for ancestor, descendant in errors:
        with pytest.raises(rad.GitError):
            repo.is_ancestor(ancestor, descendant)
    invalid = [("bad-oid", root), (root, "bad-oid")]
    for ancestor, descendant in invalid:
        with pytest.raises(ValueError):
            repo.is_ancestor(ancestor, descendant)
    with pytest.raises(TypeError):
        repo.is_ancestor(root, None)
    node_queries(profile, repo.rid, [
        *[(a, d, "GIT") for a, d in errors],
        *[(a, d, "ERR_INVALID_ARG_VALUE") for a, d in invalid],
        (root, None, "ERR_INVALID_ARG_TYPE"),
    ])
