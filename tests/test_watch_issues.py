"""Python observer spike: real public SDK data, CLI and callback failures."""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "examples/watch_issues.py"
spec = importlib.util.spec_from_file_location("watch_issues", SCRIPT)
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


@pytest.fixture
def data(tmp_path):
    profile, peer, _, repo, _, stored, _ = project_fixture(tmp_path / "project")
    return profile, peer, repo, stored, tmp_path / "checkpoint.json"


def cli(profile, rid, state, *extra):
    return subprocess.run(
        [sys.executable, str(SCRIPT), rid, "--state", str(state), "--once", *extra],
        cwd=state.parent,
        env=dict(os.environ, RAD_HOME=profile.home.path),
        capture_output=True,
        text=True,
        timeout=60,
    )


def events(result):
    assert result.returncode == 0, result.stderr
    return [json.loads(line) for line in result.stdout.splitlines()]


def refs(stored):
    return git(stored, "for-each-ref", "--format=%(refname) %(objectname)")


def test_python_observer_cli_baseline_new_issue_comments_restart_and_refs(data):
    profile, peer, repo, stored, state = data
    issues = rad.Issues.open_as(repo, profile.signer())
    old = issues.create("old", "description")
    old.comment("before baseline")
    before = refs(stored)
    assert events(cli(profile, repo.rid, state)) == []
    assert refs(stored) == before
    new = issues.create("new", "not a comment event")
    first = new.comment("peer text: ☃\n\"quoted\"")
    second = old.comment("after baseline")
    before = refs(stored)
    observed = events(cli(profile, repo.rid, state))
    assert {event["id"] for event in observed} == {
        f"{repo.rid}/issue/{new.id}",
        f"{repo.rid}/issue/{new.id}/comment/{first}",
        f"{repo.rid}/issue/{old.id}/comment/{second}",
    }
    assert [e["issueId"] for e in observed] == sorted(e["issueId"] for e in observed)
    assert sum(e["type"] == "issue.discovered" for e in observed) == 1
    assert {e["body"] for e in observed if e["type"] == "comment.observed"} == {
        "peer text: ☃\n\"quoted\"", "after baseline",
    }
    checkpoint = state.read_bytes()
    assert events(cli(profile, repo.rid, state, "--replay")) == []
    assert state.read_bytes() == checkpoint
    assert refs(stored) == before


def test_python_observer_replay_and_failed_callbacks_do_not_advance(data):
    profile, _, repo, _, state = data
    issue = rad.Issues.open_as(repo, profile.signer()).create("existing", "root")
    issue.comment("existing comment")
    partial = []

    def fail(event):
        partial.append(event)
        if len(partial) == 2:
            raise RuntimeError("handler failed")

    with pytest.raises(RuntimeError, match="handler failed"):
        observer.poll_once(profile, repo.rid, state, fail, replay=True)
    assert not state.exists()
    delivered = []
    assert observer.poll_once(profile, repo.rid, state, delivered.append, replay=True) == 2
    assert delivered == partial  # already executed callbacks can repeat
    before = state.read_bytes()
    issue.comment("later")

    async def wrong_handler(event):
        pass

    with pytest.raises(TypeError, match="synchronous"):
        observer.poll_once(profile, repo.rid, state, wrong_handler)
    assert state.read_bytes() == before
    assert observer.poll_once(profile, repo.rid, state, lambda event: None) == 1


def test_python_observer_checkpoint_replace_failure_replays_and_cleans_temp(data, monkeypatch):
    profile, _, repo, _, state = data
    issue = rad.Issues.open_as(repo, profile.signer()).create("t", "root")
    observer.poll_once(profile, repo.rid, state, lambda event: None)
    before = state.read_bytes()
    issue.comment("after")
    delivered = []

    def fail_replace(*args):
        raise OSError("replace failed")

    with monkeypatch.context() as patch:
        patch.setattr(observer.os, "replace", fail_replace)
        with pytest.raises(OSError, match="replace failed"):
            observer.poll_once(profile, repo.rid, state, delivered.append)
    assert state.read_bytes() == before
    assert list(state.parent.glob(f".{state.name}.*")) == []
    again = []
    observer.poll_once(profile, repo.rid, state, again.append)
    assert again == delivered


def test_python_observer_invalid_state_scope_paths_and_interval_fail_closed(data):
    profile, _, repo, _, state = data
    observer.poll_once(profile, repo.rid, state, lambda event: None)
    good = json.loads(state.read_text())
    for change in [
        {"version": 2}, {"version": True}, {"storage": "wrong"}, {"rid": "wrong"},
        {"seen": {"bad-id": []}}, {"seen": {"0" * 40: ["bad-id"]}},
        {"seen": {"0" * 40: ["1" * 40, "1" * 40]}}, {"unexpected": 1},
    ]:
        bad = dict(good, **change)
        state.write_text(json.dumps(bad))
        before = state.read_bytes()
        with pytest.raises((ValueError, TypeError)):
            observer.poll_once(profile, repo.rid, state, lambda event: None)
        assert state.read_bytes() == before
    state.write_text("{bad JSON")
    result = cli(profile, repo.rid, state)
    assert result.returncode == 1 and result.stdout == ""
    assert state.read_text() == "{bad JSON"
    for path in [Path(profile.home.path) / "checkpoint", Path(profile.storage.path) / "checkpoint"]:
        with pytest.raises(ValueError, match="outside"):
            observer.poll_once(profile, repo.rid, path, lambda event: None)
        assert not path.exists()
    target = state.parent / "target"
    target.write_text("do not overwrite")
    state.unlink()
    state.symlink_to(target)
    with pytest.raises(ValueError, match="symlink"):
        observer.poll_once(profile, repo.rid, state, lambda event: None)
    assert target.read_text() == "do not overwrite"
    link = state.parent / "home-link"
    link.symlink_to(profile.home.path, target_is_directory=True)
    with pytest.raises(ValueError, match="outside"):
        observer.poll_once(profile, repo.rid, link / "checkpoint", lambda event: None)
    for interval in ["nan", "inf", "0", "-1", "86401"]:
        result = cli(profile, repo.rid, state, "--interval", interval)
        assert result.returncode == 2 and result.stdout == ""


def test_python_observer_import_has_no_profile_or_loop_side_effects(tmp_path):
    result = subprocess.run(
        [sys.executable, "-c", f"import runpy; runpy.run_path({str(SCRIPT)!r})"],
        cwd=tmp_path,
        env=dict(os.environ, RAD_HOME=str(tmp_path / "absent")),
        text=True,
        capture_output=True,
        timeout=30,
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout == ""
    assert not (tmp_path / "absent").exists()
