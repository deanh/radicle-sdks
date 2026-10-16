"""Cross-SDK ownership/build-entry regressions, not language API tests."""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_workspace_and_language_build_entries():
    workspace = (ROOT / "Cargo.toml").read_text()
    assert '[workspace]' in workspace and '[package]' not in workspace
    assert 'members = ["python/native", "node/native"]' in workspace
    assert 'default-members = ["python/native"]' in workspace
    assert 'resolver = "3"' in workspace
    assert 'radicle = "=0.25.1"' in workspace
    assert 'manifest-path = "python/native/Cargo.toml"' in (ROOT / "pyproject.toml").read_text()
    for language in ("python", "node"):
        manifest = (ROOT / language / "native" / "Cargo.toml").read_text()
        assert 'radicle.workspace = true' in manifest
        assert 'indexmap.workspace = true' in manifest
        assert 'rust-version = "1.91"' in manifest
        assert (ROOT / language / "native" / "src" / "lib.rs").is_file()
        assert not (ROOT / language / "native" / "Cargo.lock").exists()
    assert not (ROOT / "src").exists()
    assert (ROOT / "python" / "tests" / "conftest.py").is_file()
    assert not (ROOT / "tests" / "conftest.py").exists()
    assert (ROOT / ".github" / "workflows" / "python.yml").is_file()
    assert (ROOT / ".github" / "workflows" / "node.yml").is_file()


def test_language_owned_domain_modules():
    for language in ("python", "node"):
        source = ROOT / language / "native" / "src"
        for domain in ("profile", "project", "repository", "issues", "patches", "error", "ids"):
            assert (source / f"{domain}.rs").is_file() or (source / domain / "mod.rs").is_file()
        entry = (source / "lib.rs").read_text()
        assert "pub struct " not in entry and "#[pymethods]" not in entry
        assert "fn materialize_" not in entry and "fn snapshot(" not in entry
    python = ROOT / "python" / "native" / "src"
    for domain in ("repository", "project", "issues", "patches", "policy", "routing"):
        assert (python / domain / "values.rs").is_file()
    assert "#[pyclass" not in (python / "hashing.rs").read_text()
    assert not (python / "values.rs").exists()
    assert (ROOT / "node/native/src/patches/values.rs").is_file()
    assert not (ROOT / "node/native/src/patches.rs").exists()
    assert (python / "raw.rs").is_file()
    for obsolete in ("cob_typed.rs", "cob_values.rs", "storage.rs", "write.rs", "identity.rs"):
        assert not (python / obsolete).exists()
    for binding_only in ("tasks", "locks", "timestamps"):
        assert (ROOT / "node" / "native" / "src" / f"{binding_only}.rs").is_file()


def test_shared_fixture_does_not_import_python_private_tests():
    import sdk_fixture

    assert Path(sdk_fixture.__file__).resolve() == ROOT / "tests" / "sdk_fixture.py"
    source = (ROOT / "tests" / "sdk_fixture.py").read_text()
    assert 'workflow_smoke' not in source and 'from conftest' not in source
    assert 'from sdk_fixture import' in (ROOT / "tests" / "test_examples.py").read_text()
