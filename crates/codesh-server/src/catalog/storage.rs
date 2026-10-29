use std::io::{self, Write};
use std::path::Path;

use super::State;

pub(super) fn save(path: &Path, state: &State, committed: &mut bool) -> io::Result<()> {
    let bytes = serde_json::to_vec(state)?;
    codesh_core::fs::replace(path, |file| file.write_all(&bytes))?;
    *committed = true;
    codesh_core::fs::sync_parent(path)
}
