"""Release scripts fail closed: stale or mismatched artifacts are never registered,
the ELF audit rejects bad binaries, and the download verifier refuses the wrong
version. Transport (rad, rad-artifact) is mocked; Git, hashing and installs are real."""
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import stat
import subprocess

import pytest

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "scripts" / "release"
VERSION = "0.1.0"
RID = "rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg"
NAMES = [
    f"radicle_py-{VERSION}-cp39-abi3-manylinux_2_28_x86_64.whl",
    f"radicle_py-{VERSION}.tar.gz",
    f"radicle-sdk-{VERSION}.tgz",
]


def shim(directory: Path, name: str, body: str) -> None:
    path = directory / name
    path.write_text("#!/usr/bin/env bash\n" + body)
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


def run(script: str, *args, cwd: Path, shims: Path, env=None):
    environment = dict(os.environ, PATH=f"{shims}:{os.environ['PATH']}", **(env or {}))
    return subprocess.run(["bash", str(SCRIPTS / script), *args], cwd=cwd, env=environment,
                          capture_output=True, text=True, timeout=120)


@pytest.fixture
def shims(tmp_path):
    directory = tmp_path / "shims"
    directory.mkdir()
    log = tmp_path / "calls.log"
    shim(directory, "rad", f'echo "{RID}"\n')
    state = tmp_path / "artifact-state.json"
    shim(directory, "rad-artifact", f"""
printf '%s\\n' "$*" >> {log}
state="${{ARTIFACT_STATE:-{state}}}"
case "$*" in
  *"node status"*) exit 0 ;;
  cid\ *) printf 'cid-%s\\n' "$(sha256sum "$2" | cut -c1-16)" ;;
  *" register "*)
    file=""; name=""; rev=""; while [ $# -gt 0 ]; do case "$1" in register) file=$2; shift ;; --name) name=$2; shift ;; --revision) rev=$2; shift ;; esac; shift; done
    cid="cid-$(sha256sum "$file" | cut -c1-16)"
    printf '%s %s %s\\n' "$rev" "$cid" "$name" >> "$state"
    printf '{{"cid":"%s","name":"%s","releaseId":"rel-%s","oid":"abc"}}\\n' "$cid" "$name" "$rev" ;;
  *" seed "*|*" show "*) exit 0 ;;
  *" download "*) for a in "$@"; do if [ -n "$prev_o" ]; then cp "$ARTIFACT_SOURCE/$a" "$a"; break; fi; [ "$a" = "-o" ] && prev_o=1; done ;;
  *" verify "*) exit 0 ;;
  *" list "*)
    if [ -n "${{ARTIFACT_LISTING:-}}" ]; then cat "$ARTIFACT_LISTING"; else
      python3 - "$state" <<'PY'
import json, sys, os
releases = {{}}
if os.path.exists(sys.argv[1]):
    for line in open(sys.argv[1]):
        rev, cid, name = line.split()
        releases.setdefault(rev, []).append({{"cid": cid, "name": name, "redactions": []}})
print(json.dumps({{"releases": [{{"releaseId": f"rel-{{rev}}", "tag": rev, "tagName": "v?", "artifacts": arts}} for rev, arts in releases.items()]}}))
PY
    fi ;;
  *) echo "unexpected rad-artifact $*" >&2; exit 9 ;;
esac
""")
    return directory


def git(path: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(path), "-c", "user.name=t", "-c", "user.email=t@example.invalid",
                           "-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", *args],
                          check=True, capture_output=True, text=True).stdout.strip()


