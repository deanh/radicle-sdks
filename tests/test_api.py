"""API.md conformance: every documented `radiclepy.<name>` exists, with the
documented kind (method vs attribute vs class vs function).

Extracts dotted names from API.md headings/code spans and checks them
against the live module. Covers top-level names and the member kinds of
every class (Phase 1 + Phase 2 surfaces).
"""

from __future__ import annotations

import inspect
import re
import types
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
    "classes": [
        "Profile",
        "Home",
        "Aliases",
        "Signer",
        "Storage",
        "Repository",
        "HeadInfo",
        "Commit",
        "Tree",
        "TreeEntry",
        "TagInfo",
        "RemoteInfo",
        "ProjectDoc",
        "HistoryPage",
        "PolicyStore",
        "RoutingStore",
        "FollowPolicyInfo",
        "SeedPolicyInfo",
        "RoutingEntry",
        "Issues",
        "Issue",
        "Patches",
        "Patch",
        "Raw",
        "RawOp",
        "CobRef",
        "CobManifest",
        "SetHead",
        "SignedRefs",
        "IssueMut",
        "PatchMut",
    ],
}

MEMBERS = {
    "Profile": {
        "attrs": ["did", "node_id", "home", "aliases"],
        "methods": ["signer", "policy_store", "routing_store"],
        "classmethods": ["load", "init"],
    },
    "Home": {"attrs": ["path"], "methods": [], "classmethods": ["load", "resolve"]},
    "Aliases": {"methods": ["get", "reverse_lookup"], "classmethods": []},
    "Signer": {"attrs": ["did", "node_id"], "methods": [], "classmethods": []},
    "Storage": {
        "attrs": ["path"],
        "methods": ["repository", "contains", "repositories"],
        "classmethods": ["from_profile", "open"],
    },
    "Repository": {
        "attrs": ["rid", "head"],
        "methods": [
            "identity",
            "description",
            "branches",
            "branch_head",
            "tags",
            "rev_parse",
            "commit",
            "tree",
            "blob",
            "history",
            "remotes",
            "set_default_branch_to_canonical_head",
            "set_identity_head",
            "set_remote_identity_root",
            "sign_refs",
        ],
        "classmethods": [],
    },
    "SetHead": {
        "attrs": ["old", "new"],
        "methods": ["is_updated"],
        "classmethods": [],
    },
    "SignedRefs": {
        "attrs": ["node_id", "at", "parent", "feature_level"],
        "methods": ["refs"],
        "classmethods": [],
    },
    "HeadInfo": {"attrs": ["name", "oid"], "methods": [], "classmethods": []},
    "Commit": {
        "attrs": [
            "oid",
            "tree",
            "parents",
            "author_name",
            "author_name_str",
            "author_email",
            "author_email_str",
            "author_time",
            "author_offset_min",
            "committer_name",
            "committer_name_str",
            "committer_email",
            "committer_email_str",
            "committer_time",
            "committer_offset_min",
            "message",
            "message_str",
            "encoding",
        ],
        "methods": [],
        "classmethods": [],
    },
    "Tree": {"attrs": ["oid", "entries"], "methods": [], "classmethods": []},
    "TreeEntry": {
        "attrs": ["name", "name_str", "kind", "mode", "oid"],
        "methods": [],
        "classmethods": [],
    },
    "TagInfo": {
        "attrs": ["name", "target", "kind", "peeled", "peeled_kind"],
        "methods": [],
        "classmethods": [],
    },
    "RemoteInfo": {"attrs": ["fetcher", "heads"], "methods": [], "classmethods": []},
    "ProjectDoc": {
        "attrs": [
            "name",
            "description",
            "default_branch",
            "version",
            "delegates",
            "threshold",
            "visibility",
            "commit",
            "blob",
        ],
        "methods": [],
        "classmethods": [],
    },
    "HistoryPage": {
        "attrs": ["name", "head", "oids", "offset", "has_more"],
        "methods": [],
        "classmethods": [],
    },
    "PolicyStore": {
        "attrs": ["path"],
        "methods": [
            "follow",
            "unfollow",
            "seed",
            "unseed",
            "set_follow_policy",
            "set_seed_policy",
            "unblock_rid",
            "unblock_nid",
            "is_following",
            "is_seeding",
            "is_blocked",
            "follow_policy",
            "seed_policy",
            "follow_policies",
            "seed_policies",
            "nodes_by_alias",
        ],
        "classmethods": ["from_profile", "reader", "open"],
    },
    "RoutingStore": {
        "methods": ["get", "get_inventory", "entry", "entries", "count", "total", "is_empty"],
        "classmethods": ["from_profile"],
    },
    "FollowPolicyInfo": {
        "attrs": ["node_id", "alias", "policy"],
        "methods": [],
        "classmethods": [],
    },
    "SeedPolicyInfo": {
        "attrs": ["rid", "policy", "scope"],
        "methods": [],
        "classmethods": [],
    },
    "RoutingEntry": {
        "attrs": ["rid", "node_id", "timestamp"],
        "methods": [],
        "classmethods": [],
    },
    "Issues": {
        "attrs": ["rid"],
        "methods": ["all", "get", "count", "is_empty", "create", "get_mut"],
        "classmethods": ["open", "open_as"],
    },
    "IssueMut": {
        "attrs": ["id", "issue"],
        "methods": [
            "reload", "comment", "edit", "edit_description", "label",
            "assign", "lifecycle",
        ],
        "classmethods": [],
    },
    "Patches": {
        "attrs": ["rid"],
        "methods": ["all", "get", "count", "is_empty", "create", "draft"],
        "classmethods": ["open", "open_as"],
    },
    "PatchMut": {
        "attrs": ["id", "patch"],
        "methods": [],
        "classmethods": [],
    },
    "Raw": {
        "attrs": ["rid"],
        "methods": ["typenames", "entities", "refs", "history"],
        "classmethods": ["open"],
    },
    "Issue": {
        "attrs": [
            "id", "title", "description", "author", "state", "close_reason",
            "assignees", "labels", "comment_count", "comment_ids",
        ],
        "methods": [],
        "classmethods": [],
    },
    "Patch": {
        "attrs": [
            "id", "title", "description", "author", "state", "target",
            "base", "head", "version", "labels", "revision_count",
        ],
        "methods": [],
        "classmethods": [],
    },
    "RawOp": {
        "attrs": [
            "id", "actor", "timestamp", "actions", "parents", "related",
            "identity", "manifest",
        ],
        "methods": ["actions_json"],
        "classmethods": [],
    },
    "CobManifest": {"attrs": ["type_name", "version"], "methods": [], "classmethods": []},
    "CobRef": {"attrs": ["name", "target"], "methods": [], "classmethods": []},
}

