//! Binding-only repository write-lock registry.
use crate::{error::err, repository::NativeRepository};
use napi::Result;
use radicle::identity::RepoId;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, Weak},
};
// Weak entries avoid keeping an unbounded number of repository mutexes alive.
// Clean dead keys on acquisition; every active operation owns the same Arc.
type LockKey = (PathBuf, RepoId);
type Locks = HashMap<LockKey, Weak<Mutex<()>>>;
pub(crate) fn repo_lock(repo: &NativeRepository) -> Result<Arc<Mutex<()>>> {
    static LOCKS: OnceLock<Mutex<Locks>> = OnceLock::new();
    let key = (
        repo.storage
            .path()
            .canonicalize()
            .map_err(|e| err("STORAGE", e))?,
        repo.rid,
    );
    let mut locks = LOCKS
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| err("INTERNAL", e))?;
    locks.retain(|_, v| v.strong_count() != 0);
    let entry = locks.entry(key).or_default();
    if let Some(lock) = entry.upgrade() {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    *entry = Arc::downgrade(&lock);
    Ok(lock)
}
