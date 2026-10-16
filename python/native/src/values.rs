//! Binding-only snapshot hashing; no common DTO schema.
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};
use std::sync::OnceLock;

/// Hash the complete Rust snapshot with a shared, process-local random key.
/// Unlike PyO3's generated hash (DefaultHasher::new), this does not use fixed
/// keys for peer-controlled content. Hashes are intentionally not persistent
/// and are independent of PYTHONHASHSEED; no Python field copies are needed.
pub(crate) fn snapshot_hash(value: &impl Hash) -> u64 {
    static STATE: OnceLock<RandomState> = OnceLock::new();
    STATE.get_or_init(RandomState::new).hash_one(value)
}
