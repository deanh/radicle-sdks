"""Helper-free Python workflows, also executed against installed artifacts.

Uses the explicit shared source fixture in tests/sdk_fixture.py, the installed
SDK, the standard library and Git. No pytest or deterministic Rust helpers.
Git calls use argument arrays, never a shell or the interactive `rad init` CLI.
"""
from pathlib import Path
import sys
import tempfile
import time

import radiclepy as rad


# Direct script execution must find shared support without depending on pytest's
# import mode, a private conftest ancestor, or the caller's working directory.
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from sdk_fixture import add_commit, git, project_fixture, signed_refs


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

    before, _ = signed_refs(stored, signer.node_id)
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
    after, refs = signed_refs(stored, signer.node_id)
    assert after != before  # read only: the COB writes themselves moved sigrefs
    issue_ref = "refs/cobs/xyz.radicle.issue/" + issue.id
    assert refs[issue_ref] == git(stored, "rev-parse", f"refs/namespaces/{signer.node_id}/{issue_ref}")

    head = add_commit(work, stored, signer, "revision one\n")
    patches = rad.Patches.open_as(repo, signer)
    patch = patches.draft("change", "first", base=base, head=head)
    assert patch.patch.base != patch.patch.head
    assert patch.ready() and not patch.ready()
    assert patch.unready() and patch.ready()
    patch.edit("updated title", "delegates")
    patch.assign([peer.did])
    patch.label(["ready"])
    assert patch.archive() and patch.unarchive() and not patch.unarchive()
    newer = add_commit(work, stored, signer, "revision two\n")
    revision = patch.update("second", base=base, head=newer)
    other = rad.Patches.open_as(storage.repository(repo.rid), peer.signer()).get_mut(patch.id)
    comment = other.comment(revision, "looks good")
    other.comment(revision, "reply", comment)
    review = other.review(revision, "accept", "approved")
    refreshed = patch.reload()
    latest = next(r for r in refreshed.revisions if r.id == refreshed.latest_revision)
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
    _, refs = signed_refs(stored, signer.node_id)
    patch_ref = "refs/cobs/xyz.radicle.patch/" + patch.id
    assert refs[patch_ref] == git(stored, "rev-parse", f"refs/namespaces/{signer.node_id}/{patch_ref}")
    assert rad.Raw.open(repo).history("xyz.radicle.patch", patch.id)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="rad-sdk-workflow-") as root:
        run_workflows(root)
    print("core workflow smoke OK")
