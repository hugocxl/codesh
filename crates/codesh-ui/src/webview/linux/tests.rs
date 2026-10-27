//! Serializes browser engine tests that share GTK and the clipboard.
use std::sync::{Mutex, MutexGuard, PoisonError};
pub(super) fn alone() -> MutexGuard<'static, ()> {
    static ALONE: Mutex<()> = Mutex::new(());
    ALONE.lock().unwrap_or_else(PoisonError::into_inner)
}
