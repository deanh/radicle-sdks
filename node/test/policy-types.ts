import {
  Profile,
  PolicyStore,
  RoutingStore,
  RadicleError,
  type FollowPolicyInfo,
  type SeedPolicyInfo,
  type RoutingEntry,
} from 'radicle-sdk';

// @ts-expect-error private constructor
new PolicyStore();
// @ts-expect-error private constructor
new RoutingStore();
// @ts-expect-error private native handles are never exposed
PolicyStore.fromNative({});
// @ts-expect-error no routing writes exposed
RoutingStore.prototype.addInventory('rid');
async function use(profile: Profile, nid: string, rid: string): Promise<void> {
  const store: PolicyStore = await profile.policyStore();
  const reader: PolicyStore = await PolicyStore.reader(profile);
  const opened: PolicyStore = await PolicyStore.open(store.path);
  const changed: boolean = await store.follow(nid, { alias: 'friend' });
  await store.unfollow(nid);
  await store.seed(rid, { scope: 'followed' });
  await store.unseed(rid);
  await store.setFollowPolicy(nid, 'block');
  await store.setSeedPolicy(rid, 'allow');
  await store.unblockRid(rid);
  await store.unblockNid(nid);
  const a: boolean = await reader.isFollowing(nid);
  const b: boolean = await reader.isBlocked(nid);
  const c: boolean = await reader.isSeeding(rid);
  const following: FollowPolicyInfo | null = await reader.followPolicy(nid);
  const seeding: SeedPolicyInfo | null = await reader.seedPolicy(rid);
  const allFollow: ReadonlyArray<FollowPolicyInfo> = await reader.followPolicies();
  const allSeed: ReadonlyArray<SeedPolicyInfo> = await reader.seedPolicies();
  const pairs: ReadonlyArray<readonly [string, string]> = await reader.nodesByAlias('fri');
  if (following) {
    const policy: 'allow' | 'block' = following.policy;
    const alias: string | undefined = following.alias;
    // @ts-expect-error readonly snapshots
    following.alias = 'other';
    void [policy, alias];
  }
  if (seeding) {
    const scope: 'all' | 'followed' | undefined = seeding.scope;
    void scope;
  }
  // @ts-expect-error lists readonly
  allFollow.push(following!);
  // @ts-expect-error tuple readonly
  pairs[0][0] = 'different';
  // @ts-expect-error no unknown policy domain
  await store.setSeedPolicy(rid, 'permit');
  // @ts-expect-error no arbitrary scope
  await store.seed(rid, { scope: 'everyone' });
  // @ts-expect-error no null optional alias coercion
  await store.follow(nid, { alias: null });
  const r: RoutingStore = await profile.routingStore();
  const independent: RoutingStore = await RoutingStore.fromProfile(profile);
  const nodes: ReadonlyArray<string> = await r.get(rid);
  const repos: ReadonlyArray<string> = await r.getInventory(nid);
  const time: number | null = await r.entry(rid, nid);
  const entries: ReadonlyArray<RoutingEntry> = await r.entries();
  const timestamp: number | null = entries[0].timestamp;
  const count: number = await r.count(rid);
  const total: number = await r.total();
  const empty: boolean = await r.isEmpty();
  // @ts-expect-error snapshots readonly
  entries[0].timestamp = 1;
  // @ts-expect-error factories async
  const immediate: PolicyStore = PolicyStore.fromProfile(profile);
  const errors = [new RadicleError('DATABASE', 'db'), new RadicleError('POLICY', 'policy')];
  void [
    opened,
    changed,
    a,
    b,
    c,
    allSeed,
    independent,
    nodes,
    repos,
    time,
    timestamp,
    count,
    total,
    empty,
    immediate,
    errors,
  ];
}
void use;
