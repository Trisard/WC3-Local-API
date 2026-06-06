//! Standalone library for reading Warcraft III process state and connecting
//! to its local WebSocket API.
//!
//! # Quick start
//!
//! ```no_run
//! use wc3_local_api::connect_auto;
//!
//! let handle = connect_auto().expect("WC3 must be running");
//! for msg in handle.iter() {
//!     println!("{msg}");
//! }
//! ```

pub mod discovery;
pub mod error;
pub mod socket;

pub use error::{Result, Wc3Error};
pub use socket::{connect, ConnectionConfig, ConnectionHandle, ConnectionStatus};

#[cfg(windows)]
pub use discovery::{get_w3_guid, get_w3_port};

#[cfg(windows)]
pub use socket::{connect_auto, connect_when_ready};
