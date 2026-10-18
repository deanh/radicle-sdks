//! Read-only routing queries over upstream's thread-safe node database.
use crate::{
    error::err,
    ids::rid,
    profile::NativeProfile,
    tasks::{Work, count_number, work},
    timestamps::{metadata_timestamp, unsigned_millis},
};
use napi::bindgen_prelude::{AsyncTask, Either, Null};
use napi_derive::napi;
use radicle::node::{Database, routing::Store as _};
#[napi(object)]
pub struct RoutingEntry {
    #[napi(readonly)]
    pub rid: String,
    #[napi(readonly)]
    pub node_id: String,
    #[napi(readonly)]
    pub timestamp: Either<f64, Null>,
}
#[napi]
#[derive(Clone)]
pub struct NativeRoutingStore {
    db: Database,
}
#[napi]
impl NativeRoutingStore {
    #[napi(catch_unwind, ts_return_type = "Promise<NativeRoutingStore>")]
    pub fn from_profile(profile: &NativeProfile) -> AsyncTask<Work<NativeRoutingStore>> {
        let profile = profile.inner.clone();
        work(move || {
            Ok(Self {
                db: profile.database().map_err(|e| err("DATABASE", e))?,
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<string>>")]
    pub fn get(&self, repo_id: String) -> AsyncTask<Work<Vec<String>>> {
        let db = self.db.clone();
        work(move || {
            let id = rid(&repo_id)?;
            let mut ids: Vec<_> = db
                .get(&id)
                .map_err(|e| err("DATABASE", e))?
                .into_iter()
                .map(|n| n.to_string())
                .collect();
            ids.sort();
            Ok(ids)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<string>>")]
    pub fn get_inventory(&self, node_id: String) -> AsyncTask<Work<Vec<String>>> {
        let db = self.db.clone();
        work(move || {
            let id = node_id.parse().map_err(|e| err("ARG_VALUE", e))?;
            let mut ids: Vec<_> = db
                .get_inventory(&id)
                .map_err(|e| err("DATABASE", e))?
                .into_iter()
                .map(|r| r.to_string())
                .collect();
            ids.sort();
            Ok(ids)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<number | null>")]
    pub fn entry(&self, repo_id: String, node_id: String) -> AsyncTask<Work<Either<f64, Null>>> {
        let db = self.db.clone();
        work(move || {
            let rid = rid(&repo_id)?;
            let nid = node_id.parse().map_err(|e| err("ARG_VALUE", e))?;
            Ok(
                match db.entry(&rid, &nid).map_err(|e| err("DATABASE", e))? {
                    Some(t) => metadata_timestamp(unsigned_millis(*t)),
                    None => Either::B(Null),
                },
            )
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<RoutingEntry>>")]
    pub fn entries(&self) -> AsyncTask<Work<Vec<RoutingEntry>>> {
        let db = self.db.clone();
        work(move || {
            let mut entries = Vec::new();
            for (rid, nid) in db.entries().map_err(|e| err("DATABASE", e))? {
                let timestamp = match db.entry(&rid, &nid).map_err(|e| err("DATABASE", e))? {
                    Some(t) => metadata_timestamp(unsigned_millis(*t)),
                    None => Either::B(Null),
                };
                entries.push(RoutingEntry {
                    rid: rid.to_string(),
                    node_id: nid.to_string(),
                    timestamp,
                });
            }
            entries.sort_by(|a, b| (&a.rid, &a.node_id).cmp(&(&b.rid, &b.node_id)));
            Ok(entries)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<number>")]
    pub fn count(&self, repo_id: String) -> AsyncTask<Work<f64>> {
        let db = self.db.clone();
        work(move || {
            let rid = rid(&repo_id)?;
            count_number(db.get(&rid).map_err(|e| err("DATABASE", e))?.len())
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<number>")]
    pub fn total(&self) -> AsyncTask<Work<f64>> {
        let db = self.db.clone();
        work(move || count_number(db.len().map_err(|e| err("DATABASE", e))?))
    }
    #[napi(catch_unwind, ts_return_type = "Promise<boolean>")]
    pub fn is_empty(&self) -> AsyncTask<Work<bool>> {
        let db = self.db.clone();
        work(move || db.is_empty().map_err(|e| err("DATABASE", e)))
    }
}
