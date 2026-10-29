mod archive;
mod mobile;
mod transaction;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::Path;

use codesh_app_core::backup::Result;
use codesh_app_core::settings::Settings;

pub(crate) use mobile::apply_mobile_settings;
pub(crate) use transaction::{apply_pending, cancel_pending, stage};

pub(super) const ROOTS: &[&str] = &[
    "settings.toml",
    "terminal.toml",
    "server.toml",
    "mobile-settings.json",
    "desktop-state.json",
    "themes",
];
pub(super) const MAX_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const PENDING: &str = "pending-import.codesh";
type Files = BTreeMap<String, Vec<u8>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Restore {
    /// Replaces every configuration file, resetting what the backup leaves out.
    Complete,
    /// Replaces only the files the backup holds.
    Partial,
    /// Replaces only the files the import holds, and adds its projects and
    /// workspaces to the current ones.
    Merge,
}

pub(crate) struct PreparedImport {
    pub(super) files: Files,
    pub(super) restore: Restore,
    pub(crate) summary: String,
}

pub(crate) fn export(
    profile: &Path,
    destination: &Path,
    state: &codesh_app_core::AppState,
) -> Result<()> {
    let mut files = archive::collect(profile, ROOTS)?;
    files.insert(
        "desktop-state.json".into(),
        serde_json::to_vec_pretty(&state.configuration_backup())?,
    );
    files
        .entry("settings.toml".into())
        .or_insert(codesh_app_core::backup::settings_source(&Settings::default())?.into_bytes());
    files.insert(
        mobile::FILE.into(),
        serde_json::to_vec_pretty(&mobile::current(profile)?)?,
    );
    validate(&files)?;
    archive::write(destination, &files, Restore::Complete)
}

pub(crate) fn prepare(profile: &Path, source: &Path) -> Result<PreparedImport> {
    prepare_from(profile, source)
}

fn prepare_from(profile: &Path, source: &Path) -> Result<PreparedImport> {
    let (files, restore) = archive::read(source)?;
    validate_effective(profile, &files, restore)?;
    let projects = files
        .get("desktop-state.json")
        .map(|bytes| codesh_app_core::store::decode(bytes))
        .transpose()?;
    if let Some(state) = &projects {
        let current = codesh_app_core::store::load(profile.join("desktop-state.json"))?;
        state.check_restore_directories(&current)?;
    }
    let count = projects
        .as_ref()
        .map_or(0, |state| state.projects().len().saturating_sub(1));
    let summary = if restore == Restore::Merge {
        format!(
            "Import settings and {count} projects with their layouts on the next launch. Projects you already have keep their details and tabs, and workspaces with the same name are combined. A recovery copy is saved in the profile’s Backups folder."
        )
    } else {
        format!(
            "Restore settings and {count} saved projects with their layouts on the next launch. Existing projects are kept; matching projects receive the saved layouts. A recovery copy is saved in the profile’s Backups folder. Restart the server from Settings to apply restored server settings."
        )
    };

    Ok(PreparedImport {
        files,
        restore,
        summary,
    })
}

fn validate(files: &Files) -> Result<()> {
    if !files.contains_key("settings.toml") {
        return Err("The backup does not contain settings.toml"
            .to_string()
            .into());
    }
    let directory = tempfile::tempdir()?;
    archive::materialize(directory.path(), files)?;
    codesh_app_core::backup::validate_configuration(directory.path())?;
    if let Some(state) = files.get("desktop-state.json") {
        codesh_app_core::store::decode(state)?;
    }
    if let Some(source) = files.get(mobile::FILE) {
        mobile::parse(source)?;
    }

    Ok(())
}

fn validate_effective(profile: &Path, files: &Files, restore: Restore) -> Result<()> {
    if restore == Restore::Complete || files.contains_key("terminal.toml") {
        return validate(files);
    }
    let mut effective = files.clone();
    let native = profile.join("terminal.toml");
    if native.try_exists()? {
        effective.insert("terminal.toml".into(), archive::read_file(&native)?);
    }
    validate(&effective)
}
