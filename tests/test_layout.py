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


def test_shared_fixture_does_not_import_python_private_tests():
    import sdk_fixture

    assert Path(sdk_fixture.__file__).resolve() == ROOT / "tests" / "sdk_fixture.py"
    source = (ROOT / "tests" / "sdk_fixture.py").read_text()
    assert 'workflow_smoke' not in source and 'from conftest' not in source
    assert 'from sdk_fixture import' in (ROOT / "tests" / "test_examples.py").read_text()
