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
}

impl Error {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Msg(s.into())
    }
}