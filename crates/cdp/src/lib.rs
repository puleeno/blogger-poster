mod client;
mod error;
mod types;

pub use client::{CdpConnection, CdpController};
pub use error::{Error, Result, is_timeout};
pub use types::{BrowserInfo, CdpEvent, Target};