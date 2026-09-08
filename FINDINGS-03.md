# Phase 3 — Policy & routing stores: spike findings

Date: 2026-07-28 · Plan: [`PLAN.md`](./PLAN.md) §7 Phase 3 (exit criterion: "verify DB open/lock semantics")

All facts verified against the crates.io `radicle = "=0.25.1"` source.

## Store topology (three stores, not one database)

- **Policy store** — its **own sqlite file**: `<home>/node/policies.db`
  (`node::POLICIES_DB_FILE`). Upstream type: `node::policy::store::Store<T>`
  with `T ∈ {Read, Write}`; reads live in `impl<T> Store<T>` (both), writes
  only on `Store<Write>`. There is **no** `Profile::policy()` accessor —
  `Profile::policies()` returns a *config-combining* reader
  (`policy::config::Config<Read>`). **Binding decision:** `PolicyStore`
  mirrors the **raw `Store` trait surface** (1:1 methods, explicit), not the
  config-combining view; path derived from upstream layout
  (`profile.node().join(POLICIES_DB_FILE)`).
- **Routing store** — `Home::routing(config)` is *literally*
  `Home::database(config)`: the routing table lives in the node database
  (`<home>/node/node.db`) and `node::Database` implements
  `node::routing::Store`. **Binding decision:** `RoutingStore` holds the
  named `node::Database` opened via `Profile::database()` (read-only); the
  `impl Store` return types upstream returns are unnameable, so we go to the
  concrete type underneath — identical construction path.
- **Address store** — `Home::addresses(config)` = `database_mut(config)`
  (the same node db, mutable view). **Decision (closes PLAN open item 1):
  parked to a later phase.** The trait is write/connection-bookkeeping-shaped
  (`attempted`/`connected`/`disconnected`/`record_ip`/bans); a read-only Phase
  3 slice would cover `get`/`addresses_of`/`entries` only, and mixing in
  mutable views contradicts the read-first phase ordering. Not dropped —
  deferred with its reads + Phase 5 writes together.

## DB open/lock semantics (the Phase 3 exit criterion)

- `policy::store::Store::reader(path)` — sqlite **read-only** connection
  (upstream comment: "useful to have multiple open databases, as no locking
  is required"), busy timeout on reads.
- `policy::store::Store::open(path)` — read-write, sqlite busy timeout on
  writes; writers serialize (single-writer sqlite). The node process holds
  the same file; our read-write opens coexist via sqlite locking — no
  advisory lock is taken upstream, so **don't hold write transactions open**.
- `node::Database::reader(path, config)` opens with **read-only flags and
  runs no migrations** (`open_thread_safe_with_flags(..., with_read_only())`).
  A **missing** `node.db` therefore raises `DatabaseError` (sqlite code 14,
  "unable to open database file"). Note: `Profile::init` creates and
  migrates `node.db`, so a freshly-initialized profile opens fine with an
  **empty** routing table — those two facts were initially conflated here
  and corrected after review (regression test: delete `node.db` →
  `DatabaseError`).
- Two `PolicyStore` handles on one file in one process behave as two sqlite
  connections (tested: reader sees committed writer state — statement-level
  autocommit upstream).

## Type shapes

- `node::Timestamp` is radicle's own newtype: **milliseconds since epoch**
  (`Deref<Target = u64>`, `EPOCH`, `From<LocalTime>`); *not*
  `radicle_localtime::LocalTime` itself. Binding surfaces raw **epoch
  millis** ints.
- `routing::Store::entries()` yields only `(RepoId, NodeId)` pairs;
  timestamps require `entry(rid, node)` per pair. Binding `entries()` is an
  eager list doing that N+1 lookup (local sqlite; tables are small).
- `policy::FollowPolicy { nid, alias: Option<Alias>, policy }`;
  `SeedPolicy { rid, SeedingPolicy::Allow { scope } | Block }` → binding
  `SeedPolicyInfo.scope` is `None` iff blocked.
- `nodes_by_alias` is upstream a **fuzzy case-insensitive substring**
  (`LIKE %alias%`) lookup returning `(NodeId, Alias)` pairs — preserved and
  documented (surprising for anyone expecting exact match).
- `is_blocked(nid)` is an explicit state distinct from "not followed".
  **Deletion semantics (upstream SQL, corrected post-review):**
  `unfollow`/`unseed` are *unconditional* `DELETE ... WHERE id = ?` — they
  remove **any** policy row, allow or block; `unblock_nid`/`unblock_rid` are
  `DELETE ... WHERE id = ? AND policy = 'block'` — they remove **only**
  block rows (and return `false` when the row is an allow row).

## Binding-level validations (ValueError before any sqlite call)

NodeIds/RepoIds/aliases parse first; policy ∈ {"allow","block"};
scope ∈ {"all","followed"}; read-only handle + write method = `StorageError`
(our own message, not an upstream error).
