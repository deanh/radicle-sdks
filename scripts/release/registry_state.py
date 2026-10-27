"""Compare a release manifest with what PyPI and npm hold for that version.

Published registry files are immutable, so the only acceptable states for a
file the manifest lists are "absent" (upload it) and "present with the same
digest" (skip it). A file present under this version with different bytes,
or a registry file this release never built, is a conflict: refuse, and
publish a fixed version instead.

    registry_state.py pypi-plan <manifest> <pypi-release-json | ->
        One line per Python artifact: "upload NAME" or "skip NAME". "-" means
        PyPI has no such version. Exit 1 on a conflict.
    registry_state.py pypi-files <manifest> <pypi-release-json>
        One line per Python artifact, "URL NAME SHA256", requiring PyPI to
        hold exactly the manifest's Python files with the manifest's digests;
        for verification downloads.
    registry_state.py npm-plan <manifest> <tarball-path> <integrity | ->
        "upload NAME" or "skip NAME" for the npm tarball, given the integrity
        npm reports for this version ("-" when absent). Exit 1 on a conflict.
    registry_state.py npm-check <manifest> <integrity>
        Exit 0 when the integrity npm reports is that of the manifest's
        tarball bytes (the manifest records sha512 integrity from 0.1.1;
        pass the tarball path as a fourth argument for older manifests).
    registry_state.py integrity <file>
        Print the file's npm integrity string (sha512, base64).
  pypi-requirement <manifest> <project>            requirements line with --hash for every Python file
  npm-lock <manifest> <registry> <package> <consumer> package-lock.json pinning the tarball integrity
"""
from __future__ import annotations

import base64
import hashlib
import json
import sys
from pathlib import Path


def python_files(m: dict) -> dict:
    return {n: e for n, e in m["files"].items() if n.endswith(".whl") or n.endswith(".tar.gz")}


def npm_tarball(m: dict) -> str:
    names = [n for n in m["files"] if n.endswith(".tgz")]
    if len(names) != 1:
        raise SystemExit(f"manifest lists {len(names)} npm tarballs, expected one")
    return names[0]


def integrity(path: Path) -> str:
    return "sha512-" + base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode()


def pypi_release(path: str) -> dict | None:
    if path == "-":
        return None
    r = json.loads(Path(path).read_text())
    urls = r.get("urls")
    if not isinstance(urls, list):
        raise SystemExit("PyPI release JSON has no 'urls' list")
    return {u["filename"]: u for u in urls}


def pypi_plan(m: dict, release: dict | None) -> list[tuple[str, str]]:
    wanted = python_files(m)
    present = release or {}
    foreign = sorted(set(present) - set(wanted))
    if foreign:
        raise SystemExit(f"PyPI holds files under {m['version']} that this release did not build: {foreign}")
    plan = []
    for name, entry in sorted(wanted.items()):
        if name not in present:
            plan.append(("upload", name))
            continue
        got = present[name].get("digests", {}).get("sha256")
        if got != entry["sha256"]:
            raise SystemExit(f"{name}: PyPI holds sha256 {got}, this release built {entry['sha256']}; "
                             "published files are immutable, publish a new version")
        plan.append(("skip", name))
    return plan


def pypi_files(m: dict, release: dict | None) -> list[tuple[str, str, str]]:
    if release is None:
        raise SystemExit(f"PyPI has no release {m['version']}")
    plan = pypi_plan(m, release)
    missing = [n for action, n in plan if action == "upload"]
    if missing:
        raise SystemExit(f"PyPI lacks {missing} for {m['version']}")
    return [(release[n]["url"], n, m["files"][n]["sha256"]) for _, n in plan]


def npm_plan(m: dict, tarball: Path, reported: str) -> tuple[str, str]:
    name = npm_tarball(m)
    if tarball.name != name:
        raise SystemExit(f"tarball {tarball.name} is not the manifest's {name}")
    local = integrity(tarball)
    if reported == "-" or reported == "":
        return ("upload", name)
    if reported != local:
        raise SystemExit(f"{name}: npm holds integrity {reported}, this release built {local}; "
                         "published versions are immutable, publish a new version")
    return ("skip", name)


def npm_check(m: dict, reported: str, tarball: Path | None) -> str:
    name = npm_tarball(m)
    want = m["files"][name].get("integrity")
    if want is None:
        if tarball is None:
            raise SystemExit(f"manifest records no integrity for {name}; pass the tarball path")
        want = integrity(tarball)
    if reported in ("", "-"):
        raise SystemExit(f"npm has no {name.rsplit('-', 1)[0]}@{m['version']}")
    if reported != want:
        raise SystemExit(f"{name}: npm reports integrity {reported}, manifest says {want}")
    return name


def pypi_requirement(m: dict, project: str) -> str:
    """A requirements line pinning the project version to every Python file digest the
    manifest records, for `uv pip install --require-hashes`: the installer may then only
    accept bytes the receipt attests, whatever the index resolves."""
    hashes = [e["sha256"] for _, e in sorted(python_files(m).items())]
    if not hashes:
        raise SystemExit("manifest lists no Python files")
    return f"{project}=={m['version']} " + " ".join(f"--hash=sha256:{h}" for h in hashes)


def npm_lock(m: dict, registry: str, package: str, consumer: str) -> dict:
    """A package-lock.json (v3) pinning the package to the registry tarball with the
    manifest's integrity, for `npm ci`: npm refuses tarball bytes that differ."""
    name = npm_tarball(m)
    want = m["files"][name].get("integrity")
    if want is None:
        raise SystemExit(f"manifest records no integrity for {name}")
    return {"name": consumer, "version": "1.0.0", "lockfileVersion": 3, "requires": True,
            "packages": {"": {"name": consumer, "version": "1.0.0", "dependencies": {package: m["version"]}},
                         f"node_modules/{package}": {"version": m["version"],
                                                     "resolved": f"{registry.rstrip('/')}/{package}/-/{name}",
                                                     "integrity": want}}}


def main(argv: list[str]) -> int:
    cmd = argv[1] if len(argv) > 1 else ""
    if cmd == "integrity" and len(argv) == 3:
        print(integrity(Path(argv[2])))
        return 0
    if cmd == "pypi-requirement" and len(argv) == 4:
        print(pypi_requirement(json.loads(Path(argv[2]).read_text()), argv[3]))
        return 0
    if cmd == "npm-lock" and len(argv) == 6:
        print(json.dumps(npm_lock(json.loads(Path(argv[2]).read_text()), argv[3], argv[4], argv[5]), indent=2))
        return 0
    if cmd in ("pypi-plan", "pypi-files", "npm-plan", "npm-check") and len(argv) >= 4:
        m = json.loads(Path(argv[2]).read_text())
        if cmd == "pypi-plan":
            for action, name in pypi_plan(m, pypi_release(argv[3])):
                print(action, name)
        elif cmd == "pypi-files":
            for url, name, sha in pypi_files(m, pypi_release(argv[3])):
                print(url, name, sha)
        elif cmd == "npm-plan" and len(argv) == 5:
            print(*npm_plan(m, Path(argv[3]), argv[4]))
        elif cmd == "npm-check":
            print(npm_check(m, argv[3], Path(argv[4]) if len(argv) > 4 else None))
        else:
            print(__doc__, file=sys.stderr)
            return 2
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
