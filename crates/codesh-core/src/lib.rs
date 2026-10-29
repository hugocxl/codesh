//! Narrow primitives that are genuinely shared across Codesh boundaries.
//!
//! Wire-visible types belong in `codesh-protocol`; app and server concepts stay
//! in their respective domain crates. The shared shortcut catalog and the
//! bounded worker primitive are independent of GPUI and server execution.

pub mod bundle;
pub mod capability;
pub mod dirs;
pub mod executable;
pub mod file_lock;
pub mod fs;
pub mod release;
pub mod shortcuts;
pub mod time;
pub mod worker;
