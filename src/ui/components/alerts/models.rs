//! Display-only review data. Producers supply account-qualified item identities.
use gettextrs::gettext;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlertKind {
    Conflict,
    Problem,
}

/// Persistent impediments requiring user intervention, not transient retries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProblemCategory {
    Authentication,
    Permission,
    Storage,
    LocalFolder,
}
impl ProblemCategory {
    pub fn title(self) -> String {
        gettext(match self {
            Self::Authentication => "Account needs reconnection",
            Self::Permission => "Access permission missing",
            Self::Storage => "Storage is full",
            Self::LocalFolder => "Local folder unavailable",
        })
    }
    pub fn action(self) -> (&'static str, String) {
        match self {
            Self::Authentication => ("reconnect", gettext("Reconnect account")),
            Self::Permission => ("permissions", gettext("Review permissions")),
            Self::Storage => ("storage", gettext("Free up space")),
            Self::LocalFolder => ("folder", gettext("Select local folder")),
        }
    }
}

/// Presentation metadata; missing measurements must be displayed as unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileVersion {
    pub modified: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AlertDetails {
    Conflict {
        local: FileVersion,
        remote: FileVersion,
    },
    Problem(ProblemCategory),
}

/// No file contents or executable commands are retained by this presentation model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AlertItem {
    pub id: String,
    pub account: String,
    pub title: String,
    pub detail: String,
    pub details: AlertDetails,
}
impl AlertItem {
    pub fn kind(&self) -> AlertKind {
        match self.details {
            AlertDetails::Conflict { .. } => AlertKind::Conflict,
            AlertDetails::Problem(_) => AlertKind::Problem,
        }
    }
}
