//! Networking for Nux Emulator.
//!
//! Provides TAP and passt network backends for crosvm, bridge detection,
//! backend selection, port forwarding configuration, and DNS setup.

pub mod backend;
pub mod bridge;
pub mod config;
pub mod error;
pub mod passt;
pub mod tap;

pub use backend::{NetworkSetup, ResolvedBackend, select_backend};
pub use config::{NetworkBackend, NetworkVmConfig};
pub use error::{NetworkError, NetworkResult};