def release_fixture(tmp_path: Path, *, version=VERSION, source=None):
    """A Git repository with a bare 'rad' remote, an annotated tag pushed to it,
    and a release directory with a consistent manifest."""
    repo = tmp_path / "repo"
    repo.mkdir()
    git(repo, "init", "--initial-branch=main")
    (repo / "file").write_text("x\n")
    git(repo, "add", "."); git(repo, "commit", "-m", "initial")
    # Like a real rad remote: the fetch URL lists only canonical refs (no tags),
    # the push URL addresses the publisher's namespace, where the tag is stored.
    canonical = tmp_path / "canonical.git"
    git(tmp_path, "init", "--bare", str(canonical))
    bare = tmp_path / "namespace.git"
    git(tmp_path, "init", "--bare", str(bare))
    git(repo, "remote", "add", "rad", str(canonical))
    git(repo, "config", "remote.rad.pushurl", str(bare))
    git(repo, "tag", "-a", f"v{version}", "-m", "release")
    git(repo, "push", "-q", "rad", f"v{version}")
    assert git(tmp_path, "-C", str(canonical), "tag") == ""  # the fetch side never sees the tag
    commit = git(repo, "rev-parse", "HEAD")
    out = repo / "release" / version
    out.mkdir(parents=True)
    files = {}
    for name in NAMES:
        data = f"bytes of {name}\n".encode()
        (out / name).write_bytes(data)
        files[name] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    (out / "MANIFEST.json").write_text(json.dumps({
        "schema": "radicle-sdks-release-manifest/1", "version": version,
        "source": source or commit, "targets": ["x86_64-unknown-linux-gnu"], "files": files,
    }))
    (out / "SHA256SUMS").write_text("".join(f"{v['sha256']}  {k}\n" for k, v in files.items()))
    return repo, out, commit


def test_register_happy_path_registers_files_and_manifest(tmp_path, shims):
    repo, out, commit = release_fixture(tmp_path)
    result = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert result.returncode == 0, result.stderr
    calls = (tmp_path / "calls.log").read_text().splitlines()
    registered = [c for c in calls if " register " in c]
    assert [c.split("--name ")[1].split(" ")[0] for c in registered] == NAMES + ["MANIFEST.json"]
    tag_object = git(repo, "rev-parse", f"refs/tags/v{VERSION}")
    assert all(f"--revision {tag_object}" in c for c in registered)
    assert len([c for c in calls if " seed " in c]) == 4
    assert commit in result.stdout
    # Identical rerun: everything resumes, nothing is registered again, seeding repeats.
    (tmp_path / "calls.log").unlink()
    result = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert result.returncode == 0, result.stderr
    calls = (tmp_path / "calls.log").read_text().splitlines()
    assert not any(" register " in c for c in calls)
    assert len([c for c in calls if " seed " in c]) == 4
    assert result.stdout.count("already registered") == 4


def registered_names(tmp_path):
    log = tmp_path / "calls.log"
    return [c.split("--name ")[1].split(" ")[0] for c in log.read_text().splitlines() if " register " in c] if log.exists() else []


def rewrite_manifest(out: Path, mutate):
    m = json.loads((out / "MANIFEST.json").read_text()); mutate(m)
    (out / "MANIFEST.json").write_text(json.dumps(m, indent=2, sort_keys=True) + "\n")


def test_register_refuses_same_name_different_content_after_a_first_registration(tmp_path, shims):
    repo, out, commit = release_fixture(tmp_path)
    assert run("register.sh", VERSION, cwd=repo, shims=shims).returncode == 0
    (tmp_path / "calls.log").unlink()
    # Manifest-only rebuild: same artifacts, different manifest bytes (for example a timestamp).
    rewrite_manifest(out, lambda m: m.update(built_at="later"))
    r = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert r.returncode != 0 and "MANIFEST.json: live content" in r.stderr and "immutable" in r.stderr
    assert registered_names(tmp_path) == []
    # Changed artifact with a consistent manifest: still refused.
    data = b"rebuilt with different bytes\n"
    (out / NAMES[2]).write_bytes(data)
    rewrite_manifest(out, lambda m: m["files"].__setitem__(NAMES[2], {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}))
    (out / "SHA256SUMS").write_text("".join(f"{hashlib.sha256((out / n).read_bytes()).hexdigest()}  {n}\n" for n in NAMES))
    r = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert r.returncode != 0 and f"{NAMES[2]}: live content" in r.stderr
    assert registered_names(tmp_path) == []


def test_register_resumes_an_interrupted_registration(tmp_path, shims):
    repo, out, commit = release_fixture(tmp_path)
    tag_object = git(repo, "rev-parse", f"refs/tags/v{VERSION}")
    state = tmp_path / "artifact-state.json"
    for name in NAMES[:2]:
        cid = "cid-" + hashlib.sha256((out / name).read_bytes()).hexdigest()[:16]
        state.write_text(state.read_text() + f"{tag_object} {cid} {name}\n" if state.exists() else f"{tag_object} {cid} {name}\n")
    r = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert r.returncode == 0, r.stderr
    assert registered_names(tmp_path) == [NAMES[2], "MANIFEST.json"]
    assert r.stdout.count("already registered") == 2


