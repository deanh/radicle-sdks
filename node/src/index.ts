import * as raw from '../dist/native.cjs';
import { rawOp } from './raw.js';
import type { RawOp } from './raw.js';
export type { RawOp, RawJsonValue } from './raw.js';
export type {
  Comment,
  Commit,
  HeadInfo,
  ProjectDoc,
  Issue,
  IssueChange,
  InitOptions,
  CreateIssueOptions,
  CloseIssueOptions,
  Patch,
  Revision,
  Review,
  PatchChange,
  PatchRevisionChange,
  PatchStateChange,
  CreatePatchOptions,
  EditPatchOptions,
  UpdatePatchOptions,
  PatchCommentOptions,
  ReviewPatchOptions,
  TreeEntry,
  Tree,
  HistoryOptions,
  HistoryPage,
  SignedRefs,
  TagInfo,
  NamespaceTip,
  RemoteInfo,
  SetHead,
  FollowPolicyInfo,
  SeedPolicyInfo,
  RoutingEntry,
  CobRef,
  CobManifest,
} from '../dist/native.cjs';

import { run, sync } from './bridge.js';
export { RadicleError } from './bridge.js';
export type { ErrorCode } from './bridge.js';

/** Validate and canonicalize IDs synchronously; annotations do not validate IDs. */
export function parseRepoId(value: string): string {
  return sync(() => raw.parseRepoId(value));
}
export function parseDid(value: string): string {
  return sync(() => raw.parseDid(value));
}
export function parseNodeId(value: string): string {
  return sync(() => raw.parseNodeId(value));
}
export function parseOid(value: string): string {
  return sync(() => raw.parseOid(value));
}
export function parseObjectId(value: string): string {
  return sync(() => raw.parseObjectId(value));
}

// Module-private factories keep native conversion out of emitted public signatures.
const constructionKey = Symbol('SDK handle construction');
function checkConstruction(key: symbol): void {
  if (key !== constructionKey)
    throw new TypeError('SDK handles must be obtained from their factories');
}
// Factory-only access to Profile's private native state; never public conversion.
let profileRaw: (value: Profile) => raw.NativeProfile;
let wrapHome: (value: raw.NativeHome) => Home;
let wrapAliases: (value: raw.NativeAliases) => Aliases;
let wrapSigner: (value: raw.NativeSigner) => Signer;
let wrapStorage: (value: raw.NativeStorage) => Storage;
let wrapRepository: (value: raw.NativeRepository) => Repository;
let wrapIssue: (value: raw.NativeIssueMut) => IssueMut;
let wrapPatch: (value: raw.NativePatchMut) => PatchMut;

// Handles that other classes or free functions must unwrap (initProject takes a
// Storage and a Signer; the store factories take a Repository) keep their native
// object in these module-private maps, reachable through the accessors below.
// Other handles use private #raw fields; Profile grants database factories
// access only through the module-private closure above.
const storages = new WeakMap<Storage, raw.NativeStorage>();
const signers = new WeakMap<Signer, raw.NativeSigner>();
const repositories = new WeakMap<Repository, raw.NativeRepository>();
function storage(s: Storage): raw.NativeStorage {
  const value = storages.get(s);
  if (!value)
    throw Object.assign(new TypeError('expected Storage'), { code: 'ERR_INVALID_ARG_TYPE' });
  return value;
}
function signer(s: Signer): raw.NativeSigner {
  const value = signers.get(s);
  if (!value)
    throw Object.assign(new TypeError('expected Signer'), { code: 'ERR_INVALID_ARG_TYPE' });
  return value;
}
function repository(r: Repository): raw.NativeRepository {
  const value = repositories.get(r);
  if (!value)
    throw Object.assign(new TypeError('expected Repository'), { code: 'ERR_INVALID_ARG_TYPE' });
  return value;
}

