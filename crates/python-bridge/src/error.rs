use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("bridge not initialized")]
    NotInitialized,
    #[error("{0}")]
    Other(String),
}

impl BridgeError {
    pub fn other(msg: impl Into<String>) -> Self {
        BridgeError::Other(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, BridgeError>;

macro_rules! from_err {
    ($t:ty) => {
        impl From<$t> for BridgeError {
            fn from(e: $t) -> Self {
                BridgeError::Other(e.to_string())
            }
        }
    };
}

from_err!(blogger_client::Error);
from_err!(blogger_cdp::Error);
from_err!(serde_json::Error);
from_err!(std::io::Error);