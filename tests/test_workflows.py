"""Release-surface integration tests: no deterministic Rust test helpers."""
import subprocess

import pytest
import radiclepy as rad

from workflow_smoke import project_fixture, run_workflows, git, add_commit


@pytest.fixture
def project(tmp_path):
    return project_fixture(tmp_path)


def test_core_workflows(tmp_path):
    run_workflows(tmp_path)


def test_local_project_failures(project, tmp_path, monkeypatch):
    profile, peer, storage, repo, work, stored, base = project
    with pytest.raises(ValueError):
        rad.init_project(storage, profile.signer(), str(work), name="bad/name", description="d", default_branch="main")
    for branch in ("", "bad..branch", "refs/heads/main"):
        with pytest.raises(ValueError):
            rad.init_project(storage, profile.signer(), str(work), name="ok", description="d", default_branch=branch)
    with pytest.raises(ValueError):
        rad.init_project(storage, profile.signer(), str(work), name="ok", description="d", default_branch="main", visibility="invalid")
    config_before = git(work, "config", "--local", "--list")
    refs_before = git(work, "for-each-ref", "refs/remotes/rad/")
    assert refs_before
    for description in ("d", "corrected description"):
        with pytest.raises(rad.Error, match="already has a 'rad' remote"):
            rad.init_project(storage, profile.signer(), str(work), name="ok", description=description, default_branch="main")
        assert git(work, "remote") == "rad"
        assert git(work, "config", "--local", "--list") == config_before
        assert git(work, "for-each-ref", "refs/remotes/rad/") == refs_before
        assert storage.repositories() == [repo.rid]
    with pytest.raises(rad.Error):
        rad.checkout_project(storage, repo.rid, profile.node_id, str(work))
    with pytest.raises(rad.Error):
        rad.fork_project(rad.Storage.from_profile(peer), peer.signer(), repo.rid)
    with pytest.raises(rad.Error):
        rad.checkout_project(rad.Storage.from_profile(peer), repo.rid, peer.node_id, str(tmp_path / "missing"))
    with pytest.raises(ValueError):
        rad.checkout_project(storage, repo.rid, "invalid", str(tmp_path / "invalid"))
    bare = tmp_path / "bare"
    rad.checkout_project(storage, repo.rid, profile.node_id, str(bare), bare=True)
    assert git(bare, "rev-parse", "--is-bare-repository") == "true"
    assert git(bare, "rev-parse", "HEAD") == base
    # Missing Git executable is an operational failure, never a prompt.
    fresh = tmp_path / "fresh"
    fresh.mkdir()
    git(fresh, "init", "--initial-branch=main")
    git(fresh, "commit", "--allow-empty", "-m", "initial")
    with pytest.raises(rad.GitError):
        rad.init_project(storage, profile.signer(), str(fresh), name="ok", description="d", default_branch="absent")
    assert git(fresh, "remote") == ""
    with monkeypatch.context() as env:
        env.setenv("PATH", "")
        with pytest.raises(rad.Error):
            rad.checkout_project(storage, repo.rid, profile.node_id, str(tmp_path / "no-git"))
        with pytest.raises(rad.Error):
            rad.init_project(storage, profile.signer(), str(fresh), name="no-git", description="d", default_branch="main")
    assert git(fresh, "remote") == ""
    assert storage.repositories() == [repo.rid]


def test_private_project(tmp_path):
    profile = rad.Profile.init(str(tmp_path / "p"), "alice")
    storage = rad.Storage.from_profile(profile)
    work = tmp_path / "work"
    work.mkdir()
    git(work, "init", "--initial-branch=main")
    git(work, "commit", "--allow-empty", "-m", "initial")
    rid = rad.init_project(storage, profile.signer(), str(work), name="private", description="d", default_branch="main", visibility="private")
    assert storage.repository(rid).identity().visibility == "private"


def test_patch_validation_and_authorization(project):
    profile, peer, storage, repo, work, stored, base = project
    head = add_commit(work, stored, profile.signer(), "patch\n")
    writer = rad.Patches.open_as(repo, profile.signer())
    patch = writer.create("title", "desc", base=base, head=head)
    revision = patch.patch.latest_revision
    with pytest.raises(rad.ReadOnlyError, match="read-only"):
        rad.Patches.open(repo).get_mut(patch.id)
    with pytest.raises(rad.CobError):
        writer.get_mut("0" * 40)
    with pytest.raises(ValueError):
        writer.get_mut("invalid")
    with pytest.raises(ValueError):
        patch.review(revision, "invalid")
    with pytest.raises(rad.CobError):
        patch.review(revision)
    with pytest.raises(ValueError):
        patch.comment("invalid", "x")
    with pytest.raises(ValueError):
        patch.update("x", base="invalid", head=head)
    with pytest.raises(rad.CobError):
        patch.update("x", base=base, head="0" * 40)
    with pytest.raises(rad.CobError):
        patch.comment("0" * 40, "x")
    with pytest.raises(rad.CobError):
        patch.merge(revision, "0" * 40)
    with pytest.raises(ValueError):
        patch.assign(["not-a-did"])
    with pytest.raises(ValueError):
        patch.label(["INVALID label"])
    with pytest.raises(ValueError):
        patch.edit("", "delegates")
    other = rad.Patches.open_as(repo, peer.signer()).get_mut(patch.id)
    for mutation in (lambda: other.assign([peer.did]), lambda: other.edit("unauthorized", "delegates"),
                     lambda: other.merge(revision, head)):
        with pytest.raises(rad.CobError):
            mutation()
    assert patch.reload().title == "title"
    # Import a real commit without moving the delegate's target branch.
    # Recording this merge succeeds, but upstream evaluation ignores it.
    git(work, "commit", "--allow-empty", "-m", "off-target")
    off_target = git(work, "rev-parse", "HEAD")
    git(work, "push", str(stored), f"HEAD:refs/namespaces/{profile.node_id}/refs/heads/feature")
    patch.merge(revision, off_target)
    assert patch.reload().state == "open"
    assert patch.patch.merged_commit is None
    patch.merge(revision, head)
    assert patch.reload().merged_commit == head