def test_register_refuses_unexpected_live_artifact_and_ambiguous_releases(tmp_path, shims):
    repo, out, commit = release_fixture(tmp_path)
    tag_object = git(repo, "rev-parse", f"refs/tags/v{VERSION}")
    state = tmp_path / "artifact-state.json"
    state.write_text(f"{tag_object} cid-stray stray.bin\n")
    r = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert r.returncode != 0 and "stray.bin: live artifact with an unexpected name" in r.stderr
    assert registered_names(tmp_path) == []
    two = {"releases": [{"releaseId": "a", "tag": tag_object, "artifacts": []}, {"releaseId": "b", "tag": tag_object, "artifacts": []}]}
    (tmp_path / "listing.json").write_text(json.dumps(two))
    r = run("register.sh", VERSION, cwd=repo, shims=shims, env={"ARTIFACT_LISTING": str(tmp_path / "listing.json")})
    assert r.returncode != 0 and "2 release objects" in r.stderr
    assert registered_names(tmp_path) == []


def test_write_manifest_is_deterministic_for_identical_bytes(tmp_path):
    import importlib.util, sys
    spec = importlib.util.spec_from_file_location("write_manifest", SCRIPTS / "write_manifest.py")
    wm = importlib.util.module_from_spec(spec); sys.modules[spec.name] = wm; spec.loader.exec_module(wm)
    out = tmp_path / "rel"; out.mkdir()
    for name in NAMES:
        (out / name).write_bytes(b"same bytes\n")
    (out / "SHA256SUMS").write_text("ignored\n")
    args = (out, VERSION, "c" * 40, "2026-10-22T00:00:00+00:00", "x86_64-unknown-linux-gnu", "img", "2.28", "26.5.0", ["rustc 1", "cargo 1", "maturin 1"])
    first = wm.render(wm.build(*args))
    (out / "MANIFEST.json").write_text(first)
    second = wm.render(wm.build(*args))
    assert first == second
    m = json.loads(first)
    assert "built_at" not in first and sorted(m["files"]) == sorted(NAMES) and m["targets"] == ["x86_64-unknown-linux-gnu"]
    assert m["files"][NAMES[0]]["targets"] == ["x86_64-unknown-linux-gnu"] and m["files"][NAMES[2]]["targets"] == ["x86_64-unknown-linux-gnu"]
    # Two targets: the macOS wheel is attributed by its platform tag and the
    # Mac's receipt is embedded under platforms; a target without a wheel or
    # without a receipt, or a receipt whose digests disagree, is an error.
    mac = f"radicle_py-{VERSION}-cp39-abi3-macosx_11_0_arm64.whl"; (out / mac).write_bytes(b"mac\n")
    both = "x86_64-unknown-linux-gnu,aarch64-apple-darwin"
    receipt = mac_receipt(out, mac, "radicle-sdk.darwin-arm64.node", source="c" * 40)
    receipt["files"]["radicle-sdk.darwin-arm64.node"] = {"sha256": "a" * 64, "bytes": 7}  # rides inside the npm tarball
    two = wm.build(out, VERSION, "c" * 40, "d", both, "img", "2.28", "26.5.0", [], [receipt])
    assert two["files"][mac]["targets"] == ["aarch64-apple-darwin"] and two["files"][NAMES[2]]["targets"] == ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
    assert two["platforms"]["aarch64-apple-darwin"]["toolchain"]["xcode"] == "Xcode 26.1" and two["platforms"]["aarch64-apple-darwin"]["files"] == sorted([mac, "radicle-sdk.darwin-arm64.node"])
    assert wm.render(two) == wm.render(wm.build(out, VERSION, "c" * 40, "d", both, "img", "2.28", "26.5.0", [], [receipt]))
    with pytest.raises(SystemExit, match="no platform receipt for aarch64-apple-darwin"):
        wm.build(out, VERSION, "c" * 40, "d", both, "img", "2.28", "26.5.0", [])
    bad = json.loads(json.dumps(receipt)); bad["files"][mac]["sha256"] = "0" * 64
    with pytest.raises(SystemExit, match="differs from the aarch64-apple-darwin receipt"):
        wm.build(out, VERSION, "c" * 40, "d", both, "img", "2.28", "26.5.0", [], [bad])
    bad = json.loads(json.dumps(receipt)); bad["source"] = "e" * 40
    with pytest.raises(SystemExit, match="is for"):
        wm.build(out, VERSION, "c" * 40, "d", both, "img", "2.28", "26.5.0", [], [bad])
    (out / mac).unlink()
    with pytest.raises(SystemExit, match="no wheel for target aarch64-apple-darwin"):
        wm.build(out, VERSION, "c" * 40, "d", both, "img", "2.28", "26.5.0", [], [receipt])


