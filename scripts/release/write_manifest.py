"""Write release/<version>/MANIFEST.json deterministically.

The manifest describes the artifacts of one release: source commit (and its
committer date), version, target, floors, toolchain identity, and the SHA-256
and size of every artifact. It contains no build timestamp, so two builds of
the same source that produce byte-identical artifacts produce a byte-identical
manifest, and registering the release twice is a no-op rather than a second
MANIFEST.json under the same name.

    write_manifest.py <release-dir> <version> <source-commit> <source-date>
                      <targets> <image> <max-glibc> <node-version> <toolchain-file>

<targets> is a comma-separated list of target triples the release covers, the
Linux build host first; a wheel is attributed to a target by its platform tag,
the sdist and the npm tarball to all targets.
"""
from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

SCHEMA = "radicle-sdks-release-manifest/1"
EXCLUDED = {"MANIFEST.json", "SHA256SUMS"}


PLATFORM_TAGS = {"manylinux_2_28_x86_64": "x86_64-unknown-linux-gnu", "macosx_11_0_arm64": "aarch64-apple-darwin"}


def file_target(name: str, targets: list[str]) -> list[str]:
    if name.endswith(".whl"):
        tag = name[:-4].split("-")[-1]
        target = PLATFORM_TAGS.get(tag)
        if target is None or target not in targets:
            raise SystemExit(f"{name}: platform tag {tag} is not one of the release targets {targets}")
        return [target]
    return list(targets)


def build(release_dir: Path, version: str, source: str, source_date: str, targets: str | list[str],
          image: str, max_glibc: str, node_version: str, toolchain: list[str]) -> dict:
    targets = [t for t in (targets.split(",") if isinstance(targets, str) else targets) if t]
    if not targets:
        raise SystemExit("at least one target is required")
    files = {}
    for p in sorted(release_dir.iterdir()):
        if p.name in EXCLUDED or not p.is_file() or p.name.startswith("."):
            continue
        data = p.read_bytes()
        files[p.name] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data), "targets": file_target(p.name, targets)}
    if not files:
        raise SystemExit(f"no artifacts in {release_dir}")
    for target in targets:
        if not any(n.endswith(".whl") and target in e["targets"] for n, e in files.items()):
            raise SystemExit(f"no wheel for target {target}")
    rustc, cargo, maturin = (toolchain + ["", "", ""])[:3]
    return {
        "schema": SCHEMA,
        "version": version,
        "source": source,
        "source_date": source_date,
        "targets": targets,
        "floors": {"glibc": max_glibc, "macos_wheel": "11.0", "macos_addon": "13.5", "python": "3.9 (abi3)", "node": "26.5.0"},
        "toolchain": {"image": image, "rustc": rustc, "cargo": cargo, "maturin": maturin, "node_loader": node_version},
        "files": files,
    }


def render(manifest: dict) -> str:
    return json.dumps(manifest, indent=2, sort_keys=True) + "\n"


def main(argv: list[str]) -> int:
    if len(argv) != 10:
        print(__doc__, file=sys.stderr)
        return 2
    release_dir = Path(argv[1])
    toolchain = [line.strip() for line in Path(argv[9]).read_text().splitlines() if line.strip()]
    manifest = build(release_dir, argv[2], argv[3], argv[4], argv[5], argv[6], argv[7], argv[8], toolchain)
    (release_dir / "MANIFEST.json").write_text(render(manifest))
    print(render(manifest), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
