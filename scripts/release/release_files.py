"""Check a release directory against its manifest.

    release_files.py check <release-dir> <version>
        Exit 0 and print the expected artifact names, one per line, when
        MANIFEST.json names this version, lists exactly the artifacts the
        manifest's targets call for, and every one is present with the
        recorded SHA-256 and size. Exit 1 with the reason otherwise.

The expected set is derived from the manifest's targets: one wheel per target
(by platform tag), the sdist and the npm tarball. Manifests before 0.1.1
carried a single `target`.
"""
from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

WHEELS = {"x86_64-unknown-linux-gnu": "manylinux_2_28_x86_64", "aarch64-apple-darwin": "macosx_11_0_arm64"}


def expected_names(manifest: dict) -> list[str]:
    version = manifest["version"]
    targets = manifest.get("targets") or [manifest["target"]]
    names = []
    for target in targets:
        if target not in WHEELS:
            raise SystemExit(f"manifest target {target} has no known wheel platform tag")
        names.append(f"radicle_py-{version}-cp39-abi3-{WHEELS[target]}.whl")
    return names + [f"radicle_py-{version}.tar.gz", f"radicle-sdk-{version}.tgz"]


def check(release_dir: Path, version: str) -> list[str]:
    manifest = release_dir / "MANIFEST.json"
    if not manifest.is_file():
        raise SystemExit(f"missing {manifest}; run build-linux-x86_64.sh first")
    m = json.loads(manifest.read_text())
    if m.get("version") != version:
        raise SystemExit(f"manifest is for version {m.get('version')}, not {version}")
    expected = expected_names(m)
    listed = sorted(m["files"])
    if listed != sorted(expected):
        raise SystemExit(f"manifest lists {listed}, expected {sorted(expected)}")
    for name, entry in m["files"].items():
        p = release_dir / name
        if not p.is_file():
            raise SystemExit(f"missing artifact {name}")
        data = p.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if digest != entry["sha256"]:
            raise SystemExit(f"{name}: sha256 {digest} differs from manifest {entry['sha256']}; rebuild before publishing")
        if len(data) != entry["bytes"]:
            raise SystemExit(f"{name}: size differs from manifest")
    return expected


def main(argv: list[str]) -> int:
    if len(argv) == 4 and argv[1] == "check":
        for name in check(Path(argv[2]), argv[3]):
            print(name)
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
