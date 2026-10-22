"""The notices generator keeps compound licence terms, includes every licence file,
separates runtime from build-time crates, uses reviewed overrides, and refuses
to claim coverage for a shipped crate that has no licence text."""
import importlib.util
import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("third_party_notices", ROOT / "scripts/release/third_party_notices.py")
notices = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = notices  # dataclasses resolve annotations through sys.modules
spec.loader.exec_module(notices)


@pytest.mark.parametrize("expression, used", [
    ("MIT OR Apache-2.0", "MIT"),
    ("Apache-2.0 OR MIT", "MIT"),
    ("MIT/Apache-2.0", "MIT"),
    ("(MIT OR Apache-2.0) AND Unicode-3.0", "MIT AND Unicode-3.0"),
    ("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT", "MIT"),
    ("Apache-2.0 WITH LLVM-exception", "Apache-2.0 WITH LLVM-exception"),
    ("MIT OR Apache-2.0 OR LGPL-2.1-or-later", "MIT"),
    ("BSD-3-Clause", "BSD-3-Clause"),
    ("GPL-3.0 AND MIT", "GPL-3.0 AND MIT"),
])
def test_used_license_keeps_and_terms_and_picks_or_alternatives(expression, used):
    assert notices.used_license(expression) == used


def crate(tmp_path, name, version, license, files=None, proc_macro=False, repository="https://example.invalid"):
    directory = tmp_path / "registry" / f"{name}-{version}"
    directory.mkdir(parents=True)
    (directory / "Cargo.toml").write_text("[package]\n")
    for filename, text in (files or {}).items():
        (directory / filename).write_text(text)
    return {
        "id": f"{name} {version}", "name": name, "version": version, "license": license,
        "repository": repository, "source": "registry+https://crates.io",
        "manifest_path": str(directory / "Cargo.toml"),
        "targets": [{"kind": ["proc-macro" if proc_macro else "lib"]}],
    }


def meta(packages, deps):
    """deps: {pkg_id: [(dep_id, kind)]} with kind None (normal), 'build' or 'dev'."""
    return {
        "packages": packages,
        "resolve": {"nodes": [
            {"id": p["id"], "deps": [{"pkg": d, "dep_kinds": [{"kind": k}]} for d, k in deps.get(p["id"], [])]}
            for p in packages
        ]},
    }


def roots(tmp_path):
    py = {"id": "radicle-py 0.1.0", "name": "radicle-py", "version": "0.1.0", "license": None, "repository": None,
          "source": None, "manifest_path": str(tmp_path / "py" / "Cargo.toml"), "targets": [{"kind": ["cdylib"]}]}
    node = dict(py, id="radicle-sdk-native 0.1.0", name="radicle-sdk-native")
    return py, node


def test_collect_classifies_runtime_and_build_time_and_includes_every_text(tmp_path):
    py, node = roots(tmp_path)
    unicode = crate(tmp_path, "unicode-ident", "1.0.27", "(MIT OR Apache-2.0) AND Unicode-3.0",
                    {"LICENSE-MIT": "mit text", "LICENSE-APACHE": "apache text", "LICENSE-UNICODE": "unicode text"})
    macro = crate(tmp_path, "some-derive", "1.0.0", "MIT", {"LICENSE": "derive mit"}, proc_macro=True)
    macro_dep = crate(tmp_path, "syn-like", "2.0.0", "MIT", {"LICENSE-MIT": "syn mit"})
    build_dep = crate(tmp_path, "cc-like", "1.0.0", "MIT", {"LICENSE-MIT": "cc mit"})
    runtime = crate(tmp_path, "shipped", "0.3.0", "Apache-2.0", {"LICENSE": "shipped apache"})
    dev_only = crate(tmp_path, "test-only", "0.1.0", "MIT", {"LICENSE": "dev mit"})
    m = meta([py, node, unicode, macro, macro_dep, build_dep, runtime, dev_only], {
        py["id"]: [(runtime["id"], None), (macro["id"], None), (build_dep["id"], "build"), (dev_only["id"], "dev")],
        node["id"]: [(unicode["id"], None)],
        macro["id"]: [(macro_dep["id"], None)],
    })
    components = notices.collect(m, overrides=tmp_path / "no-overrides")
    by_name = {c.name: c for c in components}
    assert set(by_name) == {"unicode-ident", "some-derive", "syn-like", "cc-like", "shipped"}
    assert by_name["unicode-ident"].runtime and by_name["shipped"].runtime
    assert not by_name["some-derive"].runtime and not by_name["syn-like"].runtime and not by_name["cc-like"].runtime
    assert by_name["unicode-ident"].used == "MIT AND Unicode-3.0"
    assert [f for f, _ in by_name["unicode-ident"].texts] == ["LICENSE-APACHE", "LICENSE-MIT", "LICENSE-UNICODE"]
    rendered = notices.render(components, target="x86_64-unknown-linux-gnu")
    assert "unicode text" in rendered and "| unicode-ident 1.0.27 |" in rendered
    assert rendered.index("## Runtime components") < rendered.index("| shipped 0.3.0 |") < rendered.index("## Build-time only")
    assert rendered.index("## Build-time only") < rendered.index("| some-derive 1.0.0 |")
    assert "test-only" not in rendered


def test_runtime_crate_without_text_requires_a_reviewed_override(tmp_path):
    py, node = roots(tmp_path)
    bare = crate(tmp_path, "bare-crate", "1.2.3", "MIT")
    m = meta([py, node, bare], {py["id"]: [(bare["id"], None)]})
    with pytest.raises(notices.CoverageError, match="bare-crate 1.2.3"):
        notices.collect(m, overrides=tmp_path / "no-overrides")
    override = tmp_path / "overrides" / "bare-crate"
    override.mkdir(parents=True)
    (override / "LICENSE").write_text("override mit text")
    with pytest.raises(notices.CoverageError, match="no SOURCE"):
        notices.collect(m, overrides=tmp_path / "overrides")
    (override / "SOURCE").write_text("Source: https://example.invalid/LICENSE at tag v1.2.3")
    components = notices.collect(m, overrides=tmp_path / "overrides")
    (c,) = [c for c in components if c.name == "bare-crate"]
    assert c.texts == [("LICENSE", "override mit text")]
    rendered = notices.render(components)
    assert "override mit text" in rendered
    assert "- **bare-crate 1.2.3**: Source: https://example.invalid/LICENSE at tag v1.2.3" in rendered


def test_build_only_crate_without_text_is_listed_not_fatal(tmp_path):
    py, node = roots(tmp_path)
    macro = crate(tmp_path, "bare-derive", "0.1.0", "MIT", proc_macro=True)
    m = meta([py, node, macro], {py["id"]: [(macro["id"], None)]})
    rendered = notices.render(notices.collect(m, overrides=tmp_path / "none"))
    assert "| bare-derive 0.1.0 | MIT | MIT | <https://example.invalid> | none in crate |" in rendered


def test_checked_in_notices_cover_every_runtime_crate_for_the_release_target():
    components = notices.collect(notices.metadata())
    assert all(c.texts for c in components if c.runtime)
    assert (ROOT / "THIRD-PARTY-NOTICES.md").read_text() == notices.render(components)
