#!/usr/bin/env bash
# Build the macOS arm64 release pieces of the checked-out commit on an Apple
# Silicon Mac, gate them, and stage them for assembly on the Linux build host.
#
#   scripts/release/build-macos-arm64.sh
#
# Output under release/incoming/darwin-arm64/: the abi3 wheel
# (macosx_11_0_arm64), the addon radicle-sdk.darwin-arm64.node, and
# AUDIT.json (source commit, version, floors, toolchain identity, SHA-256 of
# both files, audit lines), checked with platform_receipt.py exactly as the
# Linux assembly will check it. Copy that directory to the Linux host's
# release/incoming/ before running build-linux-x86_64.sh there; it folds the
# wheel into the release and the addon into the npm tarball, and the manifest
# then covers both targets.
#
# Floors: the wheel is built for macOS 11.0 (maturin's arm64 default); the
# addon for macOS 13.5, which is Node 26's own minimum. Each is audited against
# its floor by audit-macho.sh. Gates, each fatal: clean tree; one version in
# all four manifests; current THIRD-PARTY-NOTICES.md; pinned Rust and maturin;
# both audits; a packed tarball carrying this addon passes the packaged
# consumer smoke with no compiler; the wheel installs wheel-only into a clean
# environment at the expected version and passes the wheel smoke.
#
# Prerequisites: Xcode command line tools, rustup with the pinned toolchain
# and the aarch64-apple-darwin target, Node 26.5+, uv (project synced with
# `uv sync --locked`), Docker not required.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
here=$(cd "$(dirname "$0")" && pwd)
[ "$(uname -sm)" = "Darwin arm64" ] || { echo "run this on an Apple Silicon Mac (uname -sm: $(uname -sm))" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "working tree must be clean" >&2; exit 1; }
bash "$here/check-node-floor.sh"
version=$(node -p "require('./package.json').version")
for manifest in pyproject.toml python/native/Cargo.toml node/native/Cargo.toml; do
  grep -q "^version = \"$version\"" "$manifest" || { echo "$manifest version differs from package.json ($version)" >&2; exit 1; }
done
uv run --no-sync python scripts/release/third_party_notices.py --check
rust_version=1.91.0
maturin_version=1.9.4
target=aarch64-apple-darwin
wheel_floor=11.0
addon_floor=13.5
source=$(git rev-parse HEAD)
out="release/incoming/darwin-arm64"
wheel_name="radicle_py-$version-cp39-abi3-macosx_${wheel_floor/./_}_arm64.whl"
addon_name="radicle-sdk.darwin-arm64.node"
rm -rf "$out"; mkdir -p "$out"
export RUSTUP_TOOLCHAIN=$rust_version
rustup target list --installed --toolchain "$rust_version" | grep -q "^$target$" || rustup target add "$target" --toolchain "$rust_version"
rustc --version | grep -q " $rust_version " || { echo "rustc is not $rust_version" >&2; exit 1; }
xcode=$(xcodebuild -version 2>/dev/null | tr '\n' ' ' || clang --version | head -1)

echo "== wheel (macOS $wheel_floor floor, maturin $maturin_version)"
MACOSX_DEPLOYMENT_TARGET=$wheel_floor LIBZ_SYS_STATIC=1 uvx --from "maturin==$maturin_version" maturin build --release --locked --target "$target" --out "$out"
[ -f "$out/$wheel_name" ] || { echo "expected wheel $wheel_name, found: $(ls "$out")" >&2; exit 1; }

echo "== addon (macOS $addon_floor floor)"
rm -rf node/dist
MACOSX_DEPLOYMENT_TARGET=$addon_floor LIBZ_SYS_STATIC=1 npx napi build --release --platform --manifest-path node/native/Cargo.toml \
  --output-dir node/dist --js native.cjs --dts native.d.cts --target "$target" -- --locked
npx tsc -p node/tsconfig.json
cp "node/dist/$addon_name" "$out/$addon_name"

echo "== audit"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/whl" && (cd "$tmp/whl" && unzip -q "$root/$out/$wheel_name")
wheel_so=$(ls "$tmp"/whl/radiclepy/*.so)
audit_wheel=$(bash "$here/audit-macho.sh" "$wheel_so" "$wheel_floor")
audit_addon=$(bash "$here/audit-macho.sh" "$out/$addon_name" "$addon_floor")
echo "$audit_wheel"; echo "$audit_addon"
[ -f "$tmp"/whl/*.dist-info/licenses/THIRD-PARTY-NOTICES.md ] || { echo "wheel lacks THIRD-PARTY-NOTICES.md" >&2; exit 1; }

echo "== packaged consumer smoke (Node) with this addon"
npm pack --ignore-scripts --pack-destination "$tmp" >/dev/null
bash node/scripts/tarball-install-smoke.sh "$tmp/radicle-sdk-$version.tgz"

echo "== wheel-only install into a clean environment, no compiler"
uv venv -q "$tmp/venv"
RUSTC=/no-rust CARGO=/no-cargo uv pip install -q --python "$tmp/venv/bin/python" --no-index --only-binary :all: --find-links "$out" "radicle-py==$version"
"$tmp/venv/bin/python" -c 'import radiclepy, sys; sys.exit(0 if radiclepy.__version__ == sys.argv[1] else f"installed {radiclepy.__version__}, expected {sys.argv[1]}")' "$version"
"$tmp/venv/bin/python" python/tests/wheel_smoke.py
uv pip install -q --python "$tmp/venv/bin/python" 'pytest>=8'
"$tmp/venv/bin/python" -m pytest -q python/tests/test_ux.py python/tests/test_value_semantics.py python/tests/test_readme.py

echo "== AUDIT.json"
python3 - "$out" "$version" "$source" "$target" "$wheel_floor" "$addon_floor" "$wheel_name" "$addon_name" "$(rustc --version)" "maturin $maturin_version" "$(node -p process.versions.node)" "$xcode" "$audit_wheel" "$audit_addon" <<'PY'
import hashlib, json, sys
from pathlib import Path
out, version, source, target, wheel_floor, addon_floor, wheel, addon, rustc, maturin, node, xcode, a_wheel, a_addon = sys.argv[1:15]
d = Path(out)
files = {n: {"sha256": hashlib.sha256((d / n).read_bytes()).hexdigest(), "bytes": (d / n).stat().st_size} for n in (wheel, addon)}
audit = {"schema": "radicle-sdks-platform-audit/1", "version": version, "source": source, "target": target,
         "floors": {"macos_wheel": wheel_floor, "macos_addon": addon_floor, "python": "3.9 (abi3)", "node": "26.5.0"},
         "toolchain": {"rustc": rustc, "maturin": maturin, "node_loader": node, "xcode": xcode.strip()},
         "files": files, "audit": {wheel: a_wheel, addon: a_addon}}
(d / "AUDIT.json").write_text(json.dumps(audit, indent=2, sort_keys=True) + "\n")
print(json.dumps(audit, indent=2, sort_keys=True))
PY
echo "== receipt check (as the Linux assembly runs it)"
python3 scripts/release/platform_receipt.py check "$out" "$version" "$source" >/dev/null
echo "staged $out from $source: copy this directory to the Linux host's release/incoming/ and run build-linux-x86_64.sh there"
