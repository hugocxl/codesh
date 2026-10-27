use super::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VoiceSettings {
    pub language: String,
    pub model: String,
    pub auto_send: bool,
}

impl VoiceSettings {
    pub fn validate(&self) -> Result<()> {
        if self.model.len() > 4096 || self.model.contains('\0') {
            return Err(Error::new("voice.model", "Invalid model path"));
        }
        if self.language.len() > 200 {
            return Err(Error::new(
                "voice.language",
                "Language names must be at most 200 bytes",
            ));
        }
        Ok(())
    }
}