#: Identifier type aliases — public, runtime `str`, precise in _types.pyi.
IDENTIFIER_ALIASES = ["RepoId", "Did", "NodeId", "Oid", "ObjectId", "Alias"]


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
        # Instance methods: callable off the class, and NOT disguised as
        # static/class methods (kind distinctions are binding law).
        member = inspect.getattr_static(cls, name)
        assert not isinstance(member, (staticmethod, classmethod, property))
        assert callable(member), f"{cls_name}.{name}: {type(member)}"
    for name in spec.get("classmethods", []):
        member = inspect.getattr_static(cls, name)
        # pyo3 exposes #[classmethod] as classmethod_descriptor; accept it
        # alongside pure-Python classmethod. Never staticmethod.
        kinds = (classmethod, types.ClassMethodDescriptorType)
        assert isinstance(member, kinds), f"{cls_name}.{name}: {type(member)}"


def test_identifier_aliases_are_str():
    for name in IDENTIFIER_ALIASES:
        assert getattr(rad, name) is str, name


def test_no_undocumented_public_names():
    """Public top-level names not in API.md are a contract violation."""
    documented = set().union(*TOP_LEVEL.values()) | set(IDENTIFIER_ALIASES) | {"__all__"}
    runtime = {
        n for n in vars(rad).keys()
        # `radiclepy` = re-exported native module; `annotations` = __future__.
        if not n.startswith("_") and n not in ("radiclepy", "annotations")
    }
    undocumented = runtime - documented
    assert not undocumented, f"undocumented public names: {sorted(undocumented)}"


def test_stub_runtime_all_parity():
    """`__init__.pyi.__all__` must exactly match the runtime `__all__`."""
    import ast

    stub = API_MD.parent / "python" / "radiclepy" / "__init__.pyi"
    tree = ast.parse(stub.read_text())
    stub_all = None
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(t, ast.Name) and t.id == "__all__" for t in node.targets
        ):
            stub_all = {
                elt.value for elt in node.value.elts  # type: ignore[union-attr]
            }
    assert stub_all is not None, "no __all__ found in __init__.pyi"
    assert stub_all == set(rad.__all__)


def test_stub_classes_exist():
    """Every class declared in the stub exists at runtime (kind: class)."""
    import ast

    stub = API_MD.parent / "python" / "radiclepy" / "__init__.pyi"
    tree = ast.parse(stub.read_text())
    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            assert hasattr(rad, node.name), f"stub class missing at runtime: {node.name}"
            assert inspect.isclass(getattr(rad, node.name))
