//! Profile, Home, Aliases, and Signer wrappers.
//!
//! Handles are owned: they hold `Arc<radicle::Profile>` / paths, never
//! borrowed references into upstream structures (PLAN §5.3 — reopen-per-op,
//! no lifetime gymnastics across the FFI boundary).

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::crypto::Signer as _; // for `Signer::public_key()` on profile::Signer
use radicle::identity::Did;
use radicle::node::{Alias, AliasStore, NodeId};
use radicle::profile::Home as RawHome;

use crate::error;

// ─── Home ────────────────────────────────────────────────────────────────────

#[pyclass(name = "Home", module = "radiclepy")]
#[derive(Clone)]
pub struct PyHome {
    pub(crate) inner: RawHome,
}

#[pymethods]
impl PyHome {
    /// Create a home handle at `path`, creating directories if needed.
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: RawHome::new(path).map_err(error::io_err)?,
        })
    }

    /// Load an existing home from `path` (validates the layout).
    #[classmethod]
    fn load(_cls: &Bound<'_, PyType>, path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: RawHome::load(path).map_err(error::io_err)?,
        })
    }

    /// Resolve the home from the environment: `RAD_HOME`, else `~/.radicle`.
    #[classmethod]
    fn resolve(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        Ok(Self {
            inner: radicle::profile::home().map_err(error::io_err)?,
        })
    }

    /// Canonical filesystem path of this home.
    #[getter]
    fn path(&self) -> PyResult<&str> {
        self.inner.path().to_str().ok_or_else(|| {
            error::value_err("home path is not valid UTF-8")
        })
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Home({})", self.inner.path().display()))
    }
}

// ─── Aliases ─────────────────────────────────────────────────────────────────

/// Lookup facade over the profile's alias stores (PLAN §3) — NOT a
/// collection: upstream only supports `alias(node_id)` and `reverse_lookup`.
#[pyclass(name = "Aliases", module = "radiclepy")]
pub struct PyAliases {
    profile: Arc<radicle::Profile>,
}

#[pymethods]
impl PyAliases {
    /// The alias of `node_id`, if any. Raises `ValueError` for invalid node ids.
    fn get(&self, node_id: &str) -> PyResult<Option<String>> {
        let nid = NodeId::from_str(node_id)
            .map_err(|err| error::value_err(format!("invalid NodeId: {err}")))?;
        Ok(self.profile.aliases().alias(&nid).map(|a| a.to_string()))
    }

    /// All aliases matching `alias`, mapped to their node ids.
    fn reverse_lookup(&self, alias: &str) -> PyResult<BTreeMap<String, Vec<String>>> {
        let alias = Alias::from_str(alias)
            .map_err(|err| error::value_err(format!("invalid Alias: {err}")))?;
        Ok(self
            .profile
            .aliases()
            .reverse_lookup(&alias)
            .into_iter()
            .map(|(a, nids)| (a.to_string(), nids.into_iter().map(|n| n.to_string()).collect()))
            .collect())
    }

    fn __repr__(&self) -> String {
        "Aliases()".to_string()
    }
}

// ─── Signer ──────────────────────────────────────────────────────────────────

#[pyclass(name = "Signer", module = "radiclepy", unsendable)]
pub struct PySigner {
    pub(crate) inner: radicle::profile::Signer,
}

#[pymethods]
impl PySigner {
    /// DID corresponding to this signer's key (`did:key:z…`).
    #[getter]
    fn did(&self) -> String {
        Did::from(*self.inner.public_key()).to_string()
    }

    /// Node id / public key corresponding to this signer (`z6Mk…`).
    #[getter]
    fn node_id(&self) -> String {
        self.inner.public_key().to_string()
    }

    fn __repr__(&self) -> String {
        format!("Signer({})", self.inner.public_key())
    }
}

// ─── Profile ─────────────────────────────────────────────────────────────────

#[pyclass(name = "Profile", module = "radiclepy")]
#[derive(Clone)]
pub struct PyProfile {
    pub(crate) inner: Arc<radicle::Profile>,
}

