use std::path::Path;

use codesh_app_core::backup::Result;
use codesh_protocol::RemoteAccessSettings;
use serde_json::{Map, Value};

use super::archive;

pub(super) const FILE: &str = "mobile-settings.json";

pub(super) fn parse(source: &[u8]) -> Result<RemoteAccessSettings> {
    let value: Map<String, Value> = serde_json::from_slice(source)?;
    if value
        .keys()
        .any(|key| !matches!(key.as_str(), "enabled" | "port"))
    {
        return Err("Mobile settings may contain only enabled and port"
            .to_string()
            .into());
    }
    let settings: RemoteAccessSettings = serde_json::from_value(Value::Object(value))?;
    settings
        .validate()
        .map_err(|_| "Invalid Mobile port".to_string())?;
    Ok(settings)
}

pub(super) fn current(profile: &Path) -> Result<RemoteAccessSettings> {
    if profile.join(FILE).try_exists()? {
        return parse(&archive::read_file(&profile.join(FILE))?);
    }
    let path = profile.join("remote.json");
    if !path.try_exists()? {
        return Ok(RemoteAccessSettings::default());
    }
    let value: Value = serde_json::from_slice(&archive::read_file(&path)?)?;
    parse(&serde_json::to_vec(&serde_json::json!({
        "enabled": value["enabled"], "port": value["port"]
    }))?)
}

pub(crate) fn apply_mobile_settings(
    profile: &Path,
    apply: impl FnOnce(RemoteAccessSettings) -> Result<()>,
) -> Result<()> {
    let path = profile.join(FILE);
    if path.try_exists()? {
        apply(parse(&archive::read_file(&path)?)?)?;
        std::fs::remove_file(path)?;
    }
    Ok(())
}
