"""Focused signatures, owned value types and lazy-store contract checks."""
import ast
import inspect
from pathlib import Path
import shutil

import pytest
import radiclepy as rad
from workflow_smoke import project_fixture

STUB = Path(__file__).resolve().parent.parent / "python/radiclepy/__init__.pyi"


def test_alpha_stub_signatures():
    tree = ast.parse(STUB.read_text())
    targets = {"init_project", "fork_project", "checkout_project", "PatchMut"}
    for node in tree.body:
        if not isinstance(node, (ast.FunctionDef, ast.ClassDef)) or node.name not in targets:
            continue
        functions = node.body if isinstance(node, ast.ClassDef) else [node]
        parent = getattr(rad, node.name) if isinstance(node, ast.ClassDef) else rad
        for method in functions:
            if not isinstance(method, ast.FunctionDef) or method.name.startswith("__"):
                continue
            if any(isinstance(d, ast.Name) and d.id == "property" for d in method.decorator_list):
                continue
            signature = inspect.signature(getattr(parent, method.name))
            declared = method.args.posonlyargs + method.args.args + method.args.kwonlyargs
            assert [arg.arg for arg in declared] == list(signature.parameters)
            for arg in method.args.kwonlyargs:
                assert signature.parameters[arg.arg].kind == inspect.Parameter.KEYWORD_ONLY
            for arg, default in zip(method.args.args[-len(method.args.defaults):], method.args.defaults):
                assert signature.parameters[arg.arg].default == ast.literal_eval(default)
            for arg, default in zip(method.args.kwonlyargs, method.args.kw_defaults):
                if default is not None:
                    assert signature.parameters[arg.arg].default == ast.literal_eval(default)
            assert method.returns is not None


def test_lazy_stores_and_value_types(tmp_path):
    profile, _, storage, repo, _, stored, _ = project_fixture(tmp_path)
    issues = rad.Issues.open_as(repo, profile.signer())
    issue = issues.create("t", "d")
    assert isinstance(issue, rad.IssueMut)
    assert isinstance(issue.issue, rad.Issue)
    assert isinstance(issue.issue.comments[0], rad.Comment)
    patches = rad.Patches.open_as(repo, profile.signer())
    patch = patches.create("t", "d", "delegates", repo.head.oid, repo.head.oid)
    rev = patch.patch.revisions[0]
    assert isinstance(patch, rad.PatchMut) and isinstance(rev, rad.Revision)
    patch.review(rev.id, "reject", "reason")
    assert isinstance(patch.patch.revisions[0].reviews[0], rad.Review)
    shutil.rmtree(stored)
    # Construction only retains owned handles: it deliberately does no I/O.
    readers = (rad.Issues.open(repo), rad.Patches.open(repo))
    for reader in readers:
        with pytest.raises(rad.RepositoryError):
            reader.all()
