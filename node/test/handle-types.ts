// Compile against emitted declarations, never implementation files.
import {
  Profile,
  Home,
  Aliases,
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

// @ts-expect-error obtain from Home.create/load/resolve or profile.home
new Home();
// @ts-expect-error obtain from profile.aliases
new Aliases();
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
  const home: Home = await Home.load(profile.home.path);
  const created: Home = await Home.create('/tmp/radicle-home');
  const resolved: Home = await Home.resolve();
  const storage: Storage = await Storage.open(profile.storage.path, 'alice', profile.nodeId);
  const alias: string | null = await profile.aliases.get(profile.nodeId);
  const grouped: Readonly<Record<string, ReadonlyArray<string>>> =
    await profile.aliases.reverseLookup('alice');
  const did: string = profile.signer().did;
  // @ts-expect-error alias results are frozen
  grouped.alice.push(profile.nodeId);
  // @ts-expect-error setup work returns a Promise
  const immediate: Home = Home.load('/tmp/radicle-home');
  void [home, created, resolved, alias, grouped, did, immediate];
  const signer: Signer = profile.signer();
  const loaded: Profile = await Profile.loadAsync();
  const asynchronous: Signer = await loaded.signerAsync();
  // @ts-expect-error asynchronous setup returns a Promise
  const immediateProfile: Profile = Profile.loadAsync();
  // @ts-expect-error asynchronous signer setup returns a Promise
  const immediateSigner: Signer = loaded.signerAsync();
  void [asynchronous, immediateProfile, immediateSigner];
  const rid = await initProject(storage, signer, '/work', privateOptions);
  const repo: Repository = storage.repository(rid);
  const issues: Issues = Issues.openAs(repo, signer);
  const count: number = await issues.count();
  const empty: boolean = await issues.isEmpty();
  const issueRid: string = issues.rid;
  const issue: IssueMut = await issues.create({ title: 'First', description: '' });
  const value = await issue.reload();
  const commentCount: number = value.commentCount;
  const commentIds: ReadonlyArray<string> = value.commentIds;
  void [count, empty, issueRid, commentCount, commentIds];
  const patches: Patches = Patches.openAs(repo, signer);
  const patch: PatchMut = await patches.draft({
    title: 'First',
    description: '',
    base: '',
    head: '',
  });
  const patchCount: number = await patches.count();
  const patchEmpty: boolean = await patches.isEmpty();
  const patchRid: string = patches.rid;
  const revisionCount: number = (await patch.reload()).revisionCount;
  void [issue, patch, publicOptions, patchCount, patchEmpty, patchRid, revisionCount];
}
void useFactories;
