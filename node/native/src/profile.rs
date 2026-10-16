//! Profile setup and thread-safe signer handles.
use crate::repository::NativeStorage;
use crate::{
    error::err,
    tasks::{Work, work},
};
use napi::{Result, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use radicle::crypto::Signer as _;
use std::sync::{Arc, Mutex};
#[napi]
#[derive(Clone)]
pub struct NativeProfile {
    inner: Arc<radicle::Profile>,
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
    #[napi(catch_unwind)]
    pub fn signer(&self) -> Result<NativeSigner> {
        let signer = self.inner.signer().map_err(|e| err("SIGNER", e))?;
        Ok(NativeSigner {
            node_id: signer.public_key().to_string(),
            inner: Arc::new(Mutex::new(signer)),
        })
    }
}
#[napi]
#[derive(Clone)]
pub struct NativeSigner {
    pub(crate) inner: Arc<Mutex<radicle::profile::Signer>>,
    node_id: String,
}
#[napi]
impl NativeSigner {
    #[napi(getter, catch_unwind)]
    pub fn node_id(&self) -> String {
        self.node_id.clone()
    }
}
