"""Observer spikes agree on events and can hand off one versioned checkpoint."""

import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent
PYTHON = ROOT / "examples/watch_issues.py"
NODE = ROOT / "examples/watch-issues.mjs"


def run(command, profile, rid, state, *options):
    result = subprocess.run(
        [*command, rid, "--state", str(state), "--once", *options],
        cwd=state.parent,
        env=dict(os.environ, RAD_HOME=profile.home.path),
        text=True,
        capture_output=True,
        timeout=60,
    )
    assert result.returncode == 0, result.stderr
    return [json.loads(line) for line in result.stdout.splitlines()]


def test_observer_events_and_cross_language_checkpoint_handoff(tmp_path):
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK unavailable")
        pytest.skip("Node SDK unavailable")
    profile, peer, _, repo, _, stored, _ = project_fixture(tmp_path / "project")
    issues = rad.Issues.open_as(repo, profile.signer())
    issue = issues.create("Unicode 😀 Ａ", "root")
    rad.Issues.open_as(repo, peer.signer()).get_mut(issue.id).comment("peer ☃")
    py, js = [sys.executable, str(PYTHON)], ["node", str(NODE)]
    py_state, js_state = tmp_path / "python.json", tmp_path / "node.json"
    before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    assert run(py, profile, repo.rid, py_state, "--replay") == run(
        js, profile, repo.rid, js_state, "--replay"
    )
    assert json.loads(py_state.read_text()) == json.loads(js_state.read_text())
    assert run(js, profile, repo.rid, py_state) == []
    assert run(py, profile, repo.rid, js_state) == []
    assert git(stored, "for-each-ref", "--format=%(refname) %(objectname)") == before
    comment = issue.comment("after Python checkpoint")
    seen = run(js, profile, repo.rid, py_state)
    assert [event["commentId"] for event in seen] == [comment]
    assert run(py, profile, repo.rid, py_state) == []
    fresh = issues.create("after Node checkpoint", "root")
    assert [event["issueId"] for event in run(py, profile, repo.rid, py_state)] == [fresh.id]
    assert run(js, profile, repo.rid, py_state) == []
    # A backend read failure cannot acknowledge or emit even a partial batch.
    saved = py_state.read_bytes()
    moved = stored.with_name(stored.name + "-moved")
    stored.rename(moved)
    try:
        for command in [py, js]:
            result = subprocess.run(
                [*command, repo.rid, "--state", str(py_state), "--once"],
                cwd=tmp_path, env=dict(os.environ, RAD_HOME=profile.home.path),
                text=True, capture_output=True, timeout=60,
            )
            assert result.returncode != 0 and result.stdout == ""
            assert py_state.read_bytes() == saved
            assert not stored.exists()
    finally:
        moved.rename(stored)

    # Exercise the actual polling loops and documented stop behavior, not just
    # one-shot scans. Checkpoint visibility is the synchronization barrier.
    for index, command in enumerate([py, js]):
        state = tmp_path / f"live-{index}.json"
        process = subprocess.Popen(
            [*command, repo.rid, "--state", str(state), "--interval", "0.02"],
            cwd=tmp_path, env=dict(os.environ, RAD_HOME=profile.home.path),
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        try:
            deadline = time.monotonic() + 30
            while not state.exists():
                assert process.poll() is None
                assert time.monotonic() < deadline
                time.sleep(0.02)
            live_issue = issues.create(f"live-{index}", "root")
            while live_issue.id not in json.loads(state.read_text())["seen"]:
                assert process.poll() is None
                assert time.monotonic() < deadline
                time.sleep(0.02)
            process.send_signal(signal.SIGINT if index == 0 else signal.SIGTERM)
            stdout, stderr = process.communicate(timeout=15)
            assert process.returncode == (130 if index == 0 else 0), stderr
            assert any(json.loads(line)["issueId"] == live_issue.id
                       for line in stdout.splitlines())
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=15)
