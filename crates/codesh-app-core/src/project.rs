use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::{AppError, ProjectId, ServerId, Tab};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color(String);

impl Color {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Color {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() == 7
            && value.starts_with('#')
            && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            Ok(Self(value.to_ascii_lowercase()))
        } else {
            Err(AppError::InvalidState(
                "color must be a #RRGGBB hex string".into(),
            ))
        }
    }
}

impl TryFrom<String> for Color {
    type Error = AppError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Color> for String {
    fn from(color: Color) -> Self {
        color.0
    }
}

impl Default for Color {
    fn default() -> Self {
        Self("#808080".into())
    }
}

impl fmt::Display for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The first character of a name, which stands for it on a project's tile.
pub fn initial(name: &str) -> &str {
    name.graphemes(true).next().unwrap_or("?")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    #[serde(default)]
    pub home: bool,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<std::time::SystemTime>,
    pub server_id: ServerId,
    #[serde(with = "crate::catalog::directory")]
    pub directory: PathBuf,
    pub kind: Option<ProjectKind>,
    pub parent_id: Option<ProjectId>,
    pub tabs: Vec<Tab>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) groups: Option<crate::TabGroups>,
    #[serde(skip)]
    pub(crate) status: ProjectStatus,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProjectStatus {
    #[default]
    Available,
    Missing,
}

/// The tab color palette, by name. Names are English keys, translated where
/// shown.
pub const TAB_COLORS: [(&str, &str); 12] = [
    ("Red", "#e5484d"),
    ("Orange", "#f76b15"),
    ("Amber", "#f5a623"),
    ("Yellow", "#ebcb00"),
    ("Lime", "#9bcd1e"),
    ("Green", "#30a46c"),
    ("Teal", "#12a594"),
    ("Cyan", "#05a2c2"),
    ("Blue", "#3e63dd"),
    ("Indigo", "#5b5bd6"),
    ("Violet", "#8e4ec6"),
    ("Pink", "#d6409f"),
];

impl Project {
    pub fn status(&self) -> ProjectStatus {
        self.status
    }

    /// Another computer's Home, which stands for its server.
    pub fn is_remote_home(&self) -> bool {
        self.home && !self.server_id.is_local()
    }

    /// Only this computer's folders can be checked here. Another computer's
    /// projects keep what their server last said.
    pub(crate) fn refresh_status(&mut self) {
        if !self.server_id.is_local() {
            return;
        }
        self.status = if self.directory.is_dir()
            && (self.kind != Some(ProjectKind::Worktree) || self.directory.join(".git").is_file())
        {
            ProjectStatus::Available
        } else {
            ProjectStatus::Missing
        };
    }

    pub(crate) fn require_available(&self) -> Result<(), AppError> {
        if self.status == ProjectStatus::Missing {
            Err(AppError::InvalidState("project folder is missing".into()))
        } else {
            Ok(())
        }
    }
}

pub use codesh_protocol::ProjectKind;
