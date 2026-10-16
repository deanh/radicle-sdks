import * as raw from '../dist/native.cjs';
export type { Comment, Commit, HeadInfo, ProjectDoc, Issue, IssueChange, InitOptions, CreateIssueOptions, CloseIssueOptions,
  Patch, Revision, Review, PatchChange, PatchRevisionChange, PatchStateChange,
  CreatePatchOptions, EditPatchOptions, UpdatePatchOptions, PatchCommentOptions, ReviewPatchOptions,
  TreeEntry, Tree, HistoryOptions, HistoryPage, SignedRefs } from '../dist/native.cjs';

import { run, sync } from './bridge.js';
export { RadicleError } from './bridge.js';
export type { ErrorCode } from './bridge.js';

const storages = new WeakMap<Storage, raw.NativeStorage>();
const signers = new WeakMap<Signer, raw.NativeSigner>();
const repositories = new WeakMap<Repository, raw.NativeRepository>();
function storage(s: Storage): raw.NativeStorage {
  const value = storages.get(s);
  if (!value) throw Object.assign(new TypeError('expected Storage'), { code: 'ERR_INVALID_ARG_TYPE' });
  return value;
}
function signer(s: Signer): raw.NativeSigner {
  const value = signers.get(s);
  if (!value) throw Object.assign(new TypeError('expected Signer'), { code: 'ERR_INVALID_ARG_TYPE' });
  return value;
}
function repository(r: Repository): raw.NativeRepository {
  const value = repositories.get(r);
  if (!value) throw Object.assign(new TypeError('expected Repository'), { code: 'ERR_INVALID_ARG_TYPE' });
  return value;
}