def mac_receipt(directory: Path, wheel: str, addon: str, *, source, version=VERSION) -> dict:
    """A complete receipt for the files of those names already in directory."""
    files = {n: {"sha256": hashlib.sha256((directory / n).read_bytes()).hexdigest(), "bytes": (directory / n).stat().st_size}
             for n in (wheel, addon) if (directory / n).is_file()}
    return {"schema": "radicle-sdks-platform-audit/1", "version": version, "source": source, "target": "aarch64-apple-darwin",
            "floors": {"macos_wheel": "11.0", "macos_addon": "13.5", "python": "3.9 (abi3)", "node": "26.5.0"},
            "toolchain": {"rustc": "rustc 1.91.0", "maturin": "maturin 1.9.4", "node_loader": "26.5.0", "xcode": "Xcode 26.1"},
            "files": files,
            "audit": {wheel: f"{wheel}: arm64 Mach-O, macOS minimum 11.0, system libraries only, signature verifies",
                      addon: f"{addon}: arm64 Mach-O, macOS minimum 13.5, system libraries only, signature verifies"}}


def staged_mac(tmp_path: Path, source="c" * 40):
    d = tmp_path / "incoming"; d.mkdir(exist_ok=True)
    wheel, addon = f"radicle_py-{VERSION}-cp39-abi3-macosx_11_0_arm64.whl", "radicle-sdk.darwin-arm64.node"
    (d / wheel).write_bytes(b"mac wheel\n"); (d / addon).write_bytes(b"mac addon\n")
    receipt = mac_receipt(d, wheel, addon, source=source)
    (d / "AUDIT.json").write_text(json.dumps(receipt))
    return d, wheel, addon, receipt


def receipt_cmd(*args):
    return subprocess.run(["python3", "-I", str(SCRIPTS / "platform_receipt.py"), *map(str, args)], capture_output=True, text=True)


def test_platform_receipt_check_accepts_a_complete_receipt_and_verifies_packaged_bytes(tmp_path):
    d, wheel, addon, _ = staged_mac(tmp_path)
    r = receipt_cmd("check", d, VERSION, "c" * 40)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines() == [f"  {wheel}: arm64 Mach-O, macOS minimum 11.0, system libraries only, signature verifies",
                                     f"  {addon}: arm64 Mach-O, macOS minimum 13.5, system libraries only, signature verifies"]
    packaged = tmp_path / "packaged.node"; packaged.write_bytes(b"mac addon\n")
    assert receipt_cmd("verify", d / "AUDIT.json", addon, packaged).returncode == 0
    packaged.write_bytes(b"mac addon tampered\n")
    r = receipt_cmd("verify", d / "AUDIT.json", addon, packaged)
    assert r.returncode == 1 and "differs from the receipt" in r.stderr
    r = receipt_cmd("verify", d / "AUDIT.json", "something-else.node", packaged)
    assert r.returncode == 1 and "does not list something-else.node" in r.stderr


