import {
  Raw,
  type Repository,
  type RawOp,
  type CobRef,
  type CobManifest,
  type RawJsonValue,
} from 'radicle-sdk';
// @ts-expect-error private construction
new Raw();
// @ts-expect-error no public native conversion factory
Raw.fromNative({});
async function use(repo: Repository): Promise<void> {
  const raw: Raw = Raw.open(repo);
  const rid: string = raw.rid;
  const types: ReadonlyArray<string> = await raw.typenames();
  const ids: ReadonlyArray<string> = await raw.entities('xyz.radicle.issue');
  const refs: ReadonlyArray<CobRef> = await raw.refs(types[0], ids[0]);
  const ops: ReadonlyArray<RawOp> = await raw.history(types[0], ids[0]);
  const manifest: CobManifest = ops[0].manifest;
  const version: bigint = manifest.version;
  const seconds: bigint = ops[0].timestamp;
  const actions: ReadonlyArray<Uint8Array> = ops[0].actions;
  actions[0][0] = 32; // independent mutable byte views
  const parents: ReadonlyArray<string> = ops[0].parents;
  const related: ReadonlyArray<string> = ops[0].related;
  const identity: string | undefined = ops[0].identity;
  const json: ReadonlyArray<RawJsonValue> = ops[0].actionsJson();
  // @ts-expect-error timestamps are exact raw seconds, not Date-safe milliseconds
  const date: number = seconds;
  // @ts-expect-error containers frozen
  actions.push(new Uint8Array());
  // @ts-expect-error manifest immutable
  manifest.version = 2n;
  // @ts-expect-error snapshot decoding function is immutable too
  ops[0].actionsJson = () => [];
  // @ts-expect-error refs immutable
  refs[0].target = 'other';
  // @ts-expect-error no raw writes
  raw.create('xyz.radicle.issue');
  void [rid, version, parents, related, identity, json, date];
}
void use;
