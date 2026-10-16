#!/usr/bin/env bash
# Validate committed sources, not an uncommitted worktree or a prebuilt addon.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
manager=${1:-npm}
revision=${2:-$(git rev-parse HEAD)}
url=${SDK_GIT_URL:-file://$root}
# git+file:// tests the same root-manifest/#commit contract without publication.
spec="git+${url}#${revision}"
consumer=$(mktemp -d)
trap 'rm -rf "$consumer"' EXIT
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$root/node/native/target}
export npm_config_cache="$consumer/npm-cache"
export BUN_INSTALL_CACHE_DIR="$consumer/bun-cache"
cd "$consumer"
printf '{"name":"sdk-consumer","version":"1.0.0","private":true,"type":"module"}\n' > package.json
case "$manager" in
  npm) npm install --foreground-scripts "$spec" ;;
  pnpm)
    # pnpm 12 checks Git prepare approval before --allow-build takes effect.
    # Approve this exact reviewed Git source/commit, not arbitrary build scripts.
    node -e 'require("node:fs").writeFileSync("pnpm-workspace.yaml", JSON.stringify({allowBuilds:{["radicle-sdk@"+process.argv[1]]:true}}))' "$spec"
    npm exec --yes --package pnpm@12.11.2 -- pnpm add --store-dir "$consumer/pnpm-store" "$spec"
    ;;
  bun) npm exec --yes --package bun@1.4.3 -- bun add --trust "$spec" ;;
  *) echo "expected npm, pnpm or bun" >&2; exit 2 ;;
esac
cp "$root/node/test/install-smoke.mjs" smoke.mjs
node smoke.mjs
if [ "$manager" = bun ]; then
  npm exec --yes --package bun@1.4.3 -- bun smoke.mjs
fi
