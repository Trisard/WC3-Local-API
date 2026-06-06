#[derive(Debug, thiserror::Error)]
pub enum Wc3Error {
    #[error("Warcraft III process not found")]
    ProcessNotFound,
    #[error("Cannot open WC3 process for reading")]
    CannotOpenProcess,
    #[error("GUID not found in WC3 process memory")]
    GuidNotFound,
    #[error("WC3 process found but no listening TCP port detected")]
    PortNotFound,
    #[error("Netstat error: {0}")]
    Netstat(String),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tungstenite::Error),
}

pub type Result<T> = std::result::Result<T, Wc3Error>;
