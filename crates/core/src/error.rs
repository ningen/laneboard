use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
    #[error("card not found: {0}")]
    NotFound(String),
    #[error("invalid status: {0}")]
    InvalidStatus(String),
    #[error("invalid link type: {0}")]
    InvalidLinkType(String),
    #[error("{0}")]
    AlreadyClaimed(String),
    #[error("{0}")]
    NotClaimable(String),
    #[error("{0}")]
    BlockedByDeps(String),
    #[error("{0}")]
    Validation(String),
}

impl Error {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Msg(s.into())
    }

    /// Machine-readable code for MCP / JSON agents.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "card_not_found",
            Self::AlreadyClaimed(_) => "already_claimed",
            Self::NotClaimable(_) => "not_claimable",
            Self::BlockedByDeps(_) => "blocked_by_deps",
            Self::InvalidStatus(_) => "invalid_status",
            Self::InvalidLinkType(_) => "invalid_link_type",
            Self::Validation(_) => "validation",
            Self::Db(_) | Self::Json(_) | Self::Msg(_) => "internal",
        }
    }
}