export class Profile {
  readonly #raw: raw.NativeProfile;
  private constructor(value: raw.NativeProfile) { this.#raw = value; }
  static async init(home: string, alias: string, options: { passphrase?: string } = {}): Promise<Profile> {
    return new Profile(await run(() => raw.NativeProfile.init(home, alias, options.passphrase)));
  }
  /** Synchronous setup: uses the native process environment, like radicle-py. */
  static load(): Profile { return new Profile(sync(() => raw.NativeProfile.load())); }
  get did(): string { return this.#raw.did; }
  get nodeId(): string { return this.#raw.nodeId; }
  get storage(): Storage { return new Storage(this.#raw.storage); }
  /** Synchronous setup: key decryption or agent communication can block. */
  signer(): Signer { return new Signer(sync(() => this.#raw.signer())); }
}
export class Signer {
  /** @internal Native conversion only; obtain a signer from Profile.signer(). */
  constructor(value: raw.NativeSigner) { signers.set(this, value); }
  get nodeId(): string { return signer(this).nodeId; }
}
export class Storage {
  /** @internal Native conversion only; obtain storage from Profile.storage. */
  constructor(value: raw.NativeStorage) { storages.set(this, value); }
  repository(rid: string): Repository { return new Repository(sync(() => storage(this).repository(rid))); }
  contains(rid: string): Promise<boolean> { return run(() => storage(this).contains(rid)); }
  repositories(): Promise<ReadonlyArray<string>> { return run(() => storage(this).repositories()); }
}
export class Repository {
  /** @internal Native conversion only; obtain repositories from Storage. */
  constructor(value: raw.NativeRepository) { repositories.set(this, value); }
  get rid(): string { return repository(this).rid; }
  head(): Promise<raw.HeadInfo> { return run(() => repository(this).head()); }
  identity(): Promise<raw.ProjectDoc> { return run(() => repository(this).identity()); }
  commit(oid: string): Promise<raw.Commit> { return run(() => repository(this).commit(oid)); }
  branches(): Promise<ReadonlyArray<string>> { return run(() => repository(this).branches()); }
  branchHead(name: string): Promise<string | null> { return run(() => repository(this).branchHead(name)); }
  tree(oid: string): Promise<raw.Tree> { return run(() => repository(this).tree(oid)); }
  blob(oid: string): Promise<Uint8Array> { return run(() => repository(this).blob(oid)); }
  history(rev: string, options: raw.HistoryOptions = {}): Promise<raw.HistoryPage> {
    return run(() => repository(this).history(rev, options));
  }
  signRefs(sg: Signer): Promise<raw.SignedRefs> { return run(() => repository(this).signRefs(signer(sg))); }
}
export function initProject(s: Storage, sg: Signer, path: string, options: raw.InitOptions): Promise<string> {
  return run(() => raw.initProject(storage(s), signer(sg), path, options));
}
export function forkProject(s: Storage, sg: Signer, rid: string): Promise<void> {
  return run(() => raw.forkProject(storage(s), signer(sg), rid));
}
export function checkoutProject(s: Storage, rid: string, remote: string, path: string): Promise<string> {
  return run(() => raw.checkoutProject(storage(s), rid, remote, path));
}
export class Patches {
  readonly #raw: raw.NativePatches;
  private constructor(value: raw.NativePatches) { this.#raw = value; }
  static open(repo: Repository): Patches { return new Patches(sync(() => raw.NativePatches.open(repository(repo)))); }
  static openAs(repo: Repository, sg: Signer): Patches {
    return new Patches(sync(() => raw.NativePatches.openAs(repository(repo), signer(sg))));
  }
  async create(options: raw.CreatePatchOptions): Promise<PatchMut> { return new PatchMut(await run(() => this.#raw.create(options))); }
  async draft(options: raw.CreatePatchOptions): Promise<PatchMut> { return new PatchMut(await run(() => this.#raw.draft(options))); }
  async getMut(id: string): Promise<PatchMut> { return new PatchMut(await run(() => this.#raw.getMut(id))); }
  get(id: string): Promise<raw.Patch | null> { return run(() => this.#raw.get(id)); }
  all(): Promise<ReadonlyArray<raw.Patch>> { return run(() => this.#raw.all()); }
}
export class PatchMut {
  readonly #raw: raw.NativePatchMut;
  /** @internal Obtain mutable handles from Patches. No cached snapshot. */
  constructor(value: raw.NativePatchMut) { this.#raw = value; }
  get id(): string { return this.#raw.id; }
  reload(): Promise<raw.Patch> { return run(() => this.#raw.reload()); }
  edit(options: raw.EditPatchOptions): Promise<raw.PatchChange> { return run(() => this.#raw.edit(options)); }
  label(labels: ReadonlyArray<string>): Promise<raw.PatchChange> { return run(() => this.#raw.label(labels)); }
  assign(assignees: ReadonlyArray<string>): Promise<raw.PatchChange> { return run(() => this.#raw.assign(assignees)); }
  update(options: raw.UpdatePatchOptions): Promise<raw.PatchRevisionChange> { return run(() => this.#raw.update(options)); }
  ready(): Promise<raw.PatchStateChange> { return run(() => this.#raw.ready()); }
  unready(): Promise<raw.PatchStateChange> { return run(() => this.#raw.unready()); }
  archive(): Promise<raw.PatchStateChange> { return run(() => this.#raw.archive()); }
  unarchive(): Promise<raw.PatchStateChange> { return run(() => this.#raw.unarchive()); }
  comment(revision: string, body: string, options: raw.PatchCommentOptions = {}): Promise<raw.PatchChange> {
    return run(() => this.#raw.comment(revision, body, options));
  }
  review(revision: string, options: raw.ReviewPatchOptions = {}): Promise<raw.PatchChange> {
    return run(() => this.#raw.review(revision, options));
  }
  /** Record an upstream merge decision; never moves a Git branch. Inspect snapshot.state. */
  merge(revision: string, commit: string): Promise<raw.PatchChange> { return run(() => this.#raw.merge(revision, commit)); }
}
export class Issues {
  readonly #raw: raw.NativeIssues;
  private constructor(value: raw.NativeIssues) { this.#raw = value; }
  static open(repo: Repository): Issues { return new Issues(sync(() => raw.NativeIssues.open(repository(repo)))); }
  static openAs(repo: Repository, sg: Signer): Issues {
    return new Issues(sync(() => raw.NativeIssues.openAs(repository(repo), signer(sg))));
  }
  async create(options: raw.CreateIssueOptions): Promise<IssueMut> {
    return new IssueMut(await run(() => this.#raw.create(options)));
  }
  async getMut(id: string): Promise<IssueMut> { return new IssueMut(await run(() => this.#raw.getMut(id))); }
  get(id: string): Promise<raw.Issue | null> { return run(() => this.#raw.get(id)); }
  /** Eager, ID-sorted detached snapshots; no mutable snapshot cache. */
  all(): Promise<ReadonlyArray<raw.Issue>> { return run(() => this.#raw.all()); }
}
export class IssueMut {
  readonly #raw: raw.NativeIssueMut;
  /** @internal Native conversion only; obtain mutable handles from Issues. */
  constructor(value: raw.NativeIssueMut) { this.#raw = value; }
  get id(): string { return this.#raw.id; }
  reload(): Promise<raw.Issue> { return run(() => this.#raw.reload()); }
  comment(body: string, options: { replyTo?: string } = {}): Promise<raw.IssueChange> {
    return run(() => this.#raw.comment(body, options.replyTo));
  }
  edit(title: string): Promise<raw.IssueChange> { return run(() => this.#raw.edit(title)); }
  /** Edit the root comment; does not add a discussion comment. */
  editDescription(description: string): Promise<raw.IssueChange> {
    return run(() => this.#raw.editDescription(description));
  }
  /** Replace the entire label set; [] clears it. */
  label(labels: ReadonlyArray<string>): Promise<raw.IssueChange> { return run(() => this.#raw.label(labels)); }
  /** Replace the entire assignee DID set; [] clears it. */
  assign(assignees: ReadonlyArray<string>): Promise<raw.IssueChange> { return run(() => this.#raw.assign(assignees)); }
  close(options: raw.CloseIssueOptions = {}): Promise<raw.IssueChange> {
    return run(() => this.#raw.close(options));
  }
  reopen(): Promise<raw.IssueChange> { return run(() => this.#raw.reopen()); }
}