@pytest.mark.parametrize("corruption, message", [
    ("empty-files", "expected exactly"),
    ("missing-addon", "expected exactly"),
    ("extra-entry", "expected exactly"),
    ("wrong-filename", "expected exactly"),
    ("changed-bytes", "sha256 .* differs from the receipt"),
    ("changed-size", "size .* differs"),
    ("file-missing", "missing"),
    ("no-digest", "no valid sha256"),
    ("no-audit-line", "audit lines cover"),
    ("failed-audit-line", "not a passing audit"),
    ("wrong-version", "receipt version is"),
    ("wrong-source", "receipt source is"),
    ("wrong-target", "not one the release assembles"),
    ("wrong-floor", "receipt floor macos_addon"),
    ("wrong-schema", "not a radicle-sdks-platform-audit/1 receipt"),
    ("stray-file", "unexpected files staged"),
    ("no-receipt", "cannot read receipt"),
])
def test_platform_receipt_check_refuses_incomplete_or_mismatched_receipts(tmp_path, corruption, message):
    d, wheel, addon, receipt = staged_mac(tmp_path)
    if corruption == "empty-files":
        receipt["files"] = {}; receipt["audit"] = {}
    elif corruption == "missing-addon":
        receipt["files"].pop(addon)
    elif corruption == "extra-entry":
        (d / "extra.whl").write_bytes(b"x"); receipt["files"]["extra.whl"] = {"sha256": "0" * 64, "bytes": 1}
    elif corruption == "wrong-filename":
        receipt["files"]["radicle_py-9.9.9-cp39-abi3-macosx_11_0_arm64.whl"] = receipt["files"].pop(wheel)
    elif corruption == "changed-bytes":
        (d / addon).write_bytes(b"other addon\n")  # same length
    elif corruption == "changed-size":
        receipt["files"][addon]["bytes"] += 1
    elif corruption == "file-missing":
        (d / addon).unlink()
    elif corruption == "no-digest":
        receipt["files"][wheel] = {"bytes": 10}
    elif corruption == "no-audit-line":
        receipt["audit"].pop(addon)
    elif corruption == "failed-audit-line":
        receipt["audit"][addon] = f"{addon}: macOS minimum 14.0 exceeds 13.5"
    elif corruption == "wrong-version":
        receipt["version"] = "9.9.9"
    elif corruption == "wrong-source":
        receipt["source"] = "e" * 40
    elif corruption == "wrong-target":
        receipt["target"] = "x86_64-apple-darwin"
    elif corruption == "wrong-floor":
        receipt["floors"]["macos_addon"] = "14.0"
    elif corruption == "wrong-schema":
        receipt["schema"] = "something/2"
    elif corruption == "stray-file":
        (d / "radicle-sdk.linux-x64-gnu.node").write_bytes(b"planted")
    elif corruption == "no-receipt":
        (d / "AUDIT.json").unlink()
    if corruption != "no-receipt":
        (d / "AUDIT.json").write_text(json.dumps(receipt))
    r = receipt_cmd("check", d, VERSION, "c" * 40)
    assert r.returncode == 1, (r.stdout, r.stderr)
    assert re.search(message, r.stderr), r.stderr


@pytest.mark.parametrize("corruption", ["version", "hash", "extra-file", "remote-tag", "stale-source", "unpushed", "only-on-fetch-url"])
def test_register_refuses_before_any_registration(tmp_path, shims, corruption):
    repo, out, commit = release_fixture(tmp_path)
    if corruption == "version":
        m = json.loads((out / "MANIFEST.json").read_text()); m["version"] = "9.9.9"
        (out / "MANIFEST.json").write_text(json.dumps(m))
    elif corruption == "hash":
        (out / NAMES[2]).write_bytes(b"rebuilt but manifest not updated\n")
    elif corruption == "extra-file":
        m = json.loads((out / "MANIFEST.json").read_text()); m["files"]["stray.bin"] = {"sha256": "0" * 64, "bytes": 1}
        (out / "MANIFEST.json").write_text(json.dumps(m))
    elif corruption == "remote-tag":
        git(repo, "tag", "-d", f"v{VERSION}")
        git(repo, "tag", "-a", f"v{VERSION}", "-m", "retagged locally")  # different tag object than the remote
    elif corruption == "stale-source":
        (repo / "file").write_text("y\n"); git(repo, "commit", "-am", "later")
        git(repo, "tag", "-d", f"v{VERSION}"); git(repo, "tag", "-a", f"v{VERSION}", "-m", "moved")
        git(repo, "push", "-q", "-f", "rad", f"v{VERSION}")  # tag now points past the built commit
    elif corruption == "unpushed":
        git(repo, "push", "-q", "rad", "--delete", f"v{VERSION}")
    elif corruption == "only-on-fetch-url":
        # Tag present on the canonical side only, absent from the publisher's namespace.
        git(repo, "push", "-q", "rad", "--delete", f"v{VERSION}")
        git(repo, "push", "-q", str(tmp_path / "canonical.git"), f"v{VERSION}")
    result = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert result.returncode != 0, result.stdout
    assert not (tmp_path / "calls.log").exists() or " register " not in (tmp_path / "calls.log").read_text()


