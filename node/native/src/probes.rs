//! Native containment/timestamp probes, not SDK public exports.
use crate::timestamps::millis;
use crate::{
    error::err,
    tasks::{Work, work},
};
use napi::bindgen_prelude::AsyncTask;
use napi_derive::napi;
/// T0-only native probe: decimal strings avoid JS precision loss before testing.
/// Not re-exported by the SDK wrapper.
#[napi(catch_unwind, ts_return_type = "Promise<number>")]
pub fn probe_timestamp(value: String, seconds: bool) -> AsyncTask<Work<f64>> {
    work(move || {
        let value = value.parse::<i64>().map_err(|e| {
            let code = match e.kind() {
                std::num::IntErrorKind::PosOverflow | std::num::IntErrorKind::NegOverflow => {
                    "ARG_RANGE"
                }
                _ => "ARG_VALUE",
            };
            err(code, e)
        })?;
        millis(value, seconds)
    })
}

/// T0-only containment probe, not re-exported by the package.
#[napi(catch_unwind, ts_return_type = "Promise<void>")]
pub fn probe_panic() -> AsyncTask<Work<()>> {
    work(|| panic!("T0 injected worker panic"))
}

/// T0-only synchronous FFI containment probe.
#[napi(catch_unwind)]
pub fn probe_sync_panic() {
    panic!("T0 injected synchronous panic");
}
