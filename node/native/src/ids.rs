//! Binding argument parsers; preserve original error categories and order.
use crate::error::err;
use napi::Result;
use radicle::{
    cob::{Label, ObjectId},
    identity::{Did, RepoId},
};
pub(crate) fn rid(s: &str) -> Result<RepoId> {
    s.parse().map_err(|e| err("ARG_VALUE", e))
}
pub(crate) fn oid(s: &str) -> Result<ObjectId> {
    s.parse().map_err(|e| err("ARG_VALUE", e))
}
pub(crate) fn parse_labels(values: Vec<String>) -> Result<Vec<Label>> {
    values
        .into_iter()
        .map(|v| Label::new(v).map_err(|e| err("ARG_VALUE", e)))
        .collect()
}
pub(crate) fn parse_dids(values: Vec<String>) -> Result<Vec<Did>> {
    values
        .into_iter()
        .map(|v| v.parse().map_err(|e| err("ARG_VALUE", e)))
        .collect()
}