def test_audit_elf_gates(tmp_path):
    shims = tmp_path / "shims"; shims.mkdir()
    target = tmp_path / "lib.so"; target.write_bytes(b"not really an elf")

    def tools(arch="i386:x86-64", floor="2.28", libs="libc.so.6 => /lib/libc.so.6 (0x1)\n\t/lib64/ld-linux-x86-64.so.2 (0x2)\n\tlinux-vdso.so.1 (0x3)", symbols_status=0):
        shim(shims, "objdump", f"""
case "$1" in
  -f) printf 'architecture: {arch}, flags 0x00000150:\\n' ;;
  -T) printf 'x GLIBC_2.17 y\\nx GLIBC_{floor} z\\n'; exit {symbols_status} ;;
esac
""")
        shim(shims, "ldd", f"printf '\\t{libs}\\n'\n")

    def audit():
        return subprocess.run(["bash", str(SCRIPTS / "audit-elf.sh"), str(target), "2.28"],
                              env=dict(os.environ, PATH=f"{shims}:{os.environ['PATH']}"), capture_output=True, text=True)

    tools(); assert audit().returncode == 0
    tools(floor="2.39"); r = audit(); assert r.returncode == 1 and "exceeds 2.28" in r.stderr
    tools(arch="aarch64"); r = audit(); assert r.returncode == 1 and "architecture" in r.stderr
    tools(libs="libz.so.1 => /lib/libz.so.1 (0x1)\n\tlibc.so.6 => /lib/libc.so.6 (0x2)"); r = audit()
    assert r.returncode == 1 and "unexpected dynamic libraries: libz.so.1" in r.stderr
    tools(libs="libfoo.so.1 => not found"); r = audit(); assert r.returncode == 1 and "unresolved" in r.stderr
    tools(symbols_status=1); r = audit(); assert r.returncode == 1 and "objdump -T failed" in r.stderr


def listing(tag, names, tag_oid="a" * 40):
    return json.dumps({"releases": [{"releaseId": "rel1", "tag": tag_oid, "tagName": tag,
        "artifacts": [{"cid": f"cid-{n}", "name": n, "redactions": []} for n in names]}]})


def mock_manifest(source: Path, version=VERSION, targets=("x86_64-unknown-linux-gnu",), names=NAMES, digests=True):
    files = {n: {"sha256": hashlib.sha256((source / n).read_bytes()).hexdigest() if digests else "0" * 64, "bytes": 1}
             for n in names}
    (source / "MANIFEST.json").write_text(json.dumps({"version": version, "source": "s", "targets": list(targets), "files": files}))


