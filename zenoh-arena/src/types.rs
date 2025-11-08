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

/// Public node state returned by step() method
#[derive(Debug, Clone)]
pub enum NodeState {
    /// Searching for available hosts
    SearchingHost,
    /// Connected as client to a host
    Client {
        /// ID of the host we're connected to
        host_id: NodeId,
    },
    /// Acting as host
    Host {
        /// Whether accepting new clients
        is_accepting: bool,
        /// List of connected client IDs
        connected_clients: Vec<NodeId>,
    },
}

/// Status returned by Node::step() method
#[derive(Debug, Clone)]
pub struct NodeStatus<S> {
    /// Current node state (Searching, Client, or Host)
    pub state: NodeState,
    /// Current game state (if available)
    pub game_state: Option<S>,
}

/// Current state of a Node (internal)
#[derive(Debug)]
pub(crate) enum NodeStateInternal<E> {
    /// Searching for available hosts
    SearchingHost,

    /// Connected as client to a host
    #[allow(dead_code)]
    Client {
        /// ID of the host we're connected to
        host_id: NodeId,
    },

    /// Acting as host
    Host {
        /// Whether accepting new clients
        is_accepting: bool,
        /// List of connected client IDs
        connected_clients: Vec<NodeId>,
        /// Game engine (only present in Host mode)
        #[allow(dead_code)]
        engine: E,
    },
}

impl<E> NodeStateInternal<E> {
    /// Check if currently in host mode
    #[allow(dead_code)]
    pub fn is_host(&self) -> bool {
        matches!(self, NodeStateInternal::Host { .. })
    }

    /// Check if currently in client mode
    #[allow(dead_code)]
    pub fn is_client(&self) -> bool {
        matches!(self, NodeStateInternal::Client { .. })
    }

    /// Check if host mode and accepting clients
    #[allow(dead_code)]
    pub fn is_accepting_clients(&self) -> bool {
        matches!(
            self,
            NodeStateInternal::Host {
                is_accepting: true,
                ..
            }
        )
    }

    /// Get number of connected clients (None if not host)
    #[allow(dead_code)]
    pub fn client_count(&self) -> Option<usize> {
        match self {
            NodeStateInternal::Host {
                connected_clients, ..
            } => Some(connected_clients.len()),
            _ => None,
        }
    }
}

impl<E> From<&NodeStateInternal<E>> for NodeState {
    fn from(internal: &NodeStateInternal<E>) -> Self {
        match internal {
            NodeStateInternal::SearchingHost => NodeState::SearchingHost,
            NodeStateInternal::Client { host_id } => NodeState::Client {
                host_id: host_id.clone(),
            },
            NodeStateInternal::Host {
                is_accepting,
                connected_clients,
                ..
            } => NodeState::Host {
                is_accepting: *is_accepting,
                connected_clients: connected_clients.clone(),
            },
        }
    }
}
