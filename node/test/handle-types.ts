// Compile against emitted declarations, never implementation files.
import {
  Profile,
  Signer,
  Storage,
  Repository,
  Issues,
  IssueMut,
  Patches,
  PatchMut,
  initProject,
  type InitOptions,
} from 'radicle-sdk';

// @ts-expect-error handles have private constructors, not implicit public zero-arg ones
new Profile();
// @ts-expect-error obtain from profile.signer()
new Signer();
// @ts-expect-error obtain from profile.storage
new Storage();
// @ts-expect-error obtain from storage.repository()
new Repository();
// @ts-expect-error obtain from Issues.open/openAs()
new Issues();
// @ts-expect-error obtain from Issues.create/getMut()
new IssueMut();
// @ts-expect-error obtain from Patches.open/openAs()
new Patches();
// @ts-expect-error obtain from Patches.create/draft/getMut()
new PatchMut();

// Even an any/native-looking argument must not bypass constructor privacy.
declare const native: any;
// @ts-expect-error private constructor
new Signer(native);
// @ts-expect-error private constructor
new Storage(native);
// @ts-expect-error private constructor
new Repository(native);
// @ts-expect-error private constructor
new IssueMut(native);
// @ts-expect-error private constructor
new PatchMut(native);
// @ts-expect-error module-private factories are not public class methods
Signer.fromNative(native);
// @ts-expect-error cannot subclass a handle with a private constructor
class FakeStorage extends Storage {}
void FakeStorage;

const defaults: InitOptions = { name: 'demo', description: '', defaultBranch: 'main' };
const privateOptions: InitOptions = { ...defaults, visibility: 'private' };
const publicOptions: InitOptions = { ...defaults, visibility: 'public' };
// @ts-expect-error finite visibility domain
const invalid: InitOptions = { ...defaults, visibility: 'internal' };
void invalid;
// @ts-expect-error explicit null must not silently become public
const nullVisibility: InitOptions = { ...defaults, visibility: null };
void nullVisibility;
async function useFactories(profile: Profile): Promise<void> {
  const storage: Storage = profile.storage;
  const signer: Signer = profile.signer();
  const rid = await initProject(storage, signer, '/work', privateOptions);
  const repo: Repository = storage.repository(rid);
  const issues: Issues = Issues.openAs(repo, signer);
  const issue: IssueMut = await issues.create({ title: 'First', description: '' });
  const patches: Patches = Patches.openAs(repo, signer);
  const patch: PatchMut = await patches.draft({
    title: 'First',
    description: '',
    base: '',
    head: '',
  });
  void [issue, patch, publicOptions];
}
void useFactories;
