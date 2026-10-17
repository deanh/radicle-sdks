//! Binding argument parsers; preserve original error categories and order.
use crate::error::err;
use napi::Result;
use napi_derive::napi;
use radicle::{
    cob::{Label, ObjectId},
    identity::{Did, RepoId},
};
#[napi(catch_unwind)]
pub fn parse_repo_id(value: String) -> Result<String> {
    Ok(rid(&value)?.to_string())
}
#[napi(catch_unwind)]
pub fn parse_did(value: String) -> Result<String> {
    value
        .parse::<Did>()
        .map(|id| id.to_string())
        .map_err(|e| err("ARG_VALUE", e))
}
#[napi(catch_unwind)]
pub fn parse_node_id(value: String) -> Result<String> {
    value
        .parse::<radicle::node::NodeId>()
        .map(|id| id.to_string())
        .map_err(|e| err("ARG_VALUE", e))
}
#[napi(catch_unwind)]
pub fn parse_oid(value: String) -> Result<String> {
    value
        .parse::<radicle::git::Oid>()
        .map(|id| id.to_string())
        .map_err(|e| err("ARG_VALUE", e))
}
#[napi(catch_unwind)]
pub fn parse_object_id(value: String) -> Result<String> {
    Ok(oid(&value)?.to_string())
}

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