/// Fill 32 bytes from the OS RNG — the secure seed for `Profile.init`
/// (PLAN §5.7; upstream `Seed` has no production `generate()`).
fn secure_seed() -> PyResult<[u8; 32]> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed)
        // Operational failure, not invalid caller input → radiclepy.Error (§5.8).
        .map_err(|err| error::op_err(format!("OS RNG unavailable: {err}")))?;
    Ok(seed)
}

/// Input order is deliberate: the alias is validated BEFORE the home is
/// created, so a rejected call leaves no filesystem state behind.
fn init_with_seed(
    home_path: &str,
    alias: &str,
    passphrase: Option<&str>,
    seed: [u8; 32],
) -> PyResult<PyProfile> {
    let alias = Alias::from_str(alias)
        .map_err(|err| error::value_err(format!("invalid Alias: {err}")))?;
    let passphrase = passphrase.map(|p| radicle::crypto::ssh::keystore::Passphrase::from(p.to_owned()));
    // Only now — after all input validation — touch the filesystem.
    let home = RawHome::new(home_path).map_err(error::io_err)?;
    let profile = radicle::Profile::init(home, alias, passphrase, radicle::crypto::Seed::new(seed))
        .map_err(error::profile_err)?;
    Ok(PyProfile {
        inner: Arc::new(profile),
    })
}

#[pymethods]
impl PyProfile {
    /// Load the local profile from `RAD_HOME` or `~/.radicle`.
    #[classmethod]
    fn load(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        let profile = radicle::Profile::load().map_err(error::profile_err)?;
        Ok(Self {
            inner: Arc::new(profile),
        })
    }

    /// Initialize a new profile at `home_path` with a **secure random seed**
    /// from the OS RNG. Public contract (PLAN §5.7): this is the only seed
    /// source exposed publicly; deterministic seeding is test-only via
    /// `Profile._from_seed`.
    #[classmethod]
    #[pyo3(signature = (home_path, alias, passphrase = None))]
    fn init(
        _cls: &Bound<'_, PyType>,
        home_path: &str,
        alias: &str,
        passphrase: Option<&str>,
    ) -> PyResult<Self> {
        init_with_seed(home_path, alias, passphrase, secure_seed()?)
    }

    /// TEST-ONLY (compiled exclusively behind the `test-helpers` cargo
    /// feature — absent from release wheels, per PLAN §5.7): initialize a
    /// profile from a fixed 32-byte seed. Never part of the shipped API;
    /// an underscore prefix alone would not be a boundary, hence cfg-gating.
    #[cfg(feature = "test-helpers")]
    #[classmethod]
    #[pyo3(signature = (home_path, alias, seed, passphrase = None))]
    fn _from_seed(
        _cls: &Bound<'_, PyType>,
        home_path: &str,
        alias: &str,
        seed: &[u8],
        passphrase: Option<&str>,
    ) -> PyResult<Self> {
        let seed: [u8; 32] = seed
            .try_into()
            .map_err(|_| error::value_err("seed must be exactly 32 bytes"))?;
        init_with_seed(home_path, alias, passphrase, seed)
    }

    /// This profile's DID (`did:key:z…`).
    #[getter]
    fn did(&self) -> String {
        self.inner.did().to_string()
    }

    /// This profile's node id / public key (`z6Mk…`).
    #[getter]
    fn node_id(&self) -> String {
        self.inner.id().to_string()
    }

    /// The filesystem home backing this profile.
    #[getter]
    fn home(&self) -> PyHome {
        PyHome {
            inner: self.inner.home().clone(),
        }
    }

    /// Alias lookup facade (not a collection — see API.md).
    #[getter]
    fn aliases(&self) -> PyAliases {
        PyAliases {
            profile: self.inner.clone(),
        }
    }

    /// Load the signer: unencrypted keystore, `RAD_PASSPHRASE`, or ssh-agent,
    /// matching `radicle::Profile::signer()`.
    fn signer(&self) -> PyResult<PySigner> {
        let signer = self.inner.signer().map_err(error::signer_err)?;
        Ok(PySigner { inner: signer })
    }

    fn __repr__(&self) -> String {
        format!("Profile({})", self.inner.did())
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHome>()?;
    m.add_class::<PyAliases>()?;
    m.add_class::<PySigner>()?;
    m.add_class::<PyProfile>()?;
    Ok(())
}