def test_snapshots_and_clearing(project):
    profile, peer, storage, repo, work, stored, base = project
    issues = rad.Issues.open_as(repo, profile.signer())
    issue = issues.create("t", "root")
    snapshot = issue.issue
    issue.comment("new")
    assert len(snapshot.comments) == 1 and len(issue.reload().comments) == 2
    with pytest.raises(AttributeError):
        snapshot.comments[0].body = "changed"
    comments = snapshot.comments
    comments.clear()
    assert len(snapshot.comments) == 1
    patches = rad.Patches.open_as(repo, profile.signer())
    patch = patches.create("t", "d", base=base, head=base)
    patch.assign([peer.did])
    patch.assign([])
    patch.label(["bug"])
    patch.label([])
    assert patch.reload().labels == [] and patch.patch.assignees == []
    with pytest.raises(AttributeError):
        patch.patch.revisions[0].head = "changed"
    # All actors' revisions are exposed, while upstream summary/latest()
    # intentionally follows the patch author's revisions only.
    author_revision = patch.patch.latest_revision
    peer_head = add_commit(work, stored, peer.signer(), "peer changes\n")
    peer_patch = rad.Patches.open_as(repo, peer.signer()).get_mut(patch.id)
    peer_revision = peer_patch.update("peer revision", base=base, head=peer_head)
    refreshed = patch.reload()
    assert refreshed.revisions[-1].id == peer_revision
    assert refreshed.revisions[-1].author == peer.did
    assert refreshed.description == "d"
    assert refreshed.latest_revision == author_revision != peer_revision
    latest = next(r for r in refreshed.revisions if r.id == refreshed.latest_revision)
    assert (latest.base, latest.head, latest.description) == (refreshed.base, refreshed.head, refreshed.description)
    assert refreshed.version == 1  # upstream counts both authors' slots
    assert refreshed.revisions[refreshed.version].id != refreshed.latest_revision
    peer_patch.review(refreshed.latest_revision, "accept", "author's revision")
    latest = next(r for r in patch.reload().revisions if r.id == author_revision)
    assert latest.reviews[0].summary == "author's revision"


def test_branch_prefix_preserved(project):
    _, _, _, repo, _, stored, base = project
    git(stored, "update-ref", "refs/heads/refs/heads/nested", base)
    assert "refs/heads/nested" in repo.branches()
    assert "nested" not in repo.branches()
    assert repo.branch_head("refs/heads/nested") == base


def test_history_reports_errors_in_skipped_ancestry(project):
    # Issue 8f877ad: a walk error inside the skipped range (offset > 0) used to
    # be discarded, returning an empty page with has_more=False.
    _, _, _, repo, _, stored, base = project
    tree = repo.commit(base).tree
    body = (f"tree {tree}\nparent {'1' * 40}\n"
            "author A <a@example.invalid> 1700000000 +0000\n"
            "committer A <a@example.invalid> 1700000000 +0000\n\nbroken parent\n")
    # Raw object with a missing parent; no refs move.
    broken = subprocess.run(
        ["git", "-C", str(stored), "hash-object", "--literally", "-t", "commit", "-w", "--stdin"],
        input=body, capture_output=True, text=True, check=True,
    ).stdout.strip()
    for offset in (0, 1, 10):
        with pytest.raises(rad.GitError):
            repo.history(broken, offset=offset)
    # Valid ancestry is unaffected, including offsets past the end.
    assert repo.history(base).oids == [base]
    page = repo.history(base, offset=1)
    assert page.oids == [] and page.has_more is False


def test_checkout_uses_canonical_not_remote_head(project, tmp_path):
    _, peer, storage, repo, work, stored, base = project
    rad.fork_project(storage, peer.signer(), repo.rid)
    peer_head = add_commit(work, stored, peer.signer(), "peer branch\n")
    repo.sign_refs(peer.signer())  # sign the external Git import, not a COB write
    destination = tmp_path / "canonical-checkout"
    rad.checkout_project(storage, repo.rid, peer.node_id, str(destination))
    assert git(destination, "rev-parse", "HEAD") == base != peer_head
    assert (destination / "hello.txt").read_text() == "hello\n"
    assert git(destination, "remote", "get-url", "--push", "rad").endswith("/" + peer.node_id)


def test_encrypted_signer(tmp_path, monkeypatch):
    monkeypatch.delenv("SSH_AUTH_SOCK", raising=False)
    monkeypatch.delenv("RAD_PASSPHRASE", raising=False)
    profile = rad.Profile.init(str(tmp_path / "encrypted"), "alice", "correct")
    with pytest.raises(rad.SignerError):
        profile.signer()
    monkeypatch.setenv("RAD_PASSPHRASE", "wrong")
    with pytest.raises(rad.SignerError):
        profile.signer()
    monkeypatch.setenv("RAD_PASSPHRASE", "correct")
    assert profile.signer().did == profile.did
