"""Smoke tests: import surface matches the package contract."""

from __future__ import annotations

import radiclepy as rad


def test_public_surface():
    for name in rad.__all__:
        assert hasattr(rad, name), f"missing public name: {name}"


def test_exception_hierarchy():
    children = [
        rad.StorageError,
        rad.RepositoryError,
        rad.GitError,
        rad.DatabaseError,
        rad.IdentityError,
        rad.SignerError,
        rad.PolicyError,
        rad.CobError,
    ]
    for exc in children:
        assert issubclass(exc, rad.Error)
    assert issubclass(rad.Error, Exception)
    # Siblings are distinct types.
    assert len(set(children)) == len(children)


def test_native_module_is_bundled():
    # Hybrid layout: the compiled module lives inside the package.
    from radiclepy import radiclepy as native  # noqa: F401


def test_metadata_module_is_public_package():
    # Classes and functions must report the public package, not builtins
    # or the compiled submodule.
    for cls_name in ("Profile", "Home", "Aliases", "Signer"):
        cls = getattr(rad, cls_name)
        assert cls.__module__ == "radiclepy", f"{cls_name}.__module__"
    for fn_name in (
        "parse_repo_id", "parse_did", "parse_node_id", "parse_oid", "parse_object_id",
    ):
        fn = getattr(rad, fn_name)
        assert fn.__module__ == "radiclepy", f"{fn_name}.__module__"


def test_identifier_aliases_available_at_runtime():
    # Stubs export them → runtime must provide them (review §1).
    from radiclepy import _types

    for name in ("RepoId", "Did", "NodeId", "Oid", "ObjectId", "Alias"):
        assert getattr(rad, name) is str, f"radiclepy.{name} must be str"
        assert getattr(_types, name) is str, f"radiclepy._types.{name}"


def test_documented_classmethods_are_classmethods():
    import inspect
    import types

    # pyo3 #[classmethod] surfaces as classmethod_descriptor (pure Python
    # would be classmethod); both bind cls — unlike staticmethod.
    kinds = (classmethod, types.ClassMethodDescriptorType)
    for cls_name, names in (("Profile", ("load", "init")), ("Home", ("load", "resolve"))):
        cls = getattr(rad, cls_name)
        for name in names:
            member = inspect.getattr_static(cls, name)
            assert isinstance(member, kinds), f"{cls_name}.{name} is {type(member)}"
            assert not isinstance(member, staticmethod)
