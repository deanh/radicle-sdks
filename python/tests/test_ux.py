"""Public-only Python UX contract tests (also runnable on release installs)."""
import ast
import os
from pathlib import Path

import pytest
import radiclepy as rad
from workflow_smoke import project_fixture, git


class Fspath:
    def __init__(self, path):
        self.path = path

    def __fspath__(self):
        return self.path


@pytest.fixture
def project(tmp_path):
    return project_fixture(tmp_path)


def test_pathlike_inputs(tmp_path):
    for index, convert in enumerate((str, lambda p: p, lambda p: Fspath(str(p)))):
        root = tmp_path / str(index)
        profile = rad.Profile.init(convert(root / "home"), "alice")
        assert rad.Home.load(convert(root / "home")).path == profile.home.path
        assert isinstance(rad.Home(convert(root / "other-home")).path, str)
        storage = rad.Storage.open(convert(root / "storage"), "alice", profile.node_id)
        assert isinstance(storage.path, str)
        policy = rad.PolicyStore.open(convert(root / "policy.db"))
        assert isinstance(policy.path, str)
        work = root / "work"
        work.mkdir()
        git(work, "init", "--initial-branch=main")
        git(work, "commit", "--allow-empty", "-m", "initial")
        rid = rad.init_project(profile.storage, profile.signer(), convert(work),
                               name="test", description="pathlike", default_branch="main")
        result = rad.checkout_project(profile.storage, rid, profile.node_id, convert(root / "checkout"))
        assert isinstance(result, str) and result == str(root / "checkout")


@pytest.mark.parametrize("convert", [lambda p: str(p).encode(), lambda p: Fspath(str(p).encode())])
def test_bytes_paths_rejected(tmp_path, convert):
    path = convert(tmp_path / "rejected")
    profile = rad.Profile.init(tmp_path / "home", "alice")
    for call in (
        lambda: rad.Home(path), lambda: rad.Home.load(path),
        lambda: rad.Profile.init(path, "alice"),
        lambda: rad.Storage.open(path, "alice", profile.node_id),
        lambda: rad.PolicyStore.open(path),
        lambda: rad.init_project(profile.storage, profile.signer(), path,
                                 name="test", description="d", default_branch="main"),
        lambda: rad.checkout_project(profile.storage, "invalid", profile.node_id, path),
    ):
        with pytest.raises(TypeError):
            call()
    assert not (tmp_path / "rejected").exists()


@pytest.mark.skipif(os.name != "posix", reason="Unix non-UTF-8 filesystem paths")
def test_non_utf8_home_paths_are_not_lossy(tmp_path):
    path = os.fsdecode(os.fsencode(tmp_path) + b"/home-\xff")
    home = rad.Home(Path(path))
    assert os.path.isdir(os.fsencode(path))
    with pytest.raises(ValueError, match="not valid UTF-8"):
        _ = home.path


def test_storage_tree_and_lifecycle_sugar(project):
    profile, _, storage, repo, _, _, _ = project
    assert profile.storage.path == storage.path
    assert repo.rid in profile.storage
    with pytest.raises(ValueError):
        "invalid" in storage
    tree = repo.tree(repo.commit(repo.head.oid).tree)
    assert [e.oid for e in tree] == [e.oid for e in tree.entries]
    entries = list(tree)
    entries.clear()
    assert len(list(tree)) == len(tree)
    issue = rad.Issues.open_as(repo, profile.signer()).create("title", "private body")
    raw = rad.Raw.open(repo)
    count = len(raw.history("xyz.radicle.issue", issue.id))
    issue.close("solved")
    assert len(raw.history("xyz.radicle.issue", issue.id)) == count + 1
    assert issue.issue.close_reason == "solved"
    issue.reopen()
    assert issue.issue.state == "open"
    assert len(raw.history("xyz.radicle.issue", issue.id)) == count + 2
    issue.close()
    assert issue.issue.close_reason == "unspecified"
    with pytest.raises(ValueError):
        issue.close("invalid")
    assert len(raw.history("xyz.radicle.issue", issue.id)) == count + 3


