"""Release-artifact smoke test (CI: wheel + sdist clean-venv installs).

Run with the INSTALLED artifact's interpreter:

    .venv-smoke/bin/python tests/wheel_smoke.py

Exit code 0 means the artifact is release-clean. Deliberately independent
of pytest and of this repository's test suite (it runs in a venv that has
only the built artifact installed).
"""

import os
import sys
import tempfile
from importlib.metadata import metadata, version as dist_version


def main() -> int:
    import radiclepy as rad

    # -- version integrity -------------------------------------------------
    # The runtime version must match the installed distribution metadata
    # (no hard-coded version in CI). On tag runs, the tag must match too —
    # otherwise a mistagged run could publish mismatched artifacts.
    pkg = dist_version("radicle-py")
    assert rad.__version__ == pkg, f"{rad.__version__} != installed {pkg}"
    tag = os.environ.get("RELEASE_TAG", "")
    if tag:
        assert tag == f"v{pkg}", f"git tag {tag} != packaged version v{pkg}"

    # -- Python-owned package description ----------------------------------
    info = metadata("radicle-py")
    assert info["Description-Content-Type"].startswith("text/markdown")
    description = info.get_payload()
    assert description.startswith("# radicle-py "), "package description is not the Python guide"
    assert "import radiclepy as rad" in description, "Python quick start missing from metadata"

    # -- release purity (plan §5.7) -----------------------------------------
    # Deterministic seeding is compile-time gated and must be absent.
    assert not hasattr(rad.Profile, "_from_seed"), "test helper leaked into release"
    # The _test submodule registers itself in sys.modules at import time
    # when the cargo feature is on — and it is NEVER a package attribute,
    # so `hasattr(rad, "_test")` proves nothing. Check the registration
    # itself and that a fresh import fails in a release build.
    assert "radiclepy._test" not in sys.modules, "test submodule leaked into release"
    try:
        import radiclepy._test  # noqa: F401
    except ModuleNotFoundError:
        pass
    else:
        raise AssertionError("radiclepy._test importable in a release artifact")

    # -- identifier aliases are plain str + validators -----------------------
    assert rad.RepoId is str and rad.Did is str and rad.NodeId is str
    from pathlib import Path
    from workflow_smoke import run_workflows

    # Validate PEP 561 data in the installed artifact, not the source tree.
    package = Path(rad.__file__).parent
    for filename in ("py.typed", "__init__.pyi", "_types.pyi"):
        assert (package / filename).is_file(), f"missing typing file: {filename}"
    with tempfile.TemporaryDirectory(prefix="rad-smoke-home-") as root:
        p = rad.Profile.init(str(Path(root) / "ci"), "ci")
        assert rad.parse_did(p.did) == p.did
        assert p.signer().node_id == p.node_id
        run_workflows(str(Path(root) / "workflow"))

    print(f"wheel smoke OK: radiclepy {rad.__version__}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
