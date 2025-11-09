/// Error types for the zenoh-arena library
use thiserror::Error;

/// Result type alias for arena operations
pub type Result<T> = std::result::Result<T, ArenaError>;

/// Errors that can occur in zenoh-arena operations
#[derive(Debug, Error)]
pub enum ArenaError {
    /// Zenoh-related errors
    #[error("Zenoh error: {0}")]
    Zenoh(#[from] zenoh::Error),

    /// Node name conflict detected
    #[error("Node name conflict: {0}")]
    NodeNameConflict(String),

    /// Invalid node name provided
    #[error("Invalid node name: {0}. Must be a valid single-chunk keyexpr (no /, *, $, ?, #, @)")]
    InvalidNodeName(String),

    /// Invalid keyexpr pattern
    #[error("Invalid keyexpr: {0}")]
    InvalidKeyexpr(String),

    /// Invalid state transition attempted
    #[error("Invalid state transition: from {from:?} to {to:?}")]
    InvalidStateTransition {
        /// Current state
        from: String,
        /// Attempted target state
        to: String,
    },

    /// Host not found during discovery
    #[error("Host not found")]
    HostNotFound,

    /// Connection rejected by host
    #[error("Connection rejected: {0}")]
    ConnectionRejected(String),

    /// Operation requires host mode
    #[error("Not in host mode")]
    NotHost,

    /// Operation requires client mode
    #[error("Not in client mode")]
    NotClient,

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Game engine error
    #[error("Engine error: {0}")]
    Engine(String),

    /// Operation timeout
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Liveliness token conflict - another token with same keyexpr already exists
    #[error("Liveliness token conflict: {0}")]
    LivelinessTokenConflict(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
