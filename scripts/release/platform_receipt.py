"""Check and use a staged platform receipt (release/incoming/<platform>/AUDIT.json).

A platform build script that runs on its own hardware (build-macos-arm64.sh)
stages its files with a receipt: version, source commit, target, floors,
toolchain, SHA-256 and size of every file, and the audit line each file
passed. The Linux assembly trusts nothing about that directory beyond what the
receipt binds, so the receipt must name exactly the files the release expects
for that target, every one must be present with the recorded digest and size,
and every one must carry a passing audit line.

    platform_receipt.py check <incoming-dir> <version> <source-commit>
        Exit 0 and print the audit lines when the receipt is complete and
        the files match it; exit 1 with the reason otherwise.
    platform_receipt.py verify <AUDIT.json> <name> <path>
        Exit 0 when <path> has the digest and size the receipt records for
        <name>, for comparing packaged bytes back to the audited ones.
"""
from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

SCHEMA = "radicle-sdks-platform-audit/1"
TARGETS = {
    "aarch64-apple-darwin": {
        "files": lambda version: [f"radicle_py-{version}-cp39-abi3-macosx_11_0_arm64.whl", "radicle-sdk.darwin-arm64.node"],
        "floors": {"macos_wheel": "11.0", "macos_addon": "13.5"},
        "audit_prefix": "arm64 Mach-O, macOS minimum ",
        "audit_suffix": ", system libraries only, signature verifies",
    },
}


class ReceiptError(SystemExit):
    pass


def load(path: Path) -> dict:
    try:
        receipt = json.loads(path.read_text())
    except (OSError, ValueError) as e:
        raise ReceiptError(f"{path}: cannot read receipt: {e}")
    if not isinstance(receipt, dict) or receipt.get("schema") != SCHEMA:
        raise ReceiptError(f"{path}: not a {SCHEMA} receipt")
    return receipt


def digest_of(path: Path) -> tuple[str, int]:
    data = path.read_bytes()
    return hashlib.sha256(data).hexdigest(), len(data)


def entry(receipt: dict, name: str) -> dict:
    files = receipt.get("files")
    if not isinstance(files, dict) or name not in files:
        raise ReceiptError(f"receipt does not list {name}")
    e = files[name]
    sha = e.get("sha256") if isinstance(e, dict) else None
    size = e.get("bytes") if isinstance(e, dict) else None
    if not (isinstance(sha, str) and len(sha) == 64 and all(c in "0123456789abcdef" for c in sha)):
        raise ReceiptError(f"{name}: receipt has no valid sha256")
    if not isinstance(size, int) or size <= 0:
        raise ReceiptError(f"{name}: receipt has no valid size")
    return e


def check_file(receipt: dict, name: str, path: Path) -> None:
    e = entry(receipt, name)
    if not path.is_file():
        raise ReceiptError(f"{name}: missing ({path})")
    sha, size = digest_of(path)
    if sha != e["sha256"]:
        raise ReceiptError(f"{name}: sha256 {sha} differs from the receipt's {e['sha256']}")
    if size != e["bytes"]:
        raise ReceiptError(f"{name}: size {size} differs from the receipt's {e['bytes']}")


def check(directory: Path, version: str, source: str) -> list[str]:
    receipt = load(directory / "AUDIT.json")
    target = receipt.get("target")
    if target not in TARGETS:
        raise ReceiptError(f"receipt target {target!r} is not one the release assembles")
    spec = TARGETS[target]
    for key, want in (("version", version), ("source", source)):
        if receipt.get(key) != want:
            raise ReceiptError(f"receipt {key} is {receipt.get(key)!r}, expected {want!r}")
    for floor, want in spec["floors"].items():
        if (receipt.get("floors") or {}).get(floor) != want:
            raise ReceiptError(f"receipt floor {floor} is {(receipt.get('floors') or {}).get(floor)!r}, expected {want!r}")
    expected = spec["files"](version)
    listed = sorted(receipt.get("files") or {})
    if listed != sorted(expected):
        raise ReceiptError(f"receipt lists files {listed}, expected exactly {sorted(expected)}")
    audits = receipt.get("audit") or {}
    if sorted(audits) != sorted(expected):
        raise ReceiptError(f"receipt audit lines cover {sorted(audits)}, expected exactly {sorted(expected)}")
    lines = []
    for name in expected:
        check_file(receipt, name, directory / name)
        line = audits[name]
        if not (isinstance(line, str) and line.startswith(f"{name}: {spec['audit_prefix']}") and line.endswith(spec["audit_suffix"])):
            raise ReceiptError(f"{name}: audit line is not a passing audit of this file: {line!r}")
        lines.append(line)
    stray = sorted(p.name for p in directory.iterdir() if not p.name.startswith(".") and p.name not in expected and p.name != "AUDIT.json")
    if stray:
        raise ReceiptError(f"unexpected files staged next to the receipt: {stray}")
    return lines


def verify(audit_path: Path, name: str, path: Path) -> None:
    check_file(load(audit_path), name, path)


def main(argv: list[str]) -> int:
    if len(argv) == 5 and argv[1] == "check":
        for line in check(Path(argv[2]), argv[3], argv[4]):
            print(f"  {line}")
        return 0
    if len(argv) == 5 and argv[1] == "verify":
        verify(Path(argv[2]), argv[3], Path(argv[4]))
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
