//! Binding-only Date-safe stored metadata conversion.
use crate::error::err;
use napi::{
    Result,
    bindgen_prelude::{Either, Null},
};
const DATE_MAX: i64 = 8_640_000_000_000_000;
pub(crate) fn millis(value: i64, seconds: bool) -> Result<f64> {
    let ms = if seconds {
        value.checked_mul(1000)
    } else {
        Some(value)
    }
    .filter(|v| (-DATE_MAX..=DATE_MAX).contains(v))
    .ok_or_else(|| err("INVALID_DATA", "timestamp outside Date range"))?;
    Ok(ms as f64)
}

pub(crate) fn unsigned_millis(value: u64) -> Result<f64> {
    millis(
        i64::try_from(value).map_err(|e| err("INVALID_DATA", e))?,
        false,
    )
}

// A peer controls stored metadata, not our Date/number representation. Do not
// let an unrepresentable date hide the object (or an entire issue listing).
pub(crate) fn metadata_timestamp(value: Result<f64>) -> Either<f64, Null> {
    match value {
        Ok(ms) => Either::A(ms),
        Err(_) => Either::B(Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_timestamp_unrepresentable_values_are_null() {
        assert!(
            matches!(metadata_timestamp(millis(DATE_MAX, false)), Either::A(v) if v == DATE_MAX as f64)
        );
        for value in [
            millis(DATE_MAX + 1, false),
            millis(-DATE_MAX - 1, false),
            millis(i64::MAX, true),
            millis(i64::MIN, true),
            unsigned_millis(u64::MAX),
        ] {
            assert!(matches!(metadata_timestamp(value), Either::B(Null)));
        }
    }

    #[test]
    fn timestamp_bounds() {
        assert_eq!(millis(DATE_MAX, false).unwrap(), DATE_MAX as f64);
        assert_eq!(millis(-DATE_MAX, false).unwrap(), -DATE_MAX as f64);
        assert!(millis(DATE_MAX + 1, false).is_err());
        assert!(millis(i64::MAX, true).is_err());
        assert!(millis(i64::MIN, true).is_err());
        assert!(unsigned_millis(u64::MAX).is_err());
        assert!(unsigned_millis(DATE_MAX as u64 + 1).is_err());
    }
}
