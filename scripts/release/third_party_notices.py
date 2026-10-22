"""Generate THIRD-PARTY-NOTICES.md for the native modules of both SDKs.

The wheel and the npm tarball each contain one native library that statically
links libgit2, SQLite and zlib and compiles in the Rust crates reachable from
the two native packages for the release target. This script lists those
components with their licence expressions and reproduces every licence and
notice file each crate ships, from the exact registry source named by
Cargo.lock, deduplicated by content. Crates that publish no licence text are
covered by reviewed overrides under scripts/release/notices/<crate>/ (texts
taken from the crate's pinned upstream source, with a SOURCE file recording
where from). A runtime crate with neither is an error, not a silent gap.

Crates are classified for the target platform (cargo metadata
--filter-platform): "runtime" crates are reachable through normal dependencies
only and are not procedural macros, so their code is compiled into the shipped
module; everything else (build scripts, procedural macros and their
dependencies) runs at build time and is listed separately. Compound licence
expressions keep every AND term; only OR alternatives are chosen.

    uv run --no-sync python scripts/release/third_party_notices.py --write
    uv run --no-sync python scripts/release/third_party_notices.py --check
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "THIRD-PARTY-NOTICES.md"
OVERRIDES = ROOT / "scripts" / "release" / "notices"
NATIVE_PACKAGES = ("radicle-py", "radicle-sdk-native")
TARGET = "x86_64-unknown-linux-gnu"
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE)", re.IGNORECASE)
# When a crate offers alternatives, the one we rely on, in order of preference.
PREFERRED = ("MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Zlib", "Unlicense", "MIT-0", "CC0-1.0")


class CoverageError(Exception):
    """A shipped crate has no licence text in its source and no reviewed override."""


@dataclass
class Component:
    name: str
    version: str
    expression: str
    used: str
    repository: str
    runtime: bool
    source_dir: Path
    texts: list[tuple[str, str]] = field(default_factory=list)  # (label, text)
    provenance: str = ""

    @property
    def label(self) -> str:
        return f"{self.name} {self.version}"


def metadata(target: str = TARGET) -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(out)


def used_license(expression: str) -> str:
    """Keep every AND term; within each term pick the preferred OR alternative.

    '(MIT OR Apache-2.0) AND Unicode-3.0' -> 'MIT AND Unicode-3.0'
    'MIT OR Apache-2.0 OR LGPL-2.1-or-later' -> 'MIT'
    'Apache-2.0 WITH LLVM-exception' -> 'Apache-2.0 WITH LLVM-exception'
    """
    terms = []
    for term in re.split(r"\s+AND\s+", expression):
        alternatives = [a.strip().strip("()").strip() for a in re.split(r"\s+OR\s+|/", term.strip().strip("()"))]
        chosen = next((alt for pref in PREFERRED for alt in alternatives
                       if alt == pref or alt.startswith(pref + " WITH")), alternatives[0])
        terms.append(chosen)
    return " AND ".join(terms)


def is_proc_macro(package: dict) -> bool:
    return any("proc-macro" in target["kind"] for target in package["targets"])


def classify(meta: dict) -> dict[str, bool]:
    """Map package id -> runtime (True) or build-time only (False) for the roots."""
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    roots = [p["id"] for p in meta["packages"] if p["name"] in NATIVE_PACKAGES]
    if len(roots) != len(NATIVE_PACKAGES):
        raise RuntimeError(f"expected roots {NATIVE_PACKAGES}, found {roots}")
    result: dict[str, bool] = {}

    def walk(pkg_id: str, runtime: bool) -> None:
        if result.get(pkg_id) is True or (pkg_id in result and not runtime):
            return
        result[pkg_id] = runtime
        for dep in nodes[pkg_id]["deps"]:
            kinds = [k["kind"] for k in dep["dep_kinds"]]
            normal = None in kinds
            if not normal and "build" not in kinds:
                continue  # dev-dependency only: never shipped
            child_runtime = runtime and normal and not is_proc_macro(packages[dep["pkg"]])
            walk(dep["pkg"], child_runtime)

    for root in roots:
        walk(root, True)
    return result


def read_texts(directory: Path) -> list[tuple[str, str]]:
    out = []
    for p in sorted(directory.iterdir()):
        if p.is_file() and LICENSE_FILE.match(p.name):
            text = p.read_bytes().decode("utf-8", errors="replace").replace("\r\n", "\n").strip("\n")
            out.append((p.name, text))
    return out


def collect(meta: dict, overrides: Path = OVERRIDES) -> list[Component]:
    packages = {p["id"]: p for p in meta["packages"]}
    components = []
    for pkg_id, runtime in classify(meta).items():
        p = packages[pkg_id]
        if p["source"] is None:
            continue
        expression = p.get("license") or ""
        c = Component(
            name=p["name"], version=p["version"], expression=expression or "(none declared)",
            used=used_license(expression) if expression else "(none declared)",
            repository=p.get("repository") or "", runtime=runtime,
            source_dir=Path(p["manifest_path"]).parent,
        )
        c.texts = read_texts(c.source_dir)
        if not c.texts:
            override = overrides / c.name
            if override.is_dir():
                c.texts = read_texts(override)
                source = override / "SOURCE"
                if not source.exists():
                    raise CoverageError(f"override for {c.label} has no SOURCE file recording its provenance")
                c.provenance = source.read_text().strip()
        components.append(c)
    missing = [c.label for c in components if c.runtime and not c.texts]
    if missing:
        raise CoverageError("runtime crates without licence text or reviewed override: " + ", ".join(sorted(missing)))
    return sorted(components, key=lambda c: (not c.runtime, c.name, c.version))


def c_library_sections(components: list[Component]) -> list[str]:
    by_name = {c.name: c for c in components}
    sections = []
    if "libgit2-sys" in by_name:
        c = by_name["libgit2-sys"]
        text = (c.source_dir / "libgit2" / "COPYING").read_text(encoding="utf-8", errors="replace").strip("\n")
        sections.append(f"### libgit2 {c.version.split('+')[-1]}\n\nStatically linked, built from the sources bundled "
                        f"with the `libgit2-sys` crate. Licensed under the GNU GPL version 2 with the linking "
                        f"exception reproduced below, which permits linking into this library.\n\n```text\n{text}\n```\n")
    if "libz-sys" in by_name:
        c = by_name["libz-sys"]
        text = (c.source_dir / "src" / "zlib" / "LICENSE").read_text(encoding="utf-8", errors="replace").strip("\n")
        sections.append(f"### zlib\n\nStatically linked, built from the sources bundled with the `libz-sys` "
                        f"crate.\n\n```text\n{text}\n```\n")
    if "sqlite3-src" in by_name:
        sections.append("### SQLite\n\nStatically linked, built from the amalgamation bundled with the "
                        "`sqlite3-src` crate. SQLite is in the public domain; see "
                        "<https://sqlite.org/copyright.html>.\n")
    return sections


def render(components: list[Component], target: str = TARGET) -> str:
    texts: dict[str, tuple[str, list[str]]] = {}
    refs: dict[str, list[str]] = {}
    for c in components:
        refs[c.label] = []
        for filename, text in c.texts:
            digest = hashlib.sha256(text.encode()).hexdigest()[:12]
            texts.setdefault(digest, (text, []))[1].append(f"{c.label} ({filename})")
            refs[c.label].append(digest)
    runtime = [c for c in components if c.runtime]
    build_only = [c for c in components if not c.runtime]
    lines = [
        "# Third-party notices",
        "",
        "The native module in each SDK package (`radiclepy.abi3.so` in the wheel, the",
        "`.node` addon in the npm tarball) statically links the C libraries below and",
        "compiles in the Rust crates listed under \"Runtime components\". Each component's",
        "licence and copyright text is reproduced from the exact source used for the",
        f"build (target `{target}`). The SDK itself is licensed under MIT OR Apache-2.0",
        "(`LICENSE-MIT`, `LICENSE-APACHE`). Where a crate offers a choice of licences, the",
        "\"used\" column names the alternative this distribution relies on; AND terms are",
        "kept. This file is generated by `scripts/release/third_party_notices.py` from",
        "`Cargo.lock`; do not edit by hand.",
        "",
        "## Statically linked C libraries",
        "",
        *c_library_sections(components),
        "## Runtime components",
        "",
        f"{len(runtime)} crates whose code is compiled into the native module: reachable from",
        f"`{NATIVE_PACKAGES[0]}` and `{NATIVE_PACKAGES[1]}` through normal dependencies for",
        f"`{target}`, excluding procedural macros. Every one has licence text below, from",
        "the crate itself or from a reviewed override whose provenance is given.",
        "",
        "| Crate | Licence expression | Used | Source | Notice texts |",
        "| --- | --- | --- | --- | --- |",
    ]
    for c in runtime:
        source = f"<{c.repository}>" if c.repository else ""
        lines.append(f"| {c.label} | {c.expression} | {c.used} | {source} | {', '.join(refs[c.label])} |")
    overrides = [c for c in runtime if c.provenance]
    if overrides:
        lines += ["", "### Reviewed overrides", "",
                  "These crates publish no licence text in their registry source; the texts above were",
                  "taken from the following places.", ""]
        for c in overrides:
            lines.append(f"- **{c.label}**: {c.provenance}")
    lines += [
        "", "## Build-time only", "",
        f"{len(build_only)} crates used only while building (build scripts, procedural macros and",
        "their dependencies); their code is not in the shipped module. Listed for completeness",
        "with whatever licence text their source carries.",
        "",
        "| Crate | Licence expression | Used | Source | Notice texts |",
        "| --- | --- | --- | --- | --- |",
    ]
    for c in build_only:
        source = f"<{c.repository}>" if c.repository else ""
        lines.append(f"| {c.label} | {c.expression} | {c.used} | {source} | {', '.join(refs[c.label]) or 'none in crate'} |")
    lines += ["", "## Notice texts", "",
              "Each text below is reproduced verbatim from the files named after its identifier.", ""]
    for digest, (text, users) in sorted(texts.items(), key=lambda kv: (-len(kv[1][1]), kv[0])):
        lines += [f"### {digest}", "", "Used by: " + ", ".join(users), "", "```text",
                  text.replace("```", "'''"), "```", ""]
    return "\n".join(lines).rstrip("\n") + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true", help="write THIRD-PARTY-NOTICES.md")
    mode.add_argument("--check", action="store_true", help="fail if THIRD-PARTY-NOTICES.md is stale")
    args = parser.parse_args()
    try:
        content = render(collect(metadata()))
    except CoverageError as error:
        print(error, file=sys.stderr)
        return 1
    if args.write:
        OUTPUT.write_text(content, encoding="utf-8")
        print(f"wrote {OUTPUT.relative_to(ROOT)} ({content.count(chr(10))} lines)")
        return 0
    current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
    if current != content:
        print("THIRD-PARTY-NOTICES.md is stale; regenerate with --write", file=sys.stderr)
        return 1
    print("THIRD-PARTY-NOTICES.md is up to date")
    return 0


if __name__ == "__main__":
    sys.exit(main())
