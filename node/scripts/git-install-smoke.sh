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
  npm)
    flags=(--foreground-scripts)
    version=$(npm --version)
    if (( ${version%%.*} >= 12 )); then flags+=(--allow-git=root); fi
    npm install "${flags[@]}" "$spec"
    ;;
  npm11)
    npm exec --yes --package npm@11.20.0 -- npm install --foreground-scripts "$spec"
    ;;
  npm12)
    npm exec --yes --package npm@12.2.0 -- npm install --foreground-scripts --allow-git=root "$spec"
    ;;
  pnpm)
    # pnpm 12 checks Git prepare approval before --allow-build takes effect.
    # Approve this exact reviewed Git source/commit, not arbitrary build scripts.
    node -e 'require("node:fs").writeFileSync("pnpm-workspace.yaml", JSON.stringify({allowBuilds:{["radicle-sdk@"+process.argv[1]]:true}}))' "$spec"
    npm exec --yes --package pnpm@12.11.2 -- pnpm add --store-dir "$consumer/pnpm-store" "$spec"
    ;;
  bun) npm exec --yes --package bun@1.4.3 -- bun add --trust "$spec" ;;
  *) echo "expected npm, npm11, npm12, pnpm or bun" >&2; exit 2 ;;
esac
cp "$root/node/test/install-smoke.mjs" smoke.mjs
node smoke.mjs
cp "$root/node/test/readme-consumer.mjs" readme.mjs
node readme.mjs
cp "$root/node/test/handle-types.ts" handle-types.ts
npm exec --yes --package typescript@5.9.3 -- tsc --strict --noEmit --target ES2023 --module NodeNext --moduleResolution NodeNext handle-types.ts
printf 'Installed consumer handle types OK\n'
if [ "$manager" = bun ]; then
  npm exec --yes --package bun@1.4.3 -- bun smoke.mjs
fi
