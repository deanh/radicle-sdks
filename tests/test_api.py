"""API.md conformance: every documented `radiclepy.<name>` exists, with the
documented kind (method vs attribute vs class vs function).

Extracts dotted names from API.md headings/code spans and checks them
against the live module. Phase 1 scope: top-level names + member names of
the four classes.
"""

from __future__ import annotations

import inspect
import re
from pathlib import Path

import pytest

import radiclepy as rad

API_MD = Path(__file__).resolve().parent.parent / "API.md"

# Documented top-level members and their kinds (mirrors API.md headings).
TOP_LEVEL = {
    "functions": ["parse_repo_id", "parse_did", "parse_node_id", "parse_oid", "parse_object_id"],
    "exceptions": [
        "Error", "StorageError", "RepositoryError", "GitError",
        "DatabaseError", "IdentityError", "SignerError", "PolicyError", "CobError",
    ],
    "classes": ["Profile", "Home", "Aliases", "Signer"],
}

MEMBERS = {
    "Profile": {"attrs": ["did", "node_id", "home", "aliases"], "methods": ["load", "init", "signer"]},
    "Home": {"attrs": ["path"], "methods": ["load", "resolve"]},
    "Aliases": {"methods": ["get", "reverse_lookup"]},
    "Signer": {"attrs": ["did", "node_id"], "methods": []},
}


def _documented_names() -> set[str]:
    text = API_MD.read_text()
    names = set()
    for match in re.finditer(r"`radiclepy\.([A-Za-z_][A-Za-z0-9_.]*)`", text):
        name = match.group(1).split(".")[0]
        if name.startswith("_"):
            continue  # private module references (e.g. radiclepy._types)
        names.add(name)
    return names


def test_api_md_exists_and_names_resolve():
    names = _documented_names()
    assert names, "no `radiclepy.*` names found in API.md"
    for name in names:
        assert hasattr(rad, name), f"API.md documents missing name: radiclepy.{name}"


@pytest.mark.parametrize("kind", ["functions", "exceptions", "classes"])
def test_top_level_kinds(kind):
    for name in TOP_LEVEL[kind]:
        obj = getattr(rad, name)
        if kind == "functions":
            assert callable(obj) and not inspect.isclass(obj)
        elif kind == "exceptions":
            assert inspect.isclass(obj) and issubclass(obj, rad.Error)
        else:
            assert inspect.isclass(obj)


@pytest.mark.parametrize("cls_name", sorted(MEMBERS))
def test_member_kinds(cls_name):
    cls = getattr(rad, cls_name)
    spec = MEMBERS[cls_name]
    for name in spec.get("attrs", []):
        # Attributes are non-callable descriptors on the class (pyo3 getsets
        # or properties) — never methods (plan: kind is binding law).
        member = inspect.getattr_static(cls, name)
        assert not callable(member), f"{cls_name}.{name} must be an attribute"
        assert hasattr(member, "__get__"), f"{cls_name}.{name} not a descriptor"
    for name in spec.get("methods", []):
        assert callable(getattr(cls, name)), f"{cls_name}.{name} must be callable"


def test_no_undocumented_public_names():
    """Public top-level names not in API.md are a contract violation."""
    documented = set().union(*TOP_LEVEL.values()) | {"__all__"}
    runtime = {
        n for n in vars(rad).keys()
        # `radiclepy` = re-exported native module; `annotations` = __future__.
        if not n.startswith("_") and n not in ("radiclepy", "annotations")
    }
    undocumented = runtime - documented
    assert not undocumented, f"undocumented public names: {sorted(undocumented)}"