def test_keyword_only_migration_and_readonly_errors(project):
    profile, _, storage, repo, work, _, base = project
    patches = rad.Patches.open_as(repo, profile.signer())
    patch = patches.create("t", "d", base=base, head=base)
    assert patch.patch.target == "delegates"
    for legacy in (
        lambda: rad.init_project(storage, profile.signer(), work, "name", "d", "main"),
        lambda: patches.create("t", "d", "delegates", base, base),
        lambda: patches.draft("t", "d", "delegates", base, base),
        lambda: patch.update("d", base, base),
    ):
        with pytest.raises(TypeError, match="positional"):
            legacy()
    issues_ro = rad.Issues.open(repo)
    patches_ro = rad.Patches.open(repo)
    policy_ro = rad.PolicyStore.reader(profile)
    for write in (
        lambda: issues_ro.create("t", "d"), lambda: issues_ro.get_mut(patch.id),
        lambda: patches_ro.create("t", "d", base=base, head=base),
        lambda: patches_ro.draft("t", "d", base=base, head=base),
        lambda: patches_ro.get_mut(patch.id),
        lambda: policy_ro.follow(profile.node_id), lambda: policy_ro.unfollow(profile.node_id),
        lambda: policy_ro.seed(repo.rid), lambda: policy_ro.unseed(repo.rid),
        lambda: policy_ro.set_follow_policy(profile.node_id, "allow"),
        lambda: policy_ro.set_seed_policy(repo.rid, "allow"),
        lambda: policy_ro.unblock_rid(repo.rid), lambda: policy_ro.unblock_nid(profile.node_id),
    ):
        with pytest.raises(rad.ReadOnlyError) as raised:
            write()
        assert isinstance(raised.value, rad.Error)
        assert not isinstance(raised.value, (ValueError, rad.StorageError))


def test_reprs_and_runtime_literals(project):
    profile, _, _, repo, _, stored, base = project
    issue = rad.Issues.open_as(repo, profile.signer()).create("title", "private body")
    patch = rad.Patches.open_as(repo, profile.signer()).draft(
        "title", "private description", target="delegates", base=base, head=base)
    patch.review(patch.patch.latest_revision, "accept", "private summary")
    revision = patch.patch.revisions[0]
    for value in (issue.issue.comments[0], revision, revision.reviews[0]):
        text = repr(value)
        assert text.startswith(type(value).__name__ + "(") and value.id in text
        assert "private" not in text and len(text) < 200
    git(stored, "tag", "lightweight", base)
    policy = profile.policy_store()
    policy.follow(profile.node_id)
    policy.seed(repo.rid, scope="followed")
    values = [issue.issue, patch.patch, revision.reviews[0], repo.identity(),
              repo.sign_refs(profile.signer()), *repo.tree(repo.commit(base).tree),
              *repo.tags(), *policy.follow_policies(), *policy.seed_policies()]
    policy.set_seed_policy(repo.rid, "block")
    values += policy.seed_policies()
    issue.close("solved")
    patch.ready()
    values += [issue.issue, patch.patch]
    # Read the declared Literal members rather than duplicating a second enum.
    stub = Path(__file__).resolve().parents[2] / "python/radiclepy/__init__.pyi"
    classes = {n.name: n for n in ast.parse(stub.read_text()).body if isinstance(n, ast.ClassDef)}
    checks = 0
    for value in values:
        for method in classes[type(value).__name__].body:
            if not isinstance(method, ast.FunctionDef) or method.returns is None:
                continue
            literals = [n for n in ast.walk(method.returns) if isinstance(n, ast.Subscript)
                        and isinstance(n.value, ast.Name) and n.value.id == "Literal"]
            if not literals:
                continue
            allowed = set()
            for literal in literals:
                members = literal.slice.elts if isinstance(literal.slice, ast.Tuple) else [literal.slice]
                allowed.update(ast.literal_eval(member) for member in members)
            if any(isinstance(n, ast.Constant) and n.value is None for n in ast.walk(method.returns)):
                allowed.add(None)
            assert getattr(value, method.name) in allowed
            checks += 1
    assert checks >= 15
