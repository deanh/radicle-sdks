//! Binding error envelopes; JavaScript maps stable codes to SDK errors.
use napi::{Error, Status};
pub(crate) fn err(code: &str, e: impl std::fmt::Display) -> Error {
    Error::new(Status::GenericFailure, format!("RADICLE:{code}:{e}"))
}
