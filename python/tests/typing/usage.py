"""Static consumer checks; checked by mypy/pyright, never executed."""
from __future__ import annotations

from os import PathLike
from pathlib import Path
from typing import Literal

import radiclepy as rad


def filesystem(profile: rad.Profile, directory: PathLike[str]) -> str:
    storage = profile.storage
    home = rad.Home(directory)
    rad.Profile.init(directory, "alice")
    rad.Storage.open(directory, "alice", profile.node_id)
    rad.PolicyStore.open(directory)
    rid = rad.init_project(storage, profile.signer(), Path("work"), name="demo",
                           description="Demo", default_branch="main")
    present: bool = rid in storage
    if present:
        repo = storage.repository(rid)
        path: str = rad.checkout_project(storage, rid, profile.node_id, directory)
        rad.fork_project(storage, profile.signer(), rid)
        visibility: Literal["public", "private"] = repo.identity().visibility
        assert visibility and path
    return home.path


def collaboration(repo: rad.Repository, signer: rad.Signer) -> dict[rad.Commit, str]:
    issue = rad.Issues.open_as(repo, signer).create("title", "description")
    issue.close("solved")
    issue.reopen()
    state: Literal["open", "closed"] = issue.issue.state
    assert state
    patches = rad.Patches.open_as(repo, signer)
    patch = patches.draft("title", "description", base=repo.head.oid, head=repo.head.oid)
    revision = patch.update("new description", base=repo.head.oid, head=repo.head.oid)
    patch.review(revision, "accept", "looks good")
    patch.ready()
    comment: rad.Comment = issue.reload().comments[0]
    assert comment.body
    for entry in repo.tree(repo.commit(repo.head.oid).tree):
        kind: Literal["blob", "tree", "commit", "tag", "any"] | None = entry.kind
        assert kind is not None
    commit = repo.commit(repo.head.oid)
    return {commit: "structural snapshot key"}
