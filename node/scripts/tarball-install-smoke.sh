#!/usr/bin/env bash
# Validate packed consumers separately from Git prepare in a complete checkout.
# Run `npm run build` first; tarball installation must not compile Rust.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
consumer=$(mktemp -d)
trap 'rm -rf "$consumer"' EXIT
cd "$root"
npm pack --ignore-scripts --json --pack-destination "$consumer" > "$consumer/pack.json"
node --input-type=module - "$consumer/pack.json" <<'JS'
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const [{ files }] = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const paths = files.map(file => file.path);
for (const path of ['node/README.md', 'node/API.md', 'node/CHANGELOG.md', 'LICENSE-MIT', 'LICENSE-APACHE']) {
  assert.ok(paths.includes(path), `missing ${path}`);
}
assert.ok(paths.some(path => path.endsWith('.node')), 'built addon missing');
assert.ok(paths.every(path => !path.endsWith('.rs') && !path.includes('Cargo.') &&
  !path.startsWith('node/native/') && !path.startsWith('python/')), 'Rust/workspace source leaked');
JS
cd "$consumer"
printf '{"name":"sdk-tarball-consumer","version":"1.0.0","private":true,"type":"module"}\n' > package.json
export npm_config_cache="$consumer/npm-cache"
RUSTC=/no-rust-compiler CARGO=/no-cargo npm install --foreground-scripts "$consumer"/*.tgz
cp "$root/node/test/install-smoke.mjs" smoke.mjs
cp "$root/node/test/readme-consumer.mjs" readme.mjs
cp "$root/node/test/handle-types.ts" handle-types.ts
node smoke.mjs
node readme.mjs
npm exec --yes --package typescript@5.9.3 -- tsc --strict --noEmit --target ES2023 --module NodeNext --moduleResolution NodeNext handle-types.ts
printf 'Packed consumer runtime, docs and types OK (no Rust source build)\n'
