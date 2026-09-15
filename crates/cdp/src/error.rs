use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("transport io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("websocket error: {0}")]
    Ws(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cdp call {method} failed: {message}")]
    Cdp { method: String, message: String },
    #[error("javascript evaluation error: {0}")]
    Js(String),
    #[error("timeout after {0}ms")]
    Timeout(u64),
    #[error("chrome is not reachable at port {0}, start it with --remote-debugging-port={0}")]
    NotConnected(u16),
    #[error("no matching cdP target")]
    NoTarget,
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn cdp<T: Into<String>>(method: &str, message: T) -> Self {
        Error::Cdp {
            method: method.to_string(),
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn is_timeout(e: &Error) -> bool {
    matches!(e, Error::Timeout(_))
}