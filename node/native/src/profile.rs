//! Profile setup and thread-safe signer handles.
use crate::repository::NativeStorage;
use crate::{
    error::err,
    tasks::{Work, work},
};
use napi::{Result, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use radicle::crypto::Signer as _;
use radicle::node::AliasStore;
use std::sync::{Arc, Mutex};

pub(crate) fn path_string(path: &std::path::Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| err("INVALID_DATA", "stored path is not valid UTF-8"))
}

#[napi]
pub struct NativeHome {
    inner: radicle::profile::Home,
}
#[napi]
impl NativeHome {
    #[napi(catch_unwind, ts_return_type = "Promise<NativeHome>")]
    pub fn create(path: String) -> AsyncTask<Work<NativeHome>> {
        work(move || {
            Ok(Self {
                inner: radicle::profile::Home::new(path).map_err(|e| err("STORAGE", e))?,
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativeHome>")]
    pub fn load(path: String) -> AsyncTask<Work<NativeHome>> {
        work(move || {
            Ok(Self {
                inner: radicle::profile::Home::load(path).map_err(|e| err("STORAGE", e))?,
            })
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativeHome>")]
    pub fn resolve() -> AsyncTask<Work<NativeHome>> {
        work(move || {
            Ok(Self {
                inner: radicle::profile::home().map_err(|e| err("STORAGE", e))?,
            })
        })
    }
    #[napi(getter, catch_unwind)]
    pub fn path(&self) -> Result<String> {
        path_string(self.inner.path())
    }
}

// Transport entries instead of a map: JS setters such as __proto__ must not
// consume aliases before the public wrapper creates own data properties.
#[napi(object)]
pub struct AliasMatch {
    #[napi(readonly)]
    pub alias: String,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub node_ids: Vec<String>,
}
#[napi]
pub struct NativeAliases {
    profile: Arc<radicle::Profile>,
}
#[napi]
impl NativeAliases {
    #[napi(catch_unwind, ts_return_type = "Promise<string | null>")]
    pub fn get(&self, node_id: String) -> AsyncTask<Work<Option<String>>> {
        let profile = self.profile.clone();
        work(move || {
            let id = node_id.parse().map_err(|e| err("ARG_VALUE", e))?;
            Ok(profile.aliases().alias(&id).map(|a| a.to_string()))
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<ReadonlyArray<AliasMatch>>")]
    pub fn reverse_lookup(&self, alias: String) -> AsyncTask<Work<Vec<AliasMatch>>> {
        let profile = self.profile.clone();
        work(move || {
            let alias = alias.parse().map_err(|e| err("ARG_VALUE", e))?;
            Ok(profile
                .aliases()
                .reverse_lookup(&alias)
                .into_iter()
                .map(|(a, ids)| AliasMatch {
                    alias: a.to_string(),
                    node_ids: ids.into_iter().map(|id| id.to_string()).collect(),
                })
                .collect())
        })
    }
}

#[napi]
#[derive(Clone)]
pub struct NativeProfile {
    pub(crate) inner: Arc<radicle::Profile>,
}
#[napi]
impl NativeProfile {
    #[napi(catch_unwind, ts_return_type = "Promise<NativeProfile>")]
    pub fn init(
        home: String,
        alias: String,
        passphrase: Option<String>,
    ) -> AsyncTask<Work<NativeProfile>> {
        work(move || {
            let alias = alias.parse().map_err(|e| err("ARG_VALUE", e))?;
            let mut seed = [0; 32];
            getrandom::fill(&mut seed).map_err(|e| err("INTERNAL", e))?;
            let home = radicle::profile::Home::new(home).map_err(|e| err("STORAGE", e))?;
            let profile = radicle::Profile::init(
                home,
                alias,
                passphrase.map(Into::into),
                radicle::crypto::Seed::new(seed),
            )
            .map_err(|e| err("IDENTITY", e))?;
            Ok(Self {
                inner: Arc::new(profile),
            })
        })
    }
    #[napi(factory, catch_unwind)]
    pub fn load() -> Result<Self> {
        Ok(Self {
            inner: Arc::new(radicle::Profile::load().map_err(|e| err("IDENTITY", e))?),
        })
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativeProfile>")]
    pub fn load_async() -> AsyncTask<Work<NativeProfile>> {
        work(Self::load)
    }
    #[napi(catch_unwind, ts_return_type = "Promise<NativeSigner>")]
    pub fn signer_async(&self) -> AsyncTask<Work<NativeSigner>> {
        let inner = self.inner.clone();
        work(move || Self { inner }.signer())
    }
    #[napi(getter, catch_unwind)]
    pub fn did(&self) -> String {
        self.inner.did().to_string()
    }
    #[napi(getter, catch_unwind)]
    pub fn node_id(&self) -> String {
        self.inner.id().to_string()
    }
    #[napi(getter, catch_unwind)]
    pub fn storage(&self) -> NativeStorage {
        NativeStorage {
            inner: self.inner.storage.clone(),
        }
    }
    #[napi(getter, catch_unwind)]
    pub fn home(&self) -> NativeHome {
        NativeHome {
            inner: self.inner.home.clone(),
        }
    }
    #[napi(getter, catch_unwind)]
    pub fn aliases(&self) -> NativeAliases {
        NativeAliases {
            profile: self.inner.clone(),
        }
    }
    #[napi(catch_unwind)]
    pub fn signer(&self) -> Result<NativeSigner> {
        let signer = self.inner.signer().map_err(|e| err("SIGNER", e))?;
        Ok(NativeSigner {
            node_id: signer.public_key().to_string(),
            did: radicle::identity::Did::from(*signer.public_key()).to_string(),
            inner: Arc::new(Mutex::new(signer)),
        })
    }
}
#[napi]
#[derive(Clone)]
pub struct NativeSigner {
    pub(crate) inner: Arc<Mutex<radicle::profile::Signer>>,
    node_id: String,
    did: String,
}
#[napi]
impl NativeSigner {
    #[napi(getter, catch_unwind)]
    pub fn did(&self) -> String {
        self.did.clone()
    }
    #[napi(getter, catch_unwind)]
    pub fn node_id(&self) -> String {
        self.node_id.clone()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;
    #[test]
    fn non_utf8_home_and_storage_paths_are_invalid_data() {
        let root = std::env::temp_dir().join(format!("sdk-path-test-{}", std::process::id()));
        let path = root.join(std::ffi::OsString::from_vec(vec![0xff]));
        let home = NativeHome {
            inner: radicle::profile::Home::new(&path).unwrap(),
        };
        let storage = NativeStorage {
            inner: radicle::storage::git::Storage::open(
                &path,
                radicle::git::UserInfo {
                    alias: "test".parse().unwrap(),
                    key: "z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"
                        .parse()
                        .unwrap(),
                },
            )
            .unwrap(),
        };
        for error in [home.path().unwrap_err(), storage.path().unwrap_err()] {
            assert!(error.reason.starts_with("RADICLE:INVALID_DATA:"));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
