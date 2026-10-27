use std::io;
use std::path::Path;

pub use codesh_core::bundle::{BundleLease, lock_replacement, lock_unused};

pub fn acquire_runtime(executable: &Path) -> io::Result<Option<BundleLease>> {
    codesh_core::bundle::acquire_runtime(
        executable,
        &serde_json::to_vec(&codesh_protocol::BuildInfo::current())?,
    )
}
