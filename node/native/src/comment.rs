//! Detached collaboration comment DTO shared by issue and patch snapshots.
use napi::bindgen_prelude::{Either, Null};
use napi_derive::napi;
#[napi(object)]
pub struct Comment {
    #[napi(readonly)]
    pub id: String,
    #[napi(readonly)]
    pub author: String,
    #[napi(readonly)]
    pub body: String,
    #[napi(readonly)]
    pub timestamp: Either<f64, Null>,
    #[napi(readonly)]
    pub reply_to: Option<String>,
}
