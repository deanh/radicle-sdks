#!/usr/bin/env bash
# Build the Linux x86_64 release artifacts of the checked-out commit with a
# glibc 2.28 floor, gate them, and write a build manifest.
#
#   scripts/release/build-linux-x86_64.sh
#
# Output under release/<version>/: the abi3 wheel (manylinux_2_28, auditwheel
# repaired), the sdist, the npm tarball carrying the Linux x86_64 addon,
# MANIFEST.json (source commit and date, version, target, toolchain, file
# digests; deterministic for identical artifact bytes, no build timestamp) and
# SHA256SUMS. Both native libraries are compiled inside the official
# manylinux_2_28 container with a pinned Rust toolchain, so they share one
# toolchain and one glibc floor. The host generates the JavaScript loader and
# runs the gates; it needs Docker, Node, uv, objdump, ldd, unzip and tar.
#
# Gates, each fatal: clean tree; one version in all four manifests; current
# THIRD-PARTY-NOTICES.md; every native library extracted from the final
# packaged files audits as x86-64 with a glibc floor of 2.28 or lower and no
# extra dynamic libraries; the notices ship in both packages; the exact release
# tarball passes the packaged-consumer smoke with no compiler; the wheel
# installs wheel-only into a clean environment at the expected version and
# passes the wheel smoke. Toolchain and build caches persist under target/docker-*.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
here=$(cd "$(dirname "$0")" && pwd)
[ -z "$(git status --porcelain)" ] || { echo "working tree must be clean" >&2; exit 1; }
version=$(node -p "require('./package.json').version")
for manifest in pyproject.toml python/native/Cargo.toml node/native/Cargo.toml; do
  grep -q "^version = \"$version\"" "$manifest" || { echo "$manifest version differs from package.json ($version)" >&2; exit 1; }
done
uv run --no-sync python scripts/release/third_party_notices.py --check
image=quay.io/pypa/manylinux_2_28_x86_64
rust_version=1.91.0
maturin_version=1.9.4
target=x86_64-unknown-linux-gnu
max_glibc=2.28
source=$(git rev-parse HEAD)
out="release/$version"
wheel_name="radicle_py-$version-cp39-abi3-manylinux_2_28_x86_64.whl"
sdist_name="radicle_py-$version.tar.gz"
tarball_name="radicle-sdk-$version.tgz"
rm -rf "$out" target/release-wheels
mkdir -p "$out" target/docker-cargo-home target/docker-rustup target/docker

echo "== native libraries in $image (Rust $rust_version, glibc $max_glibc floor)"
docker run --rm -v "$PWD:/io" -w /io "$image" bash -c '
  set -euo pipefail
  export CARGO_HOME=/io/target/docker-cargo-home RUSTUP_HOME=/io/target/docker-rustup CARGO_TARGET_DIR=/io/target/docker
  export PATH="$CARGO_HOME/bin:/opt/python/cp312-cp312/bin:$PATH"
  [ -x "$CARGO_HOME/bin/rustup" ] || curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain '"$rust_version"'
  rustup default '"$rust_version"' >/dev/null
  pip install -q "maturin=='"$maturin_version"'"
  { rustc --version; cargo --version; maturin --version; } | tee /io/target/docker/toolchain.txt
  LIBZ_SYS_STATIC=1 maturin build --release --locked --manylinux 2_28 --auditwheel repair --out /io/target/release-wheels
  LIBZ_SYS_STATIC=1 cargo build --release --locked -p radicle-sdk-native
  chown -R '"$(id -u):$(id -g)"' /io/target/docker /io/target/docker-cargo-home /io/target/docker-rustup /io/target/release-wheels
'
[ -f "target/release-wheels/$wheel_name" ] || { echo "expected wheel $wheel_name, found: $(ls target/release-wheels)" >&2; exit 1; }
cp "target/release-wheels/$wheel_name" "$out/"

echo "== JavaScript loader and declarations (host), then the container-built addon"
npm run build >/dev/null
cp target/docker/release/libradicle_sdk_native.so node/dist/radicle-sdk.linux-x64-gnu.node
echo "== sdist"
uv run --no-sync maturin sdist --out "$out" >/dev/null
[ -f "$out/$sdist_name" ] || { echo "expected sdist $sdist_name" >&2; exit 1; }
echo "== npm tarball"
npm pack --ignore-scripts --pack-destination "$out" >/dev/null
[ -f "$out/$tarball_name" ] || { echo "expected tarball $tarball_name" >&2; exit 1; }

echo "== audit native libraries extracted from the packaged files"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/whl" "$tmp/tgz"
(cd "$tmp/whl" && unzip -q "$root/$out/$wheel_name")
(cd "$tmp/tgz" && tar -xzf "$root/$out/$tarball_name")
wheel_so=$(ls "$tmp"/whl/radiclepy/*.so)
addon=$(ls "$tmp"/tgz/package/node/dist/*.node)
[ "$(echo "$wheel_so" | wc -l)" = 1 ] && [ "$(echo "$addon" | wc -l)" = 1 ] || { echo "expected exactly one native library per package" >&2; exit 1; }
bash "$here/audit-elf.sh" "$wheel_so" "$max_glibc"
bash "$here/audit-elf.sh" "$addon" "$max_glibc"
[ -f "$tmp"/whl/*.dist-info/licenses/THIRD-PARTY-NOTICES.md ] || { echo "wheel lacks THIRD-PARTY-NOTICES.md" >&2; exit 1; }
[ -f "$tmp/tgz/package/THIRD-PARTY-NOTICES.md" ] || { echo "npm tarball lacks THIRD-PARTY-NOTICES.md" >&2; exit 1; }
sdist_files=$(tar -tzf "$out/$sdist_name")
[[ "$sdist_files" == *"/THIRD-PARTY-NOTICES.md"* ]] || { echo "sdist lacks THIRD-PARTY-NOTICES.md" >&2; exit 1; }

echo "== packaged consumer smoke (Node) on the exact release tarball"
bash node/scripts/tarball-install-smoke.sh "$out/$tarball_name"

echo "== wheel-only install into a clean environment, no compiler"
uv venv -q "$tmp/venv"
RUSTC=/no-rust CARGO=/no-cargo uv pip install -q --python "$tmp/venv/bin/python" --no-index --only-binary :all: --find-links "$out" "radicle-py==$version"
"$tmp/venv/bin/python" -c 'import radiclepy, sys; sys.exit(0 if radiclepy.__version__ == sys.argv[1] else f"installed {radiclepy.__version__}, expected {sys.argv[1]}")' "$version"
"$tmp/venv/bin/python" python/tests/wheel_smoke.py
uv pip install -q --python "$tmp/venv/bin/python" 'pytest>=8'
"$tmp/venv/bin/python" -m pytest -q python/tests/test_ux.py python/tests/test_value_semantics.py python/tests/test_readme.py

echo "== manifest"
(cd "$out" && sha256sum "$wheel_name" "$sdist_name" "$tarball_name" > SHA256SUMS)
python3 scripts/release/write_manifest.py "$out" "$version" "$source" "$(git show -s --format=%cI HEAD)" "$target" "$image" "$max_glibc" "$(node -p process.versions.node)" target/docker/toolchain.txt
(cd "$out" && sha256sum -c --quiet SHA256SUMS)
echo "release $version built from $source: $(ls "$out" | tr '\n' ' ')"
