//! Generic read-only COB inspection. Causal parents, never Git's conflated
//! identity/related parent list, define history. Each operation reopens the repo.
use crate::{
    error::err,
    ids::oid,
    repository::NativeRepository,
    tasks::{Work, work},
};
use napi::{
    Result,
    bindgen_prelude::{AsyncTask, BigInt, Uint8Array},
};
use napi_derive::napi;
use radicle::cob::{change::store::Storage as _, object::Storage as _};
use radicle::{
    cob::{self, TypeName},
    git::{self, Oid},
};
use std::collections::{BTreeMap, BTreeSet};

#[napi(object)]
pub struct CobRef {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub target: String,
}
#[napi(object)]
pub struct CobManifest {
    #[napi(readonly)]
    pub type_name: String,
    #[napi(readonly)]
    pub version: BigInt,
}
#[napi(object)]
pub struct RawOpData {
    #[napi(readonly)]
    pub id: String,
    #[napi(readonly)]
    pub actor: String,
    #[napi(readonly)]
    pub timestamp: BigInt,
    #[napi(readonly, ts_type = "ReadonlyArray<Uint8Array>")]
    pub actions: Vec<Uint8Array>,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub parents: Vec<String>,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub related: Vec<String>,
    #[napi(readonly)]
    pub identity: Option<String>,
    #[napi(readonly)]
    pub manifest: CobManifest,
}
fn typename(value: &str) -> Result<TypeName> {
    value.parse().map_err(|e| err("ARG_VALUE", e))
}
fn typed_id(name: &str) -> Option<cob::TypedId> {
    let reference = git::fmt::RefStr::try_from_str(name).ok()?;
    match reference.to_namespaced() {
        Some(ns) => cob::TypedId::from_namespaced(&ns).ok().flatten(),
        None => reference
            .qualified()
            .and_then(|r| cob::TypedId::from_qualified(&r).ok().flatten()),
    }
}
fn load(repo: &radicle::storage::git::Repository, id: Oid) -> Result<RawOpData> {
    let entry = repo
        .load(id)
        .map_err(|e| err("COB", format!("load entry {id}: {e}")))?;
    Ok(RawOpData {
        id: id.to_string(),
        actor: entry.author().to_string(),
        timestamp: entry.timestamp.into(),
        actions: entry.contents().iter().map(|b| b.to_vec().into()).collect(),
        parents: entry.parents.iter().map(ToString::to_string).collect(),
        related: entry.related.iter().map(ToString::to_string).collect(),
        identity: entry.resource().map(|id| id.to_string()),
        manifest: CobManifest {
            type_name: entry.manifest.type_name.to_string(),
            version: u64::try_from(usize::from(entry.manifest.version))
                .map_err(|e| err("INVALID_DATA", e))?
                .into(),
        },
    })
}
// Kahn ordering with the same lexicographically-smallest ready ID tie-break as
// Python. Keep graph ordering separate from byte materialization for unit tests.
fn order(graph: BTreeMap<String, Vec<String>>) -> Result<Vec<String>> {
    let mut degrees = BTreeMap::new();
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, parents) in &graph {
        let parents: BTreeSet<_> = parents.iter().filter(|p| graph.contains_key(*p)).collect();
        degrees.insert(id.clone(), parents.len());
        for parent in parents {
            children.entry(parent.clone()).or_default().push(id.clone());
        }
    }
    let mut ready: BTreeSet<_> = degrees
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut out = Vec::with_capacity(graph.len());
    while let Some(id) = ready.pop_first() {
        if let Some(children) = children.get(&id) {
            for child in children {
                let degree = degrees.get_mut(child).expect("loaded child");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
        out.push(id);
    }
    if out.len() != graph.len() {
        return Err(err("COB", "corrupt COB history: causal cycle"));
    }
    Ok(out)
}
#[napi]
#[derive(Clone)]
pub struct NativeRaw {
    repo: NativeRepository,
}
#[napi]
impl NativeRaw {
    #[napi(factory, catch_unwind)]
    pub fn open(repository: &NativeRepository) -> Self {
        Self {
            repo: repository.clone(),
        }
    }
    #[napi(getter, catch_unwind)]
    pub fn rid(&self) -> String {
        self.repo.rid.to_string()
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<string>>")]
    pub fn typenames(&self) -> AsyncTask<Work<Vec<String>>> {
        let me = self.clone();
        work(move || {
            let repo = me.repo.open()?;
            let mut names = BTreeSet::new();
            for r in repo.backend.references().map_err(|e| err("GIT", e))? {
                let r = r.map_err(|e| err("GIT", e))?;
                if let Some(id) = typed_id(r.name().map_err(|e| err("GIT", e))?) {
                    names.insert(id.type_name.to_string());
                }
            }
            Ok(names.into_iter().collect())
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<string>>")]
    pub fn entities(&self, type_name: String) -> AsyncTask<Work<Vec<String>>> {
        let me = self.clone();
        work(move || {
            let name = typename(&type_name)?;
            let repo = me.repo.open()?;
            let mut ids = BTreeSet::new();
            for r in repo.backend.references().map_err(|e| err("GIT", e))? {
                let r = r.map_err(|e| err("GIT", e))?;
                if let Some(id) = typed_id(r.name().map_err(|e| err("GIT", e))?)
                    && id.type_name == name
                {
                    ids.insert(id.id.to_string());
                }
            }
            Ok(ids.into_iter().collect())
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<CobRef>>")]
    pub fn refs(&self, type_name: String, entity_id: String) -> AsyncTask<Work<Vec<CobRef>>> {
        let me = self.clone();
        work(move || {
            let name = typename(&type_name)?;
            let id = oid(&entity_id)?;
            let repo = me.repo.open()?;
            let objects = repo.objects(&name, &id).map_err(|e| err("COB", e))?;
            let mut out: Vec<_> = objects
                .iter()
                .map(|r| CobRef {
                    name: r.name.to_string(),
                    target: r.target.id.to_string(),
                })
                .collect();
            out.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(out)
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<RawOpData>>")]
    pub fn history(&self, type_name: String, entity_id: String) -> AsyncTask<Work<Vec<RawOpData>>> {
        let me = self.clone();
        work(move || {
            let name = typename(&type_name)?;
            let id = oid(&entity_id)?;
            let repo = me.repo.open()?;
            let objects = repo.objects(&name, &id).map_err(|e| err("COB", e))?;
            let mut stack: Vec<Oid> = objects.iter().map(|r| r.target.id).collect();
            stack.sort();
            stack.dedup();
            let mut ops = BTreeMap::new();
            while let Some(id) = stack.pop() {
                if ops.contains_key(&id.to_string()) {
                    continue;
                }
                let op = load(&repo, id)?;
                for parent in &op.parents {
                    stack.push(parent.parse().map_err(|e| err("COB", e))?);
                }
                ops.insert(op.id.clone(), op);
            }
            let graph = ops
                .iter()
                .map(|(id, op)| (id.clone(), op.parents.clone()))
                .collect();
            order(graph)?
                .into_iter()
                .map(|id| {
                    ops.remove(&id)
                        .ok_or_else(|| err("INTERNAL", "missing ordered entry"))
                })
                .collect()
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_cob_refs_skip_lookalikes_and_invalid_identifiers() {
        let id = "0".repeat(40);
        for name in [
            format!("refs/cobs/xyz.test.item/{id}"),
            format!(
                "refs/namespaces/z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK/refs/cobs/xyz.test.item/{id}"
            ),
        ] {
            assert!(typed_id(&name).is_some());
        }
        for name in [
            format!("refs/tags/xyz.test.item/{id}"),
            format!("refs/cobs/bad_type/{id}"),
            "refs/cobs/xyz.test.item/not-an-id".into(),
        ] {
            assert!(typed_id(&name).is_none());
        }
    }
    #[test]
    fn causal_order_deduplicates_parents_and_breaks_ties_lexically() {
        let graph = BTreeMap::from([
            ("a".into(), vec!["c".into(), "c".into()]),
            ("b".into(), vec![]),
            ("c".into(), vec![]),
            ("d".into(), vec!["a".into(), "b".into()]),
        ]);
        assert_eq!(order(graph).unwrap(), ["b", "c", "a", "d"]);
    }
    #[test]
    fn corrupt_causal_cycles_reject_instead_of_returning_partial_history() {
        let graph = BTreeMap::from([
            ("a".into(), vec!["b".into()]),
            ("b".into(), vec!["a".into()]),
        ]);
        assert!(order(graph).unwrap_err().reason.starts_with("RADICLE:COB:"));
        // Self edges cannot exist in valid content-addressed history either.
        assert!(order(BTreeMap::from([("a".into(), vec!["a".into()])])).is_err());
    }
}
