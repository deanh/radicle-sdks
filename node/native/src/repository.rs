//! Detached repository DTOs and binding-only pagination validation.
use super::err;
use napi::{Result, bindgen_prelude::Uint8Array};
use napi_derive::napi;
use std::collections::BTreeMap;

#[napi(object)]
pub struct TreeEntry {
    #[napi(readonly)]
    pub name: Uint8Array,
    #[napi(readonly)]
    pub name_str: String,
    #[napi(readonly, ts_type = "'blob' | 'tree' | 'commit' | 'tag' | 'any'")]
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
