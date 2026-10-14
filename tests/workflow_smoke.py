"""Helper-free core workflows, also executed against installed artifacts.

Only the standard library, the installed SDK and the Git executable are needed.
Git calls use argument arrays, never a shell or the interactive `rad init` CLI.
"""
from pathlib import Path
import subprocess
import tempfile
import time

import radiclepy as rad


def git(path, *args):
    result = subprocess.run(
        ["git", "-C", str(path), "-c", "user.name=SDK Test",
         "-c", "user.email=sdk@example.invalid", *args],
        check=True, capture_output=True, text=True,
    )
    return result.stdout.strip()


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
    rid = rad.init_project(storage, profile.signer(), str(work), "sdk-test", "SDK fixture", "main")
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


def run_workflows(root):
    profile, peer, storage, repo, work, stored, base = project_fixture(root)
    signer = profile.signer()
    assert storage.contains(repo.rid)
    assert repo.identity().default_branch == "main"
    assert repo.commit(base).message_str == "initial\n"
    tree = repo.tree(repo.commit(base).tree)
    assert repo.blob(tree.entries[0].oid) == b"hello\n"
    rad.fork_project(storage, peer.signer(), repo.rid)
    checkout = Path(root) / "checkout"
    assert rad.checkout_project(storage, repo.rid, peer.node_id, str(checkout)) == str(checkout)
    assert git(checkout, "rev-parse", "HEAD") == base
    assert (checkout / "hello.txt").read_text() == "hello\n"

    before = repo.sign_refs(signer).at
    issues = rad.Issues.open_as(repo, signer)
    issue = issues.create("hello", "root", labels=["bug"])
    first = issue.comment("first")
    issue.comment("reply", first)
    issue.assign([peer.did])
    issue.label(["urgent"])
    issue.lifecycle("closed", "solved")
    issue.lifecycle("open")
    snapshot = rad.Issues.open(storage.repository(repo.rid)).get(issue.id)
    assert [c.body for c in snapshot.comments] == ["root", "first", "reply"]
    assert snapshot.comments[-1].reply_to == first
    assert snapshot.comments[0].author == profile.did
    assert abs(snapshot.comments[0].timestamp - int(time.time() * 1000)) < 60_000
    assert snapshot.assignees == [peer.did] and snapshot.labels == ["urgent"]
    assert snapshot.state == "open"
    assert repo.sign_refs(signer).at != before  # upstream COB writes sign refs

    head = add_commit(work, stored, signer, "revision one\n")
    patches = rad.Patches.open_as(repo, signer)
    patch = patches.draft("change", "first", "delegates", base, head)
    assert patch.patch.base != patch.patch.head
    assert patch.ready() and not patch.ready()
    assert patch.unready() and patch.ready()
    patch.edit("updated title", "delegates")
    patch.assign([peer.did])
    patch.label(["ready"])
    assert patch.archive() and patch.unarchive() and not patch.unarchive()
    newer = add_commit(work, stored, signer, "revision two\n")
    revision = patch.update("second", base, newer)
    other = rad.Patches.open_as(storage.repository(repo.rid), peer.signer()).get_mut(patch.id)
    comment = other.comment(revision, "looks good")
    other.comment(revision, "reply", comment)
    review = other.review(revision, "accept", "approved")
    refreshed = patch.reload()
    latest = refreshed.revisions[-1]
    assert latest.id == revision and latest.head == newer
    assert [c.body for c in latest.comments] == ["looks good", "reply"]
    assert latest.comments[-1].reply_to == comment
    assert latest.reviews[0].id == review
    assert latest.reviews[0].author == peer.did and latest.reviews[0].verdict == "accept"
    assert refreshed.assignees == [peer.did] and refreshed.labels == ["ready"]
    # Upstream merge evaluation requires the delegate's target branch to
    # contain the supplied commit. Git has already moved that namespace.
    patch.merge(revision, newer)
    final = rad.Patches.open(storage.repository(repo.rid)).get(patch.id)
    assert final.state == "merged"
    assert final.merged_revision == revision and final.merged_commit == newer
    assert final.revision_count == 2
    assert "refs/cobs/xyz.radicle.patch/" + patch.id in repo.sign_refs(signer).refs()
    assert rad.Raw.open(repo).history("xyz.radicle.patch", patch.id)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="rad-sdk-workflow-") as root:
        run_workflows(root)
    print("core workflow smoke OK")
