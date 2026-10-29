use super::{Language, Recorder};

pub const CAPABILITY: codesh_core::capability::Capability =
    codesh_core::capability::Capability::Unsupported;
pub const LOCAL_MODEL: bool = false;

pub fn languages() -> Vec<Language> {
    Vec::new()
}

pub(super) fn start(_language: String, _model: String) -> Result<Recorder, String> {
    Err(codesh_core::capability::Feature::Voice
        .unsupported_reason()
        .to_owned())
}
