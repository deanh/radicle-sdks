#!/usr/bin/env bash
# Download a released version through rad-artifact, verify every file, and
# install both packages from the downloaded files in clean environments with
# no compiler available. Run on any machine that seeds the repository in local
# Radicle storage and has a running rad-artifact node, Python 3.9+, uv and
# Node 26.5+.
#
#   scripts/release/verify-installed.sh <version> [rid]
#
# Gates, each fatal: exactly one release is bound to tag v<version> with
# exactly one live MANIFEST.json; that manifest names this version and covers
# this machine's target; the live artifact set is exactly the files the
# manifest lists; every downloaded file verifies against the release object
# and matches the manifest digest; this platform's wheel installs wheel-only
# and reports this version; the npm tarball installs with scripts disabled and
# its package metadata reports this version. Runs on Linux x86_64 and macOS
# arm64.
set -euo pipefail
version=${1:?usage: verify-installed.sh <version> [rid]}
rid=${2:-$(rad .)}
tag="v$version"
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
case "$(uname -sm)" in
  "Linux x86_64") wheel="radicle_py-$version-cp39-abi3-manylinux_2_28_x86_64.whl"; want_target=x86_64-unknown-linux-gnu ;;
  "Darwin arm64") wheel="radicle_py-$version-cp39-abi3-macosx_11_0_arm64.whl"; want_target=aarch64-apple-darwin ;;
  *) echo "no release target for $(uname -sm)" >&2; exit 1 ;;
esac
sdist="radicle_py-$version.tar.gz"
tarball="radicle-sdk-$version.tgz"
bash "$here/fetch-manifest.sh" "$version" "$rid"
python3 - "$version" "$want_target" "$wheel" "$sdist" "$tarball" > artifacts.txt <<'PY'
import json, sys
version, want_target, wheel, sdist, tarball = sys.argv[1:6]
m = json.load(open("MANIFEST.json"))
if m["version"] != version:
    sys.exit(f"MANIFEST.json is for {m['version']}, expected {version}")
targets = m.get("targets") or [m["target"]]  # manifests before 0.1.1 carried a single target
if want_target not in targets:
    sys.exit(f"release {version} does not cover {want_target} (targets: {targets})")
expected = sorted(list(m["files"]) + ["MANIFEST.json"])
for name in (wheel, sdist, tarball):
    if name not in expected:
        sys.exit(f"MANIFEST.json does not list {name}")
live = {}
for line in open("release.txt").read().splitlines()[1:]:
    rev, cid, name = line.split(" ", 2)
    live.setdefault(name, []).append((rev, cid))
if sorted(live) != expected or any(len(v) != 1 for v in live.values()):
    sys.exit(f"release lists {sorted(live)}, expected exactly {expected}")
for name, [(rev, cid)] in live.items():
    if name != "MANIFEST.json":
        print(rev, cid, name)
PY
cat artifacts.txt
while read -r revision cid name; do
  rad-artifact --repo "$rid" --no-input download "$revision" --cid "$cid" -o "$name" >/dev/null
  rad-artifact --repo "$rid" verify "$name" --json >/dev/null   # exit 2 when the bytes match no registered artifact
done < artifacts.txt
python3 - <<'PY'
import hashlib, json, sys
m = json.load(open("MANIFEST.json"))
for name, entry in m["files"].items():
    digest = hashlib.sha256(open(name, "rb").read()).hexdigest()
    if digest != entry["sha256"]:
        sys.exit(f"{name}: downloaded sha256 {digest} differs from manifest {entry['sha256']}")
print(f"manifest: version {m['version']}, source {m['source']}, targets {m.get('targets') or [m['target']]}")
PY
echo "== python (wheel only, no compiler)"
uv venv -q venv
RUSTC=/no-rust CARGO=/no-cargo uv pip install -q --python venv/bin/python --no-index --only-binary :all: --find-links . "radicle-py==$version"
venv/bin/python -c 'import radiclepy, sys; v = radiclepy.__version__; sys.exit(0 if v == sys.argv[1] else f"installed radiclepy {v}, expected {sys.argv[1]}"); ' "$version"
venv/bin/python -c 'import radiclepy; print("radiclepy", radiclepy.__version__, "imports")'
echo "== node (tarball, scripts disabled, no compiler)"
mkdir consumer && cd consumer
printf '{"name":"release-consumer","version":"1.0.0","private":true,"type":"module"}\n' > package.json
RUSTC=/no-rust CARGO=/no-cargo npm install --ignore-scripts "../$tarball" >/dev/null
node --input-type=module -e "
import { readFileSync } from 'node:fs';
import { Profile, parseRepoId } from 'radicle-sdk';
const pkg = JSON.parse(readFileSync('node_modules/radicle-sdk/package.json', 'utf8'));
if (pkg.version !== process.argv[1]) throw new Error('installed radicle-sdk ' + pkg.version + ', expected ' + process.argv[1]);
console.log('radicle-sdk', pkg.version, 'imports', parseRepoId(process.argv[2]), typeof Profile.load);
" "$version" "$rid"
echo "release $version verified from download on $(uname -sm)"
