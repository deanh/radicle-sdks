#!/usr/bin/env bash
# Verify that PyPI and npm serve exactly the bytes a release's rad-artifact
# receipt records, then install both packages from the registries into clean
# environments with no compiler. Run on each released platform, on a machine
# that seeds the repository in local Radicle storage and has a running
# rad-artifact node, Python 3.9+, uv and Node 26.5+.
#
#   scripts/release/verify-registries.sh <version> [rid]
#
# Gates, each fatal: the receipt resolves (exactly one release on the tag,
# exactly one live MANIFEST.json, verified, for this version, covering this
# machine's target); PyPI holds exactly the manifest's Python files with the
# manifest's digests and every download matches; npm reports the manifest's
# tarball integrity and the tarball fetched from npm matches the manifest
# digest; this platform's wheel installs from PyPI wheel-only (no local files,
# no cache) through a hash-pinned requirement, so the installer itself refuses
# any bytes the receipt does not attest, and reports this version; the package
# installs from npm with `npm ci` against a lockfile pinned to the receipt's
# integrity, scripts disabled, and reports this version. A registry lookup
# that fails for any reason other than an explicit not-found is fatal.
set -euo pipefail
version=${1:?usage: verify-registries.sh <version> [rid]}
rid=${2:-$(rad .)}
here=$(cd "$(dirname "$0")" && pwd)
pypi_project=radicle-py
npm_package=radicle-sdk
pypi_url=${PYPI_URL:-https://pypi.org}
npm_registry=${NPM_REGISTRY:-https://registry.npmjs.org}
# shellcheck source=registry_lookup.sh
. "$here/registry_lookup.sh"
case "$(uname -sm)" in
  "Linux x86_64") want_target=x86_64-unknown-linux-gnu ;;
  "Darwin arm64") want_target=aarch64-apple-darwin ;;
  *) echo "no release target for $(uname -sm)" >&2; exit 1 ;;
esac
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
cd "$work"
echo "== receipt"
bash "$here/fetch-manifest.sh" "$version" "$rid"
python3 - "$want_target" <<'PY'
import json, sys
m = json.load(open("MANIFEST.json"))
targets = m.get("targets") or [m["target"]]
if sys.argv[1] not in targets:
    sys.exit(f"release {m['version']} does not cover {sys.argv[1]} (targets: {targets})")
print(f"manifest: version {m['version']}, source {m['source']}, targets {targets}")
PY
tarball="$npm_package-$version.tgz"

echo "== PyPI"
pypi_state=$(pypi_lookup pypi.json)
[ "$pypi_state" = present ] || { echo "PyPI has no $pypi_project $version" >&2; exit 1; }
python3 "$here/registry_state.py" pypi-files MANIFEST.json pypi.json > pypi-files
mkdir pypi
while read -r url name sha; do
  curl -sSf -o "pypi/$name" "$url"
  got=$(sha256sum "pypi/$name" | awk '{print $1}')
  [ "$got" = "$sha" ] || { echo "$name: PyPI served sha256 $got, manifest says $sha" >&2; exit 1; }
  echo "$name matches the receipt"
done < pypi-files

echo "== npm"
export npm_config_cache="$work/npm-cache-fetch"   # never the host's cache: every byte here comes from the registry
reported=$(npm_lookup)
[ "$reported" != absent ] || { echo "npm has no $npm_package@$version" >&2; exit 1; }
mkdir npm
(cd npm && npm pack "$npm_package@$version" --registry "$npm_registry" --pack-destination . >/dev/null 2>&1) || true
[ -f "npm/$tarball" ] || { echo "npm has no $npm_package@$version ($tarball not fetched)" >&2; exit 1; }
python3 "$here/registry_state.py" npm-check MANIFEST.json "$reported" "npm/$tarball" >/dev/null
want=$(python3 -c 'import json,sys; print(json.load(open("MANIFEST.json"))["files"][sys.argv[1]]["sha256"])' "$tarball")
got=$(sha256sum "npm/$tarball" | awk '{print $1}')
[ "$got" = "$want" ] || { echo "$tarball: npm served sha256 $got, manifest says $want" >&2; exit 1; }
echo "$tarball matches the receipt (integrity $reported)"

echo "== python (wheel only from PyPI, hash-pinned to the receipt, no compiler, no cache)"
python3 "$here/registry_state.py" pypi-requirement MANIFEST.json "$pypi_project" > requirements.txt
uv venv -q venv
RUSTC=/no-rust CARGO=/no-cargo uv pip install -q --python venv/bin/python --no-cache --only-binary :all: --require-hashes --index-url "$pypi_url/simple/" -r requirements.txt
venv/bin/python -c 'import radiclepy, sys; v = radiclepy.__version__; sys.exit(0 if v == sys.argv[1] else f"installed radiclepy {v}, expected {sys.argv[1]}")' "$version"
venv/bin/python -c 'import radiclepy; print("radiclepy", radiclepy.__version__, "imports")'

echo "== node (from npm, lockfile pinned to the receipt, scripts disabled, no compiler)"
mkdir consumer && cd consumer
printf '{"name":"release-consumer","version":"1.0.0","private":true,"type":"module","dependencies":{"%s":"%s"}}\n' "$npm_package" "$version" > package.json
python3 "$here/registry_state.py" npm-lock ../MANIFEST.json "$npm_registry" "$npm_package" release-consumer > package-lock.json
RUSTC=/no-rust CARGO=/no-cargo npm_config_cache="$work/npm-cache-install" npm ci --ignore-scripts --registry "$npm_registry" >/dev/null
node --input-type=module -e "
import { readFileSync } from 'node:fs';
import { Profile, parseRepoId } from 'radicle-sdk';
const pkg = JSON.parse(readFileSync('node_modules/radicle-sdk/package.json', 'utf8'));
if (pkg.version !== process.argv[1]) throw new Error('installed radicle-sdk ' + pkg.version + ', expected ' + process.argv[1]);
console.log('radicle-sdk', pkg.version, 'imports', parseRepoId(process.argv[2]), typeof Profile.load);
" "$version" "$rid"
echo "release $version verified from PyPI and npm against its receipt on $(uname -sm)"
