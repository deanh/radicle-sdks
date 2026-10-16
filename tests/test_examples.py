"""examples/: run the read-only digest in both SDKs against one fixture.

The Python output is checked for exact content; the TypeScript port must then
print byte-identical output for the same storage (skipped when the Node build is
absent, e.g. in Python-only environments).
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest
import radiclepy as rad

from workflow_smoke import project_fixture, add_commit

ROOT = Path(__file__).resolve().parent.parent
NODE_SDK = ROOT / "node" / "dist" / "index.js"


@pytest.fixture(scope="module")
def digest_home(tmp_path_factory):
    """One project exercising every digest section."""
    profile, peer, storage, repo, work, stored, base = project_fixture(tmp_path_factory.mktemp("digest"))
    me, bob = profile.signer(), peer.signer()
    mine = rad.Patches.open_as(repo, me)
    theirs = rad.Patches.open_as(repo, bob)

    # Merged: commit on the delegate's target branch, then a merge record.
    merged_head = add_commit(work, stored, me, "merged change\n")
    merged = mine.create("Speed up history paging", "Faster.", base=base, head=merged_head)
    merged.merge(merged.patch.latest_revision, merged_head)

    # A peer's open patch with no review from me: needs my review.
    peer_head = add_commit(work, stored, bob, "peer change\n")
    needs_review = theirs.create("Add Bun notes", "Docs.", base=base, head=peer_head)

    # My open patch, accepted by the peer, assigned to me.
    my_head = add_commit(work, stored, me, "my change\n")
    reviewed = mine.create("Document visibility", "Docs.", base=base, head=my_head)
    reviewed.assign([profile.did])
    theirs.get_mut(reviewed.id).review(reviewed.patch.latest_revision, "accept", "LGTM")

    draft = mine.draft("WIP: release notes", "Draft.", base=base, head=my_head)
    archived = mine.create("Abandoned idea", "Old.", base=base, head=my_head)
    archived.archive()

    issues = rad.Issues.open_as(repo, me)
    bug = issues.create("Crash on empty repo", "Steps...", labels=["bug", "urgent"],
                        assignees=[profile.did])
    bug.comment("Reproduced.")
    bug.comment("Fix in progress.")
    docs = issues.create("Improve docs", "Where?", labels=["docs"])
    question = issues.create("Question about seeds", "How?")
    issues.create("Old report", "Fixed.").lifecycle("closed", "solved")

    short = {name: oid[:7] for name, oid in {
        "merged": merged.id, "merged_head": merged_head, "needs_review": needs_review.id,
        "reviewed": reviewed.id, "draft": draft.id, "archived": archived.id,
        "bug": bug.id, "docs": docs.id, "question": question.id}.items()}
    return {"home": profile.home.path, "me": profile.did, "peer": peer.did[-8:],
            "rid": repo.rid, "id": short}


def run_digest(command, home):
    env = dict(os.environ, RAD_HOME=home)
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stderr
    return result.stdout


def test_python_digest_content(digest_home):
    d, s = digest_home, digest_home["id"]
    out = run_digest([sys.executable, "examples/digest.py"], d["home"])
    assert out.startswith(f"# Radicle digest\n\nProfile: {d['me']}\n\n## sdk-test ({d['rid']})\n\nSDK fixture\n")
    section = lambda name: out.split(f"### {name}\n\n")[1].split("\n\n### ")[0].rstrip("\n")
    # Open and draft only, sorted by title; merged and archived are excluded.
    assert section("Patches in progress").splitlines() == [
        f"- **Add Bun notes** `{s['needs_review']}` (open), by did:key:...{d['peer']}, "
        "1 revision(s); latest revision verdicts: none yet. **Needs your review.**",
        f"- **Document visibility** `{s['reviewed']}` (open), by you, 1 revision(s); "
        f"latest revision verdicts: accept from did:key:...{d['peer']}.",
        f"- **WIP: release notes** `{s['draft']}` (draft), by you, 1 revision(s); "
        "latest revision verdicts: none yet.",
    ]
    assert s["archived"] not in out
    assert section("Merged patches") == (
        f"- **Speed up history paging** `{s['merged']}` merged as `{s['merged_head']}`: merged change")
    # Grouped by label (a multi-label issue appears under each), unlabeled last;
    # closed issues are excluded.
    assert section("Open issues") == "\n".join([
        "#### bug", "",
        f"- **Crash on empty repo** `{s['bug']}`, 2 comment(s), assigned to you", "",
        "#### docs", "",
        f"- **Improve docs** `{s['docs']}`, 0 comment(s)", "",
        "#### urgent", "",
        f"- **Crash on empty repo** `{s['bug']}`, 2 comment(s), assigned to you", "",
        "#### (no label)", "",
        f"- **Question about seeds** `{s['question']}`, 0 comment(s)",
    ])
    assert "Old report" not in out
    assert out.endswith("### Assigned to you\n\n"
                        f"- issue **Crash on empty repo** `{s['bug']}`\n"
                        f"- patch **Document visibility** `{s['reviewed']}`\n")


@pytest.mark.skipif(shutil.which("node") is None or not NODE_SDK.exists(),
                    reason="TypeScript SDK not built (npm run build)")
def test_typescript_digest_matches_python(digest_home):
    python = run_digest([sys.executable, "examples/digest.py"], digest_home["home"])
    typescript = run_digest(["node", "examples/digest.mjs"], digest_home["home"])
    assert typescript == python


def test_digest_with_explicit_rids_and_an_unreadable_repository(digest_home):
    missing = "rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5"  # valid RID, not in this storage
    out = run_digest([sys.executable, "examples/digest.py", digest_home["rid"], missing], digest_home["home"])
    assert out.count("\n## ") == 2
    assert out.endswith(f"## {missing}\n\n_Could not read this repository._\n")