def test_verify_installed_refuses_wrong_release_shapes(tmp_path, shims):
    source = tmp_path / "source"; source.mkdir()
    for name in NAMES:
        (source / name).write_bytes(b"x")
    mock_manifest(source)
    env = {"ARTIFACT_SOURCE": str(source), "ARTIFACT_LISTING": str(tmp_path / "listing.json")}
    # A v0.2.0 release whose files are the 0.1.0 artifacts must not verify as 0.2.0.
    (tmp_path / "listing.json").write_text(listing("v0.2.0", NAMES + ["MANIFEST.json"]))
    r = run("verify-installed.sh", "0.2.0", RID, cwd=tmp_path, shims=shims, env=env)
    assert r.returncode != 0 and "MANIFEST.json is for 0.1.0, expected 0.2.0" in r.stderr
    # Extra artifact in the right release.
    (tmp_path / "listing.json").write_text(listing(f"v{VERSION}", NAMES + ["MANIFEST.json", "stray.bin"]))
    r = run("verify-installed.sh", VERSION, RID, cwd=tmp_path, shims=shims, env=env)
    assert r.returncode != 0 and "stray.bin" in r.stderr
    # Two releases bound to the same tag.
    two = json.loads(listing(f"v{VERSION}", NAMES + ["MANIFEST.json"]))
    two["releases"].append(dict(two["releases"][0], releaseId="rel2"))
    (tmp_path / "listing.json").write_text(json.dumps(two))
    r = run("verify-installed.sh", VERSION, RID, cwd=tmp_path, shims=shims, env=env)
    assert r.returncode != 0 and "exactly one" in r.stderr
    # Two live manifests on one release (the duplicate-name case).
    dup = json.loads(listing(f"v{VERSION}", NAMES + ["MANIFEST.json", "MANIFEST.json"]))
    (tmp_path / "listing.json").write_text(json.dumps(dup))
    r = run("verify-installed.sh", VERSION, RID, cwd=tmp_path, shims=shims, env=env)
    assert r.returncode != 0 and "2 live MANIFEST.json" in r.stderr
    # Manifest does not cover this machine's target.
    (tmp_path / "listing.json").write_text(listing(f"v{VERSION}", NAMES + ["MANIFEST.json"]))
    mock_manifest(source, targets=("aarch64-apple-darwin",))
    r = run("verify-installed.sh", VERSION, RID, cwd=tmp_path, shims=shims, env=env)
    assert r.returncode != 0 and "does not cover" in r.stderr
    # Right manifest, wrong bytes.
    mock_manifest(source, digests=False)
    r = run("verify-installed.sh", VERSION, RID, cwd=tmp_path, shims=shims, env=env)
    assert r.returncode != 0 and "differs from manifest" in r.stderr


@pytest.mark.skipif(not (ROOT / "release" / VERSION / "MANIFEST.json").exists(), reason="no built release directory")
def test_verify_installed_accepts_a_real_build(tmp_path, shims):
    if shutil.which("uv") is None:
        pytest.skip("uv unavailable")
    source = ROOT / "release" / VERSION
    env = {"ARTIFACT_SOURCE": str(source), "ARTIFACT_LISTING": str(tmp_path / "listing.json")}
    (tmp_path / "listing.json").write_text(listing(f"v{VERSION}", NAMES + ["MANIFEST.json"]))
    r = run("verify-installed.sh", VERSION, RID, cwd=tmp_path, shims=shims, env=env)
    assert r.returncode == 0, r.stderr + r.stdout
    assert f"release {VERSION} verified" in r.stdout


def test_register_expects_the_macos_wheel_when_the_manifest_covers_darwin(tmp_path, shims):
    repo, out, commit = release_fixture(tmp_path)
    mac = f"radicle_py-{VERSION}-cp39-abi3-macosx_11_0_arm64.whl"
    data = b"mac wheel bytes\n"; (out / mac).write_bytes(data)
    rewrite_manifest(out, lambda m: (m.__setitem__("targets", ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]),
                                     m["files"].__setitem__(mac, {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)})))
    (out / "SHA256SUMS").write_text("".join(f"{hashlib.sha256((out / n).read_bytes()).hexdigest()}  {n}\n" for n in NAMES + [mac]))
    r = run("register.sh", VERSION, cwd=repo, shims=shims)
    assert r.returncode == 0, r.stderr
    assert registered_names(tmp_path) == [NAMES[0], mac] + NAMES[1:] + ["MANIFEST.json"]


