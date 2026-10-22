#!/usr/bin/env bash
# Download a released version through rad-artifact, verify every file, and
# install both packages from the downloaded files in clean environments with
# no compiler available. Run on any machine that seeds the repository in local
# Radicle storage and has a running rad-artifact node, Python 3.9+, uv and
# Node 26.5+.
#
#   scripts/release/verify-installed.sh <version> [rid]
#
# Gates, each fatal: exactly one release is bound to tag v<version>; its
# non-redacted artifacts are exactly the expected names for this version;
# every downloaded file verifies against the release object and matches the
# digest in the downloaded MANIFEST.json; the wheel installs wheel-only and
# reports this version; the npm tarball installs with scripts disabled and its
# package metadata reports this version.
set -euo pipefail
version=${1:?usage: verify-installed.sh <version> [rid]}
rid=${2:-$(rad .)}
tag="v$version"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
rad-artifact node status >/dev/null 2>&1 || { echo "start the seeder first: rad-artifact node start" >&2; exit 1; }
wheel="radicle_py-$version-cp39-abi3-manylinux_2_28_x86_64.whl"
sdist="radicle_py-$version.tar.gz"
tarball="radicle-sdk-$version.tgz"
rad-artifact --repo "$rid" list --json > listing.json
python3 - "$tag" "$wheel" "$sdist" "$tarball" > artifacts.txt <<'PY'
import json, sys
tag, *expected = sys.argv[1:]
expected = sorted(expected + ["MANIFEST.json"])
releases = [r for r in json.load(open("listing.json"))["releases"] if r.get("tagName") == tag]
if len(releases) != 1:
    sys.exit(f"{len(releases)} releases are bound to {tag}; expected exactly one")
r = releases[0]
live = [a for a in r["artifacts"] if not a["redactions"]]
names = sorted(a["name"] for a in live)
if names != expected:
    sys.exit(f"release {tag} lists {names}, expected {expected}")
for a in live:
    print(r["tag"], a["cid"], a["name"])
PY
cat artifacts.txt
while read -r revision cid name; do
  rad-artifact --repo "$rid" --no-input download "$revision" --cid "$cid" -o "$name" >/dev/null
  rad-artifact --repo "$rid" verify "$name" --json >/dev/null   # exit 2 when the bytes match no registered artifact
done < artifacts.txt
python3 - "$version" "$wheel" "$sdist" "$tarball" <<'PY'
import hashlib, json, sys
version, *names = sys.argv[1:]
m = json.load(open("MANIFEST.json"))
if m["version"] != version:
    sys.exit(f"MANIFEST.json is for {m['version']}, expected {version}")
if sorted(m["files"]) != sorted(names):
    sys.exit(f"MANIFEST.json lists {sorted(m['files'])}, expected {sorted(names)}")
for name, entry in m["files"].items():
    digest = hashlib.sha256(open(name, "rb").read()).hexdigest()
    if digest != entry["sha256"]:
        sys.exit(f"{name}: downloaded sha256 {digest} differs from manifest {entry['sha256']}")
print(f"manifest: version {version}, source {m['source']}, target {m['target']}")
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
