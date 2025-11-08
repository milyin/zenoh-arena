/// Core types for the zenoh-arena library
use std::time::Instant;

use crate::error::{ArenaError, Result};

/// Unique node identifier
///
/// NodeId must be a valid single-chunk keyexpr:
/// - Non-empty UTF-8 string
/// - Cannot contain: / * $ ? # @
/// - Must be a single chunk (no slashes)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Generate a new unique node ID (guaranteed to be keyexpr-safe)
    /// Uses base58 encoding of UUID to avoid special characters
    pub fn generate() -> Self {
        let uuid = uuid::Uuid::new_v4();
        let encoded = bs58::encode(uuid.as_bytes()).into_string();
        // Take first 16 characters for reasonable length
        let shortened = encoded.chars().take(16).collect::<String>();
        NodeId(shortened)
    }

    /// Create from a specific name (must be unique and keyexpr-compatible)
    /// Returns error if name contains invalid characters
    pub fn from_name(name: String) -> Result<Self> {
        Self::validate(&name)?;
        Ok(NodeId(name))
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate that a string can be used as NodeId (single keyexpr chunk)
    fn validate(s: &str) -> Result<()> {
        if s.is_empty() {
            return Err(ArenaError::InvalidNodeName(
                "Node name cannot be empty".to_string(),
            ));
        }

        // Check for invalid characters: / * $ ? # @
        for ch in s.chars() {
            if matches!(ch, '/' | '*' | '$' | '?' | '#' | '@') {
                return Err(ArenaError::InvalidNodeName(format!(
                    "Node name '{}' contains invalid character '{}'",
                    s, ch
                )));
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Node role in the arena
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Client role - connects to hosts
    Client,
    /// Host role - accepts clients and runs game engine
    Host,
}

/// Node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node identifier
    pub id: NodeId,
    /// Node role
    pub role: NodeRole,
    /// Time when node was created or connected
    pub connected_since: Instant,
}

/// Current state of the Arena
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaState {
    /// Initializing
    Initializing,

    /// Searching for available hosts
    SearchingHost,

    /// Connected as client to a host
    ConnectedClient {
        /// ID of the host we're connected to
        host_id: NodeId,
    },

    /// Acting as host
    Host {
        /// Whether accepting new clients
        is_open: bool,
        /// List of connected client IDs
        connected_clients: Vec<NodeId>,
    },

    /// Transitioning between states
    Transitioning,

    /// Stopped/Closed
    Stopped,
}

impl ArenaState {
    /// Check if currently in host mode
    pub fn is_host(&self) -> bool {
        matches!(self, ArenaState::Host { .. })
    }

    /// Check if currently in client mode
    pub fn is_client(&self) -> bool {
        matches!(self, ArenaState::ConnectedClient { .. })
    }

    /// Check if host mode with no clients
    pub fn is_empty_host(&self) -> bool {
        matches!(
            self,
            ArenaState::Host {
                connected_clients,
                ..
            } if connected_clients.is_empty()
        )
    }

    /// Check if host mode and accepting clients
    pub fn is_open_host(&self) -> bool {
        matches!(
            self,
            ArenaState::Host {
                is_open: true,
                ..
            }
        )
    }
}

/// State update notification
#[derive(Debug, Clone)]
pub struct StateUpdate<T> {
    /// The game state
    pub state: T,
    /// Source node that produced this state
    pub source: NodeId,
    /// Timestamp of the update
    pub timestamp: std::time::SystemTime,
}
