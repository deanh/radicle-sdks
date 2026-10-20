//! Detached repository DTOs and binding-only pagination validation.
use crate::error::err;
use napi::{
    Result,
    bindgen_prelude::{Either, Null, Uint8Array},
};
use napi_derive::napi;
use std::collections::BTreeMap;
#[napi(object)]
pub struct HeadInfo {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub oid: String,
}
#[napi(object)]
pub struct ProjectDoc {
    #[napi(readonly)]
    pub version: u32,
    #[napi(readonly)]
    pub threshold: f64,
    #[napi(readonly)]
    pub commit: String,
    #[napi(readonly)]
    pub blob: String,
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub description: String,
    #[napi(readonly)]
    pub default_branch: String,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub delegates: Vec<String>,
    #[napi(readonly, ts_type = "'public' | 'private'")]
    pub visibility: String,
}
#[napi(object)]
pub struct Commit {
    #[napi(readonly)]
    pub oid: String,
    #[napi(readonly)]
    pub message: Uint8Array,
    #[napi(readonly)]
    pub message_str: String,
    #[napi(readonly)]
    pub author_name: Uint8Array,
    #[napi(readonly)]
    pub author_name_str: String,
    #[napi(readonly)]
    pub author_email: Uint8Array,
    #[napi(readonly)]
    pub author_email_str: String,
    #[napi(readonly)]
    pub author_time: Either<f64, Null>,
    #[napi(readonly)]
    pub author_offset_min: i32,
    #[napi(readonly)]
    pub committer_name: Uint8Array,
    #[napi(readonly)]
    pub committer_name_str: String,
    #[napi(readonly)]
    pub committer_email: Uint8Array,
    #[napi(readonly)]
    pub committer_email_str: String,
    #[napi(readonly)]
    pub committer_time: Either<f64, Null>,
    #[napi(readonly)]
    pub committer_offset_min: i32,
    #[napi(readonly)]
    pub tree: String,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub parents: Vec<String>,
    #[napi(readonly)]
    pub encoding: Option<String>,
}
#[napi(object)]
pub struct TreeEntry {
    #[napi(readonly)]
    pub name: Uint8Array,
    #[napi(readonly)]
    pub name_str: String,
    #[napi(readonly, ts_type = "'blob' | 'tree' | 'commit'")]
    pub kind: Option<String>,
    #[napi(readonly)]
    pub mode: u32,
    #[napi(readonly)]
    pub oid: String,
}
#[napi(object)]
pub struct Tree {
    #[napi(readonly)]
    pub oid: String,
    #[napi(readonly, ts_type = "ReadonlyArray<TreeEntry>")]
    pub entries: Vec<TreeEntry>,
}
#[napi(object)]
pub struct HistoryOptions {
    pub limit: Option<f64>,
    pub offset: Option<f64>,
}
#[napi(object)]
pub struct HistoryPage {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub head: String,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub oids: Vec<String>,
    #[napi(readonly)]
    pub offset: f64,
    #[napi(readonly)]
    pub has_more: bool,
}
#[napi(object)]
pub struct SignedRefs {
    #[napi(readonly)]
    pub node_id: String,
    #[napi(readonly)]
    pub at: String,
    #[napi(readonly)]
    pub parent: Option<String>,
    #[napi(readonly, ts_type = "'none' | 'root' | 'parent'")]
    pub feature_level: String,
    #[napi(readonly, ts_type = "Readonly<Record<string, string>>")]
    pub refs: BTreeMap<String, String>,
}

impl From<radicle::storage::refs::SignedRefs> for SignedRefs {
    fn from(signed: radicle::storage::refs::SignedRefs) -> Self {
        Self {
            node_id: signed.id().to_string(),
            at: signed.at.to_string(),
            parent: signed.parent().map(|id| id.to_string()),
            feature_level: signed.feature_level().to_string(),
            refs: signed
                .refs()
                .iter()
                .map(|(name, id)| (name.to_string(), id.to_string()))
                .collect(),
        }
    }
}

#[napi(object)]
pub struct TagInfo {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub target: String,
    #[napi(readonly, ts_type = "'annotated' | 'lightweight'")]
    pub kind: String,
    #[napi(readonly)]
    pub peeled: String,
    #[napi(readonly, ts_type = "'commit' | 'tree' | 'blob'")]
    pub peeled_kind: String,
}
#[napi(object)]
pub struct NamespaceTip {
    #[napi(readonly)]
    pub node_id: String,
    #[napi(readonly)]
    pub target: String,
}
#[napi(object)]
pub struct RemoteInfo {
    #[napi(readonly)]
    pub fetcher: String,
    #[napi(readonly, ts_type = "ReadonlyArray<string>")]
    pub heads: Vec<String>,
}
#[napi(object)]
pub struct SetHead {
    #[napi(readonly)]
    pub old: Option<String>,
    #[napi(readonly)]
    pub new: String,
    #[napi(readonly)]
    pub is_updated: bool,
}

pub(super) fn page_number(value: f64, name: &str, minimum: f64) -> Result<usize> {
    if !value.is_finite()
        || value.fract() != 0.0
        || !(minimum..=9_007_199_254_740_991.0).contains(&value)
    {
        return Err(err(
            "ARG_VALUE",
            format!("{name} must be a safe integer >= {minimum}"),
        ));
    }
    usize::try_from(value as u64).map_err(|e| err("ARG_VALUE", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pagination_numbers_are_checked_before_integer_conversion() {
        assert_eq!(page_number(0.0, "offset", 0.0).unwrap(), 0);
        assert_eq!(page_number(1001.0, "limit", 1.0).unwrap(), 1001);
        assert!(page_number(0.0, "limit", 1.0).is_err());
        for value in [
            -1.0,
            0.5,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            9_007_199_254_740_992.0,
        ] {
            assert!(page_number(value, "offset", 0.0).is_err());
        }
        assert_eq!(
            page_number(9_007_199_254_740_991.0, "offset", 0.0).is_ok(),
            usize::BITS >= 64
        );
    }
}
