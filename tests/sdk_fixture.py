"""Shared public-API project fixture for cross-SDK and Python workflow tests.

No deterministic Rust test helpers, process-global profile environment changes,
pytest fixtures or imports from the Python-private test subtree are required.
"""
from pathlib import Path
import subprocess

import radiclepy as rad


def git(path, *args):
    result = subprocess.run(
        ["git", "-C", str(path), "-c", "user.name=SDK Test",
         "-c", "user.email=sdk@example.invalid", *args],
        check=True, capture_output=True, text=True,
    )
    return result.stdout.strip()


def signed_refs(stored, node_id):
    """Read persisted sigrefs without calling the SDK's signing operation."""
    tip = git(stored, "rev-parse", f"refs/namespaces/{node_id}/refs/rad/sigrefs")
    refs = dict((name, oid) for oid, name in
                (line.split() for line in git(stored, "show", f"{tip}:refs").splitlines()))
    return tip, refs


def project_fixture(root):
    root = Path(root)
    profile = rad.Profile.init(str(root / "alice"), "alice")
    peer = rad.Profile.init(str(root / "bob"), "bob")
    storage = rad.Storage.from_profile(profile)
    work = root / "work"
    work.mkdir()
    git(work, "init", "--initial-branch=main")
    (work / "hello.txt").write_text("hello\n")
    git(work, "add", "hello.txt")
    git(work, "commit", "-m", "initial")
    base = git(work, "rev-parse", "HEAD")
    rid = rad.init_project(storage, profile.signer(), work, name="sdk-test", description="SDK fixture", default_branch="main")
    stored = Path(storage.path) / rid.removeprefix("rad:")
    return profile, peer, storage, storage.repository(rid), work, stored, base


def add_commit(work, stored, signer, text):
    (work / "hello.txt").write_text(text)
    git(work, "commit", "-am", text.strip())
    head = git(work, "rev-parse", "HEAD")
    # Import real Git commits into the signer's namespace. Generic Git writes
    # are intentionally not part of this SDK's API.
    git(work, "push", str(stored), f"HEAD:refs/namespaces/{signer.node_id}/refs/heads/main")
    return head
