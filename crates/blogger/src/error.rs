use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("cache error: {0}")]
    Cache(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("oauth error: {0}")]
    Oauth(String),
    #[error("blogger api error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn oauth(msg: impl Into<String>) -> Self {
        Error::Oauth(msg.into())
    }
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// True if the error indicates a transient HTTP/network failure that deserves a retry.
    pub fn is_transient(&self) -> bool {
        matches!(self, Error::Http(e) if e.is_timeout() || e.is_connect())
    }
}