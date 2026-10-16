import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

const text = path => readFile(new URL(path, import.meta.url), 'utf8');

test('alpha package/native identities agree and source prepare selects release builds', async () => {
  const pkg = JSON.parse(await text('../../package.json'));
  const lock = JSON.parse(await text('../../package-lock.json'));
  const cargo = await text('../native/Cargo.toml');
  const cargoLock = await text('../../Cargo.lock');
  const workspace = await text('../../Cargo.toml');
  assert.match(workspace, /resolver = "3"/);
  assert.match(workspace, /radicle = "=0\.25\.1"/);
  assert.match(cargo, /radicle\.workspace = true/);
  assert.match(cargo, /indexmap\.workspace = true/);
  const bindings = await text('../dist/native.cjs');
  assert.equal(pkg.name, 'radicle-sdk');
  assert.equal(pkg.version, '0.1.0');
  assert.equal(pkg.private, true); // no accidental registry release
  assert.equal(lock.version, pkg.version);
  assert.equal(lock.packages[''].version, pkg.version);
  assert.equal(pkg.napi.binaryName, 'radicle-sdk');
  assert.match(cargo, /^name = "radicle-sdk-native"$/m);
  assert.equal(cargo.match(/^version = "([^"]+)"/m)[1], pkg.version);
  assert.ok(cargoLock.includes(`name = "radicle-sdk-native"\nversion = "${pkg.version}"`));
  assert.match(pkg.scripts['build:native'], /\bnapi build --release\b/);
  assert.equal(pkg.scripts.prepare, 'npm run build');
  assert.equal(pkg.scripts.install, undefined);
  assert.equal(pkg.scripts.postinstall, undefined);
  assert.ok(pkg.files.every(file => !file.startsWith('node/native') && !file.includes('Cargo')));
  assert.ok(pkg.files.includes('LICENSE-MIT') && pkg.files.includes('LICENSE-APACHE'));
  assert.match(pkg.scripts.build, /^npm run clean && /);
  const addons = (await readdir(new URL('../dist/', import.meta.url))).filter(name => name.endsWith('.node'));
  assert.deepEqual(addons, ['radicle-sdk.linux-x64-gnu.node']); // no leftover T0/debug addon
  assert.match(bindings, /radicle-sdk\.linux-x64-gnu\.node/);
  assert.ok(!bindings.includes('radicle-sdk-t0'));
  for (const file of ['node/README.md', 'node/API.md', 'node/CHANGELOG.md']) {
    assert.ok(pkg.files.includes(file), `${file} must ship with the package`);
  }
  assert.ok((await text('../CHANGELOG.md')).includes(`## ${pkg.version}`));
});
