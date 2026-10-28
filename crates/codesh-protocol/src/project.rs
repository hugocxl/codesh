use std::fmt;
use std::str::FromStr;

use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::wire::cbor::open_enum;
use uuid::Uuid;

macro_rules! id {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
            Serialize,
            Deserialize,
            Encode,
            Decode,
        )]
        #[serde(transparent)]
        #[cbor(transparent)]
        pub struct $name(
            #[n(0)]
            #[cbor(with = "crate::wire::cbor::bytes")]
            #[serde(with = "uuid::serde::simple")]
            Uuid,
        );

        impl $name {
            pub const fn from_u128(value: u128) -> Self {
                Self(Uuid::from_u128(value))
            }

            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.simple().fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

id!(ProjectId);
id!(OperationId);
id!(ClientId);
id!(ServerIdentity);
id!(DeviceId);

use crate::{ErrorCode, ServerPath, SessionId, SessionInfo};

pub const CATALOG_PAGE_SIZE: usize = 128;
pub const MAX_PROJECTS: usize = 4096;

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ProjectKind {
        Worktree = 0,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ProjectDescriptor {
    #[n(0)]
    pub id: ProjectId,
    #[n(1)]
    pub home: bool,
    #[n(2)]
    pub directory: ServerPath,
    #[n(3)]
    pub name: String,
    #[n(7)]
    pub kind: Option<ProjectKind>,
    #[n(8)]
    pub parent_id: Option<ProjectId>,
}

impl ProjectDescriptor {
    /// A new project, named after its folder.
    pub fn new(directory: ServerPath) -> Self {
        let name = directory
            .0
            .rsplit(|byte| *byte == b'/')
            .find(|part| !part.is_empty())
            .unwrap_or(&directory.0);
        let name = String::from_utf8_lossy(name).into_owned();
        Self::create(false, name, directory)
    }

    /// A server's Home, in the user's home folder.
    pub fn home(directory: ServerPath) -> Self {
        Self::create(true, "Home".into(), directory)
    }

    /// Every app and server makes new projects here.
    fn create(home: bool, name: String, directory: ServerPath) -> Self {
        Self {
            id: ProjectId::new(),
            home,
            directory,
            name,
            kind: None,
            parent_id: None,
        }
    }

    pub fn validate(&self) -> Result<(), ErrorCode> {
        validate_name(&self.name)?;
        if !self.directory.0.starts_with(b"/")
            || self.directory.0.len() > 4096
            || self.directory.0.contains(&0)
            || self.parent_id == Some(self.id)
            || (self.kind == Some(ProjectKind::Worktree) && self.parent_id.is_none())
            || (self.home && (self.kind.is_some() || self.parent_id.is_some()))
        {
            return Err(ErrorCode::BadRequest);
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), ErrorCode> {
    if name.trim().is_empty() || name.len() > 1024 {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum ProjectPatch {
    #[n(0)]
    Name(#[n(0)] String),
}

impl ProjectPatch {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Name(name) => validate_name(name),
        }
    }
    pub fn apply(&self, project: &mut ProjectDescriptor) {
        match self {
            Self::Name(name) => project.name = name.trim().into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum ProjectMutation {
    #[n(0)]
    Create(#[n(0)] ProjectDescriptor),
    #[n(1)]
    Patch {
        #[n(0)]
        project: ProjectId,
        #[n(1)]
        patch: ProjectPatch,
    },
    #[n(2)]
    Delete(#[n(0)] ProjectId),
    #[n(3)]
    PruneWorktree(#[n(0)] ProjectId),
}

impl ProjectMutation {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Create(project)
                if project.home || matches!(project.kind, Some(ProjectKind::Unrecognized(_))) =>
            {
                Err(ErrorCode::BadRequest)
            }
            Self::Create(project) => project.validate(),
            Self::Patch { patch, .. } => patch.validate(),
            Self::Delete(_) | Self::PruneWorktree(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ProjectIntent {
    #[n(0)]
    pub operation: OperationId,
    #[n(1)]
    pub mutation: ProjectMutation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct CatalogPage {
    #[n(0)]
    pub server: ServerIdentity,
    #[n(1)]
    pub home: ProjectId,
    #[n(2)]
    pub revision: u64,
    #[n(3)]
    pub projects: Vec<ProjectDescriptor>,
    #[n(4)]
    pub next: Option<ProjectId>,
    #[n(5)]
    pub legacy_home: Option<ProjectId>,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum SessionStatus {
        Starting = 0,
        Live = 1,
        Ended = 2,
        Unavailable = 3,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ProjectSession {
    #[n(0)]
    pub info: SessionInfo,
    #[n(1)]
    pub status: SessionStatus,
    #[n(2)]
    pub owner: Option<crate::SessionClient>,
    #[n(3)]
    pub attached: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ProjectSessions {
    #[n(0)]
    pub revision: u64,
    #[n(1)]
    pub sessions: Vec<ProjectSession>,
    #[n(2)]
    pub next: Option<SessionId>,
}

impl CatalogPage {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.projects.len() > CATALOG_PAGE_SIZE
            || self
                .projects
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
            || self.next.is_some_and(|next| {
                self.projects
                    .last()
                    .is_none_or(|project| project.id != next)
            })
            || self
                .projects
                .iter()
                .any(|project| project.home != (project.id == self.home))
        {
            return Err(ErrorCode::BadRequest);
        }
        for project in &self.projects {
            project.validate()?;
        }
        Ok(())
    }
}

impl ProjectSessions {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.sessions.len() > CATALOG_PAGE_SIZE
            || self
                .sessions
                .windows(2)
                .any(|pair| pair[0].info.id >= pair[1].info.id)
            || self.next.is_some_and(|next| {
                self.sessions
                    .last()
                    .is_none_or(|session| session.info.id != next)
            })
        {
            return Err(ErrorCode::BadRequest);
        }
        for session in &self.sessions {
            crate::validate_path(&session.info.directory)?;
        }
        Ok(())
    }
}