def test_audit_macho_gates(tmp_path):
    shims = tmp_path / "shims"; shims.mkdir()
    target = tmp_path / "lib.node"; target.write_bytes(b"not really mach-o")
    system = ["LC_LOAD_DYLIB /usr/lib/libSystem.B.dylib", "LC_LOAD_DYLIB /System/Library/Frameworks/Security.framework/Versions/A/Security"]

    def load_commands(minos, loads, identity="@rpath/libradicle_sdk_native.dylib"):
        # The shape `otool -l` prints: an MH_DYLIB such as a Rust cdylib carries its own
        # install name (LC_ID_DYLIB), which is not a dependency.
        text = f"Load command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform 1\n    minos {minos}\n      sdk 15.0\n"
        text += f"Load command 10\n          cmd LC_ID_DYLIB\n      cmdsize 56\n         name {identity} (offset 24)\n"
        for i, load in enumerate(loads):
            cmd, name = load.split()
            text += f"Load command {11 + i}\n          cmd {cmd}\n      cmdsize 56\n         name {name} (offset 24)\n   time stamp 2\n      current version 1.0.0\ncompatibility version 1.0.0\n"
        return text

    def tools(kind="Mach-O 64-bit dynamically linked shared library arm64", minos="13.5", loads=system, sign=0, **kw):
        shim(shims, "file", f"printf '%s\\n' '{kind}'\n")
        (shims / "otool-l.txt").write_text(load_commands(minos, loads, **kw))
        shim(shims, "otool", f'[ "$1" = -l ] && cat "{shims}/otool-l.txt"\n')
        shim(shims, "codesign", f"exit {sign}\n")

    def audit(max_="13.5"):
        return subprocess.run(["bash", str(SCRIPTS / "audit-macho.sh"), str(target), max_],
                              env=dict(os.environ, PATH=f"{shims}:{os.environ['PATH']}"), capture_output=True, text=True)

    tools(); r = audit(); assert r.returncode == 0, r.stderr
    tools(identity="/Users/x/radicle-py/target/aarch64-apple-darwin/release/deps/libradicle_py.dylib"); assert audit().returncode == 0
    tools(minos="11.0"); assert audit().returncode == 0            # lower minimum than the floor is fine
    tools(minos="14.0"); r = audit(); assert r.returncode == 1 and "exceeds 13.5" in r.stderr
    tools(kind="Mach-O 64-bit dynamically linked shared library x86_64"); r = audit(); assert r.returncode == 1 and "arm64" in r.stderr
    tools(loads=system + ["LC_LOAD_DYLIB /opt/homebrew/lib/libz.1.dylib"]); r = audit()
    assert r.returncode == 1 and "unexpected dynamic libraries: /opt/homebrew/lib/libz.1.dylib" in r.stderr
    tools(loads=system + ["LC_LOAD_DYLIB @rpath/libfoo.dylib"]); r = audit(); assert r.returncode == 1 and "@rpath/libfoo.dylib" in r.stderr
    tools(loads=system + ["LC_LOAD_WEAK_DYLIB @rpath/libweak.dylib"]); r = audit(); assert r.returncode == 1 and "@rpath/libweak.dylib" in r.stderr
    tools(loads=system + ["LC_REEXPORT_DYLIB /opt/homebrew/lib/libre.dylib"]); r = audit(); assert r.returncode == 1 and "libre.dylib" in r.stderr
    tools(loads=[]); r = audit(); assert r.returncode == 1 and "no dylib load commands" in r.stderr
    tools(sign=1); r = audit(); assert r.returncode == 1 and "signature" in r.stderr


@pytest.mark.parametrize("have, ok", [("26.5.0", True), ("26.6.1", True), ("27.0.0", True), ("26.4.9", False), ("25.2.0", False), ("20.19.0", False)])
def test_check_node_floor_compares_major_then_minor(tmp_path, have, ok):
    shims = tmp_path / "shims"; shims.mkdir()
    shim(shims, "node", f"printf '%s\\n' '{have}'\n")
    r = subprocess.run(["bash", str(SCRIPTS / "check-node-floor.sh")],
                       env=dict(os.environ, PATH=f"{shims}:{os.environ['PATH']}"), capture_output=True, text=True)
    assert (r.returncode == 0) == ok, r.stderr
    if not ok:
        assert f"Node {have} is below the 26.5.0 floor" in r.stderr


def test_build_scripts_gate_on_the_node_floor_before_building(tmp_path):
    # Both build scripts call the gate right after the clean-tree check, so a
    # Node below the floor stops the run before any toolchain work.
    for script in ("build-linux-x86_64.sh", "build-macos-arm64.sh"):
        lines = (SCRIPTS / script).read_text().splitlines()
        gate = next(i for i, l in enumerate(lines) if "check-node-floor.sh" in l)
        assert lines[gate].strip() == 'bash "$here/check-node-floor.sh"'
        assert "git status --porcelain" in lines[gate - 1], script
