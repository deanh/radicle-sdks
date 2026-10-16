//! Node binding: language-owned FFI modules, no shared core.
#[cfg(not(panic = "unwind"))]
compile_error!("radicle-sdk requires panic=unwind to contain worker task panics");

mod comment;
mod error;
mod ids;
mod issues;
mod locks;
mod patches;
mod probes;
mod profile;
mod project;
mod repository;
mod tasks;
mod timestamps;

// Keep native free-function entry points exported from the crate root.
pub use probes::{probe_panic, probe_sync_panic, probe_timestamp};
pub use project::{InitOptions, checkout_project, fork_project, init_project};

#[cfg(test)]
mod tests {
    static_assertions::assert_impl_all!(radicle::profile::Signer: Send);
    static_assertions::assert_not_impl_any!(radicle::profile::Signer: Sync);
    static_assertions::assert_impl_all!(radicle::Profile: Send, Sync);
    static_assertions::assert_impl_all!(radicle::storage::git::Storage: Send, Sync);
    static_assertions::assert_impl_all!(radicle::storage::git::Repository: Send);
    static_assertions::assert_not_impl_any!(radicle::storage::git::Repository: Sync);
    static_assertions::assert_impl_all!(crate::patches::NativePatches: Send, Sync);
    static_assertions::assert_impl_all!(crate::patches::NativePatchMut: Send, Sync);
}
