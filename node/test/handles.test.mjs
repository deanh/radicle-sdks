import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  Home,
  Aliases,
  Profile,
  Signer,
  Storage,
  Repository,
  Issues,
  IssueMut,
  Patches,
  PatchMut,
} from 'radicle-sdk';
import { fixture } from './helpers.mjs';

test('handles reject direct JavaScript construction and expose no native conversion factories', async (t) => {
  for (const Handle of [
    Home,
    Aliases,
    Profile,
    Signer,
    Storage,
    Repository,
    Issues,
    IssueMut,
    Patches,
    PatchMut,
  ]) {
    for (const args of [[], [{}], [Symbol('not the construction key'), {}]]) {
      assert.throws(
        () => Reflect.construct(Handle, args),
        /handles must be obtained from their factories/,
      );
    }
    assert.equal(Handle.fromNative, undefined);
  }
  const f = await fixture(t);
  assert.ok(f.profile instanceof Profile);
  assert.ok(f.profile.home instanceof Home);
  assert.ok(f.profile.aliases instanceof Aliases);
  assert.ok(f.signer instanceof Signer);
  assert.ok(f.storage instanceof Storage);
  assert.ok(f.repo instanceof Repository);
  const issues = Issues.openAs(f.repo, f.signer);
  const issue = await issues.create({ title: 'Factory', description: '' });
  assert.ok(issues instanceof Issues);
  assert.ok(issue instanceof IssueMut);
  assert.ok((await issues.getMut(issue.id)) instanceof IssueMut);
  const patches = Patches.openAs(f.repo, f.signer);
  const head = (await f.repo.head()).oid;
  const patch = await patches.draft({ title: 'Factory', description: '', base: head, head });
  assert.ok(patches instanceof Patches);
  assert.ok(patch instanceof PatchMut);
  assert.ok((await patches.getMut(patch.id)) instanceof PatchMut);
  const declarations = await readFile(new URL('../dist/index.d.ts', import.meta.url), 'utf8');
  assert.equal((declarations.match(/private constructor\(\);/g) || []).length, 10);
  assert.doesNotMatch(
    declarations,
    /raw\.Native|wrapHome|wrapAliases|wrapSigner|wrapStorage|wrapRepository|wrapIssue|wrapPatch|constructionKey/,
  );
});