function optionString(options: unknown, name: string): string | undefined {
  if (options === null || typeof options !== 'object' || Array.isArray(options))
    throw new TypeError('options must be an object');
  const value = (options as Record<string, unknown>)[name];
  if (value !== undefined && typeof value !== 'string')
    throw new TypeError(`${name} must be a string or omitted`);
  return value;
}
export class PolicyStore {
  readonly #raw: raw.NativePolicyStore;
  private constructor(key: symbol, value: raw.NativePolicyStore) {
    checkConstruction(key);
    this.#raw = value;
  }
  static async fromProfile(profile: Profile): Promise<PolicyStore> {
    return new PolicyStore(
      constructionKey,
      await run(() => raw.NativePolicyStore.fromProfile(profileRaw(profile))),
    );
  }
  static async reader(profile: Profile): Promise<PolicyStore> {
    return new PolicyStore(
      constructionKey,
      await run(() => raw.NativePolicyStore.reader(profileRaw(profile))),
    );
  }
  static async open(path: string): Promise<PolicyStore> {
    return new PolicyStore(constructionKey, await run(() => raw.NativePolicyStore.open(path)));
  }
  get path(): string {
    return sync(() => this.#raw.path);
  }
  follow(nodeId: string, options: { alias?: string } = {}): Promise<boolean> {
    return run(() => this.#raw.follow(nodeId, optionString(options, 'alias')));
  }
  seed(rid: string, options: { scope?: 'all' | 'followed' } = {}): Promise<boolean> {
    return run(() => this.#raw.seed(rid, optionString(options, 'scope') ?? 'all'));
  }
  setFollowPolicy(nodeId: string, policy: 'allow' | 'block'): Promise<boolean> {
    return run(() => this.#raw.setFollowPolicy(nodeId, policy));
  }
  setSeedPolicy(rid: string, policy: 'allow' | 'block'): Promise<boolean> {
    return run(() => this.#raw.setSeedPolicy(rid, policy));
  }
  unfollow(nodeId: string): Promise<boolean> {
    return run(() => this.#raw.unfollow(nodeId));
  }
  unseed(rid: string): Promise<boolean> {
    return run(() => this.#raw.unseed(rid));
  }
  unblockRid(rid: string): Promise<boolean> {
    return run(() => this.#raw.unblockRid(rid));
  }
  unblockNid(nodeId: string): Promise<boolean> {
    return run(() => this.#raw.unblockNid(nodeId));
  }
  isFollowing(nodeId: string): Promise<boolean> {
    return run(() => this.#raw.isFollowing(nodeId));
  }
  isSeeding(rid: string): Promise<boolean> {
    return run(() => this.#raw.isSeeding(rid));
  }
  isBlocked(nodeId: string): Promise<boolean> {
    return run(() => this.#raw.isBlocked(nodeId));
  }
  followPolicy(nodeId: string): Promise<raw.FollowPolicyInfo | null> {
    return run(() => this.#raw.followPolicy(nodeId));
  }
  seedPolicy(rid: string): Promise<raw.SeedPolicyInfo | null> {
    return run(() => this.#raw.seedPolicy(rid));
  }
  followPolicies(): Promise<ReadonlyArray<raw.FollowPolicyInfo>> {
    return run(() => this.#raw.followPolicies());
  }
  seedPolicies(): Promise<ReadonlyArray<raw.SeedPolicyInfo>> {
    return run(() => this.#raw.seedPolicies());
  }
  nodesByAlias(alias: string): Promise<ReadonlyArray<readonly [string, string]>> {
    return run(() => this.#raw.nodesByAlias(alias));
  }
}
export class RoutingStore {
  readonly #raw: raw.NativeRoutingStore;
  private constructor(key: symbol, value: raw.NativeRoutingStore) {
    checkConstruction(key);
    this.#raw = value;
  }
  static async fromProfile(profile: Profile): Promise<RoutingStore> {
    return new RoutingStore(
      constructionKey,
      await run(() => raw.NativeRoutingStore.fromProfile(profileRaw(profile))),
    );
  }
  get(rid: string): Promise<ReadonlyArray<string>> {
    return run(() => this.#raw.get(rid));
  }
  getInventory(nodeId: string): Promise<ReadonlyArray<string>> {
    return run(() => this.#raw.getInventory(nodeId));
  }
  entry(rid: string, nodeId: string): Promise<number | null> {
    return run(() => this.#raw.entry(rid, nodeId));
  }
  entries(): Promise<ReadonlyArray<raw.RoutingEntry>> {
    return run(() => this.#raw.entries());
  }
  count(rid: string): Promise<number> {
    return run(() => this.#raw.count(rid));
  }
  total(): Promise<number> {
    return run(() => this.#raw.total());
  }
  isEmpty(): Promise<boolean> {
    return run(() => this.#raw.isEmpty());
  }
}

export class Raw {
  readonly #raw: raw.NativeRaw;
  private constructor(key: symbol, value: raw.NativeRaw) {
    checkConstruction(key);
    this.#raw = value;
  }
  static open(repo: Repository): Raw {
    return new Raw(
      constructionKey,
      sync(() => raw.NativeRaw.open(repository(repo))),
    );
  }
  get rid(): string {
    return sync(() => this.#raw.rid);
  }
  typenames(): Promise<ReadonlyArray<string>> {
    return run(() => this.#raw.typenames());
  }
  entities(typename: string): Promise<ReadonlyArray<string>> {
    return run(() => this.#raw.entities(typename));
  }
  refs(typename: string, entityId: string): Promise<ReadonlyArray<raw.CobRef>> {
    return run(() => this.#raw.refs(typename, entityId));
  }
  async history(typename: string, entityId: string): Promise<ReadonlyArray<RawOp>> {
    return Object.freeze((await run(() => this.#raw.history(typename, entityId))).map(rawOp));
  }
}

export class Home {
  readonly #raw: raw.NativeHome;
  private constructor(key: symbol, value: raw.NativeHome) {
    checkConstruction(key);
    this.#raw = value;
  }
  static {
    wrapHome = (value) => new Home(constructionKey, value);
  }
  static async create(path: string): Promise<Home> {
    return wrapHome(await run(() => raw.NativeHome.create(path)));
  }
  static async load(path: string): Promise<Home> {
    return wrapHome(await run(() => raw.NativeHome.load(path)));
  }
  static async resolve(): Promise<Home> {
    return wrapHome(await run(() => raw.NativeHome.resolve()));
  }
  get path(): string {
    return sync(() => this.#raw.path);
  }
}
export class Aliases {
  readonly #raw: raw.NativeAliases;
  private constructor(key: symbol, value: raw.NativeAliases) {
    checkConstruction(key);
    this.#raw = value;
  }
  static {
    wrapAliases = (value) => new Aliases(constructionKey, value);
  }
  get(nodeId: string): Promise<string | null> {
    return run(() => this.#raw.get(nodeId));
  }
  async reverseLookup(alias: string): Promise<Readonly<Record<string, ReadonlyArray<string>>>> {
    const groups = await run(() => this.#raw.reverseLookup(alias));
    // Define own data properties, even for aliases such as __proto__.
    return Object.freeze(Object.fromEntries(groups.map((g) => [g.alias, g.nodeIds])));
  }
}

export class Profile {
  readonly #raw: raw.NativeProfile;
  static {
    profileRaw = (value) => value.#raw;
  }
  private constructor(key: symbol, value: raw.NativeProfile) {
    checkConstruction(key);
    this.#raw = value;
  }
  static async init(
    home: string,
    alias: string,
    options: { passphrase?: string } = {},
  ): Promise<Profile> {
    return new Profile(
      constructionKey,
      await run(() => raw.NativeProfile.init(home, alias, options.passphrase)),
    );
  }
  /** Synchronous setup: uses the native process environment, like radicle-py. */
  static load(): Profile {
    return new Profile(
      constructionKey,
      sync(() => raw.NativeProfile.load()),
    );
  }
  get did(): string {
    return this.#raw.did;
  }
  get nodeId(): string {
    return this.#raw.nodeId;
  }
  get home(): Home {
    return wrapHome(sync(() => this.#raw.home));
  }
  get aliases(): Aliases {
    return wrapAliases(sync(() => this.#raw.aliases));
  }
  get storage(): Storage {
    return wrapStorage(this.#raw.storage);
  }
  policyStore(): Promise<PolicyStore> {
    return PolicyStore.fromProfile(this);
  }
  routingStore(): Promise<RoutingStore> {
    return RoutingStore.fromProfile(this);
  }
  /** Synchronous setup: key decryption or agent communication can block. */
  signer(): Signer {
    return wrapSigner(sync(() => this.#raw.signer()));
  }
}
export class Signer {
  private constructor(key: symbol, value: raw.NativeSigner) {
    checkConstruction(key);
    signers.set(this, value);
  }
  static {
    wrapSigner = (value) => new Signer(constructionKey, value);
  }
  get did(): string {
    return sync(() => signer(this).did);
  }
  get nodeId(): string {
    return signer(this).nodeId;
  }
}
export class Storage {
  static async open(path: string, alias: string, nodeId: string): Promise<Storage> {
    return wrapStorage(await run(() => raw.NativeStorage.open(path, alias, nodeId)));
  }
  get path(): string {
    return sync(() => storage(this).path);
  }
  private constructor(key: symbol, value: raw.NativeStorage) {
    checkConstruction(key);
    storages.set(this, value);
  }
  static {
    wrapStorage = (value) => new Storage(constructionKey, value);
  }
  repository(rid: string): Repository {
    return wrapRepository(sync(() => storage(this).repository(rid)));
  }
  contains(rid: string): Promise<boolean> {
    return run(() => storage(this).contains(rid));
  }
  repositories(): Promise<ReadonlyArray<string>> {
    return run(() => storage(this).repositories());
  }
}
export class Repository {
  private constructor(key: symbol, value: raw.NativeRepository) {
    checkConstruction(key);
    repositories.set(this, value);
  }
  static {
    wrapRepository = (value) => new Repository(constructionKey, value);
  }
  get rid(): string {
    return repository(this).rid;
  }
  head(): Promise<raw.HeadInfo> {
    return run(() => repository(this).head());
  }
  identity(): Promise<raw.ProjectDoc> {
    return run(() => repository(this).identity());
  }
  async description(): Promise<string> {
    return (await this.identity()).description;
  }
  tags(): Promise<ReadonlyArray<raw.TagInfo>> {
    return run(() => repository(this).tags());
  }
  /** Reflexive plain-Git commit ancestry, not COB causal ancestry. */
  isAncestor(ancestor: string, descendant: string): Promise<boolean> {
    return run(() => repository(this).isAncestor(ancestor, descendant));
  }
  revParse(rev: string): Promise<string> {
    return run(() => repository(this).revParse(rev));
  }
  /** Sorted sigrefs targets; reads refs only, not signed sets or objects. */
  namespaceTips(): Promise<ReadonlyArray<raw.NamespaceTip>> {
    return run(() => repository(this).namespaceTips());
  }
  /** Upstream-verified signed set at an explicit tip; no operation verification. */
  signedRefsAt(nodeId: string, oid: string): Promise<raw.SignedRefs> {
    return run(() => repository(this).signedRefsAt(nodeId, oid));
  }
  remotes(): Promise<ReadonlyArray<raw.RemoteInfo>> {
    return run(() => repository(this).remotes());
  }
  setDefaultBranchToCanonicalHead(): Promise<raw.SetHead> {
    return run(() => repository(this).setDefaultBranchToCanonicalHead());
  }
  setIdentityHead(): Promise<string> {
    return run(() => repository(this).setIdentityHead());
  }
  setRemoteIdentityRoot(remote: string): Promise<string> {
    return run(() => repository(this).setRemoteIdentityRoot(remote));
  }
  commit(oid: string): Promise<raw.Commit> {
    return run(() => repository(this).commit(oid));
  }
  branches(): Promise<ReadonlyArray<string>> {
    return run(() => repository(this).branches());
  }
  branchHead(name: string): Promise<string | null> {
    return run(() => repository(this).branchHead(name));
  }
  tree(oid: string): Promise<raw.Tree> {
    return run(() => repository(this).tree(oid));
  }
  blob(oid: string): Promise<Uint8Array> {
    return run(() => repository(this).blob(oid));
  }
  history(rev: string, options: raw.HistoryOptions = {}): Promise<raw.HistoryPage> {
    return run(() => repository(this).history(rev, options));
  }
  signRefs(sg: Signer): Promise<raw.SignedRefs> {
    return run(() => repository(this).signRefs(signer(sg)));
  }
}
export function initProject(
  s: Storage,
  sg: Signer,
  path: string,
  options: raw.InitOptions,
): Promise<string> {
  return run(() => {
    // napi treats optional null as absent; do not silently turn invalid visibility public.
    if (options?.visibility === null)
      throw new TypeError('visibility must be public, private, or omitted');
    return raw.initProject(storage(s), signer(sg), path, options);
  });
}
export function forkProject(s: Storage, sg: Signer, rid: string): Promise<void> {
  return run(() => raw.forkProject(storage(s), signer(sg), rid));
}
export function checkoutProject(
  s: Storage,
  rid: string,
  remote: string,
  path: string,
  options: { bare?: boolean } = {},
): Promise<string> {
  return run(() => {
    if (options === null || typeof options !== 'object' || Array.isArray(options))
      throw new TypeError('checkout options must be an object');
    if (options.bare !== undefined && typeof options.bare !== 'boolean')
      throw new TypeError('bare must be a boolean');
    return raw.checkoutProject(storage(s), rid, remote, path, options.bare ?? false);
  });
}
export class Patches {
  readonly #raw: raw.NativePatches;
  private constructor(key: symbol, value: raw.NativePatches) {
    checkConstruction(key);
    this.#raw = value;
  }
  get rid(): string {
    return sync(() => this.#raw.rid);
  }
  count(): Promise<number> {
    return run(() => this.#raw.count());
  }
  isEmpty(): Promise<boolean> {
    return run(() => this.#raw.isEmpty());
  }
  static open(repo: Repository): Patches {
    return new Patches(
      constructionKey,
      sync(() => raw.NativePatches.open(repository(repo))),
    );
  }
  static openAs(repo: Repository, sg: Signer): Patches {
    return new Patches(
      constructionKey,
      sync(() => raw.NativePatches.openAs(repository(repo), signer(sg))),
    );
  }
  async create(options: raw.CreatePatchOptions): Promise<PatchMut> {
    return wrapPatch(await run(() => this.#raw.create(options)));
  }
  async draft(options: raw.CreatePatchOptions): Promise<PatchMut> {
    return wrapPatch(await run(() => this.#raw.draft(options)));
  }
  async getMut(id: string): Promise<PatchMut> {
    return wrapPatch(await run(() => this.#raw.getMut(id)));
  }
  get(id: string): Promise<raw.Patch | null> {
    return run(() => this.#raw.get(id));
  }
  all(): Promise<ReadonlyArray<raw.Patch>> {
    return run(() => this.#raw.all());
  }
}
export class PatchMut {
  readonly #raw: raw.NativePatchMut;
  private constructor(key: symbol, value: raw.NativePatchMut) {
    checkConstruction(key);
    this.#raw = value;
  }
  static {
    wrapPatch = (value) => new PatchMut(constructionKey, value);
  }
  get id(): string {
    return this.#raw.id;
  }
  reload(): Promise<raw.Patch> {
    return run(() => this.#raw.reload());
  }
  edit(options: raw.EditPatchOptions): Promise<raw.PatchChange> {
    return run(() => this.#raw.edit(options));
  }
  label(labels: ReadonlyArray<string>): Promise<raw.PatchChange> {
    return run(() => this.#raw.label(labels));
  }
  assign(assignees: ReadonlyArray<string>): Promise<raw.PatchChange> {
    return run(() => this.#raw.assign(assignees));
  }
  update(options: raw.UpdatePatchOptions): Promise<raw.PatchRevisionChange> {
    return run(() => this.#raw.update(options));
  }
  ready(): Promise<raw.PatchStateChange> {
    return run(() => this.#raw.ready());
  }
  unready(): Promise<raw.PatchStateChange> {
    return run(() => this.#raw.unready());
  }
  archive(): Promise<raw.PatchStateChange> {
    return run(() => this.#raw.archive());
  }
  unarchive(): Promise<raw.PatchStateChange> {
    return run(() => this.#raw.unarchive());
  }
  comment(
    revision: string,
    body: string,
    options: raw.PatchCommentOptions = {},
  ): Promise<raw.PatchChange> {
    return run(() => this.#raw.comment(revision, body, options));
  }
  review(revision: string, options: raw.ReviewPatchOptions = {}): Promise<raw.PatchChange> {
    return run(() => this.#raw.review(revision, options));
  }
  /** Record an upstream merge decision; never moves a Git branch. Inspect snapshot.state. */
  merge(revision: string, commit: string): Promise<raw.PatchChange> {
    return run(() => this.#raw.merge(revision, commit));
  }
}
export class Issues {
  readonly #raw: raw.NativeIssues;
  private constructor(key: symbol, value: raw.NativeIssues) {
    checkConstruction(key);
    this.#raw = value;
  }
  get rid(): string {
    return sync(() => this.#raw.rid);
  }
  count(): Promise<number> {
    return run(() => this.#raw.count());
  }
  isEmpty(): Promise<boolean> {
    return run(() => this.#raw.isEmpty());
  }
  static open(repo: Repository): Issues {
    return new Issues(
      constructionKey,
      sync(() => raw.NativeIssues.open(repository(repo))),
    );
  }
  static openAs(repo: Repository, sg: Signer): Issues {
    return new Issues(
      constructionKey,
      sync(() => raw.NativeIssues.openAs(repository(repo), signer(sg))),
    );
  }
  async create(options: raw.CreateIssueOptions): Promise<IssueMut> {
    return wrapIssue(await run(() => this.#raw.create(options)));
  }
  async getMut(id: string): Promise<IssueMut> {
    return wrapIssue(await run(() => this.#raw.getMut(id)));
  }
  get(id: string): Promise<raw.Issue | null> {
    return run(() => this.#raw.get(id));
  }
  /** Eager, ID-sorted detached snapshots; no mutable snapshot cache. */
  all(): Promise<ReadonlyArray<raw.Issue>> {
    return run(() => this.#raw.all());
  }
}
export class IssueMut {
  readonly #raw: raw.NativeIssueMut;
  private constructor(key: symbol, value: raw.NativeIssueMut) {
    checkConstruction(key);
    this.#raw = value;
  }
  static {
    wrapIssue = (value) => new IssueMut(constructionKey, value);
  }
  get id(): string {
    return this.#raw.id;
  }
  reload(): Promise<raw.Issue> {
    return run(() => this.#raw.reload());
  }
  comment(body: string, options: { replyTo?: string } = {}): Promise<raw.IssueChange> {
    return run(() => this.#raw.comment(body, options.replyTo));
  }
  edit(title: string): Promise<raw.IssueChange> {
    return run(() => this.#raw.edit(title));
  }
  /** Edit the root comment; does not add a discussion comment. */
  editDescription(description: string): Promise<raw.IssueChange> {
    return run(() => this.#raw.editDescription(description));
  }
  /** Replace the entire label set; [] clears it. */
  label(labels: ReadonlyArray<string>): Promise<raw.IssueChange> {
    return run(() => this.#raw.label(labels));
  }
  /** Replace the entire assignee DID set; [] clears it. */
  assign(assignees: ReadonlyArray<string>): Promise<raw.IssueChange> {
    return run(() => this.#raw.assign(assignees));
  }
  close(options: raw.CloseIssueOptions = {}): Promise<raw.IssueChange> {
    return run(() => this.#raw.close(options));
  }
  reopen(): Promise<raw.IssueChange> {
    return run(() => this.#raw.reopen());
  }
}
