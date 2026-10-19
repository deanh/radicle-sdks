//! Node binding: language-owned FFI modules, no shared core.
#[cfg(not(panic = "unwind"))]
compile_error!("radicle-sdk requires panic=unwind to contain worker task panics");

mod comment;
mod error;
mod ids;
mod issues;
mod locks;
mod patches;
mod policy;
mod probes;
mod profile;
mod project;
mod raw;
mod repository;
mod routing;
mod tasks;
mod timestamps;

// Keep native free-function entry points exported from the crate root.
pub use ids::{parse_did, parse_node_id, parse_object_id, parse_oid, parse_repo_id};
pub use probes::{probe_panic, probe_sync_panic, probe_timestamp};
pub use project::{InitOptions, checkout_project, fork_project, init_project};

#[cfg(test)]
mod tests {
    static_assertions::assert_impl_all!(radicle::profile::Signer: Send);
    static_assertions::assert_not_impl_any!(radicle::profile::Signer: Sync);
    static_assertions::assert_impl_all!(radicle::Profile: Send, Sync);
    static_assertions::assert_impl_all!(radicle::node::Database: Send, Sync);
    static_assertions::assert_impl_all!(crate::policy::NativePolicyStore: Send, Sync);
    static_assertions::assert_not_impl_any!(radicle::node::policy::store::Store<radicle::node::policy::store::Write>: Sync);
    static_assertions::assert_impl_all!(crate::routing::NativeRoutingStore: Send, Sync);
    static_assertions::assert_impl_all!(crate::raw::NativeRaw: Send, Sync);
    static_assertions::assert_impl_all!(radicle::storage::git::Storage: Send, Sync);
    static_assertions::assert_impl_all!(radicle::storage::git::Repository: Send);
    static_assertions::assert_not_impl_any!(radicle::storage::git::Repository: Sync);
    static_assertions::assert_impl_all!(crate::patches::NativePatches: Send, Sync);
    static_assertions::assert_impl_all!(crate::patches::NativePatchMut: Send, Sync);
}
