"""Write release/<version>/MANIFEST.json deterministically.

The manifest describes the artifacts of one release: source commit (and its
committer date), version, target, floors, toolchain identity, and the SHA-256
and size of every artifact. It contains no build timestamp, so two builds of
the same source that produce byte-identical artifacts produce a byte-identical
manifest, and registering the release twice is a no-op rather than a second
MANIFEST.json under the same name.

    write_manifest.py <release-dir> <version> <source-commit> <source-date>
                      <target> <image> <max-glibc> <node-version> <toolchain-file>
"""
from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

SCHEMA = "radicle-sdks-release-manifest/1"
EXCLUDED = {"MANIFEST.json", "SHA256SUMS"}


def build(release_dir: Path, version: str, source: str, source_date: str, target: str,
          image: str, max_glibc: str, node_version: str, toolchain: list[str]) -> dict:
    files = {}
    for p in sorted(release_dir.iterdir()):
        if p.name in EXCLUDED or not p.is_file():
            continue
        data = p.read_bytes()
        files[p.name] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    if not files:
        raise SystemExit(f"no artifacts in {release_dir}")
    rustc, cargo, maturin = (toolchain + ["", "", ""])[:3]
    return {
        "schema": SCHEMA,
        "version": version,
        "source": source,
        "source_date": source_date,
        "target": target,
        "floors": {"glibc": max_glibc, "python": "3.9 (abi3)", "node": "26.5.0"},
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
