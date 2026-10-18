//! Worker dispatch and panic containment; no retries or poison recovery.
use crate::error::err;
use napi::{
    Env, Result, Task,
    bindgen_prelude::{AsyncTask, ToNapiValue, TypeName},
};
/// Convert a store cardinality without silently rounding an upstream count.
pub(crate) fn count_number(value: usize) -> Result<f64> {
    if value as u128 > 9_007_199_254_740_991 {
        return Err(err(
            "INVALID_DATA",
            "store count exceeds the JavaScript safe integer range",
        ));
    }
    Ok(value as f64)
}

type Job<T> = Box<dyn FnOnce() -> Result<T> + Send>;
pub struct Work<T>(Option<Job<T>>);
impl<T: Send + ToNapiValue + TypeName + 'static> Task for Work<T> {
    type Output = T;
    type JsValue = T;
    fn compute(&mut self) -> Result<T> {
        let job = self
            .0
            .take()
            .ok_or_else(|| err("INTERNAL", "task already consumed"))?;
        // A job is consumed even on panic. Do not resume a possibly modified
        // transaction or clear poisoned locks; errors do not imply rollback.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
            Ok(result) => result,
            Err(payload) => {
                // Do not expose a potentially sensitive panic payload to JS.
                // A custom payload can itself panic on Drop; contain that too.
                if let Err(nested) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)))
                {
                    std::mem::forget(nested);
                }
                Err(err(
                    "INTERNAL",
                    "Rust task panicked; operation may have partial side effects",
                ))
            }
        }
    }
    fn resolve(&mut self, _: Env, value: T) -> Result<T> {
        Ok(value)
    }
}
pub(crate) fn work<T: Send + ToNapiValue + TypeName + 'static>(
    f: impl FnOnce() -> Result<T> + Send + 'static,
) -> AsyncTask<Work<T>> {
    AsyncTask::new(Work(Some(Box::new(f))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[test]
    fn counts_preserve_safe_integers() {
        assert_eq!(count_number(0).unwrap(), 0.0);
        if let Ok(boundary) = usize::try_from(9_007_199_254_740_991u64) {
            assert_eq!(count_number(boundary).unwrap(), 9_007_199_254_740_991.0);
            assert!(count_number(boundary + 1).is_err());
        }
    }
    #[test]
    fn task_panic_fails_closed_without_unwinding_or_clearing_poison() {
        let lock = Arc::new(Mutex::new(()));
        let held = lock.clone();
        let mut task = Work::<()>(Some(Box::new(move || {
            let _guard = held.lock().unwrap();
            panic!("injected while locked");
        })));
        assert!(
            task.compute()
                .unwrap_err()
                .reason
                .starts_with("RADICLE:INTERNAL:")
        );
        assert!(lock.is_poisoned());
        assert!(task.compute().is_err()); // consumed; never rerun
        let mut next = Work(Some(Box::new(|| Ok(42))));
        assert_eq!(next.compute().unwrap(), 42);
    }
}
