/// Core types for the zenoh-arena library
use std::sync::Arc;
use std::time::Instant;
use zenoh::key_expr::KeyExpr;

use crate::error::{ArenaError, Result};
use crate::network::{HostQueryable, NodeLivelinessToken, NodeLivelinessWatch, NodePublisher, NodeSubscriber};
use crate::network::keyexpr::{LinkType, NodeType};
use crate::node::client_state::ClientState;
use crate::node::host_state::HostState;
use crate::node::node::GameEngine;
use crate::node::name_generator;

/// Unique node identifier
///
/// NodeId must be a valid single-chunk keyexpr:
/// - Non-empty UTF-8 string
/// - Cannot contain: / * $ ? # @
/// - Must be a single chunk (no slashes)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Generate a new unique node ID with human-readable name
    /// Uses Markov chain-based name generation to create pronounceable,
    /// fantasy-style names with a numeric suffix for uniqueness
    pub fn generate() -> Self {
        let name = name_generator::generate_random_name();
        NodeId(name)
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
pub enum NodeState<S = ()> {
    /// Searching for available hosts
    SearchingHost,
    /// Connected as client to a host
    Client {
        /// ID of the host we're connected to
        host_id: NodeId,
        /// Current game state received from host (if available)
        game_state: Option<S>,
    },
    /// Acting as host
    Host {
        /// Whether accepting new clients (derived from queryable presence and capacity)
        is_accepting: bool,
        /// List of connected client IDs
        connected_clients: Vec<NodeId>,
        /// Current game state (if available)
        game_state: Option<S>,
    },
    /// Node has stopped
    Stop,
}

impl<S: std::fmt::Display> std::fmt::Display for NodeState<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeState::SearchingHost => write!(f, "Searching for host..."),
            NodeState::Client { host_id, game_state } => {
                if let Some(state) = game_state {
                    write!(f, "Connected as client to host: {} | State: {}", host_id, state)
                } else {
                    write!(f, "Connected as client to host: {}", host_id)
                }
            }
            NodeState::Host {
                is_accepting,
                connected_clients,
                game_state,
            } => {
                let accepting_str = if *is_accepting { "open" } else { "closed" };
                let client_info = if connected_clients.is_empty() {
                    "no clients".to_string()
                } else {
                    format!("{} client(s)", connected_clients.len())
                };
                
                if let Some(state) = game_state {
                    write!(f, "Host mode ({}, {}, game: {})", accepting_str, client_info, state)
                } else {
                    write!(f, "Host mode ({}, {})", accepting_str, client_info)
                }
            }
            NodeState::Stop => write!(f, "Node stopped"),
        }
    }
}

/// Current state of a Node (internal)
pub(crate) enum NodeStateInternal<E>
where
    E: GameEngine,
{
    /// Searching for available hosts
    SearchingHost,

    /// Connected as client to a host
    Client(ClientState<E>),

    /// Acting as host
    Host(HostState<E>),

    /// Node has stopped
    Stop,
}

impl<E> std::fmt::Debug for NodeStateInternal<E>
where
    E: GameEngine,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStateInternal::SearchingHost => f.debug_tuple("SearchingHost").finish(),
            NodeStateInternal::Client(client_state) => {
                f.debug_struct("Client")
                    .field("host_id", &client_state.host_id)
                    .finish()
            }
            NodeStateInternal::Host(host_state) => f
                .debug_struct("Host")
                .field("connected_clients", &host_state.connected_clients)
                .field("pending_client_disconnects_count", &"<futures>")
                .finish(),
            NodeStateInternal::Stop => f.debug_tuple("Stop").finish(),
        }
    }
}

impl<E> NodeStateInternal<E>
where
    E: GameEngine,
{
    /// Transition to SearchingHost state from any state
    ///
    /// Drops the current state (including engine and liveliness token if in Host mode)
    pub fn searching() -> Self {
        NodeStateInternal::SearchingHost
    }

    /// Create a new Host state
    ///
    /// Creates liveliness token and queryable for host discovery
    pub async fn host(
        engine: E,
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        node_id: &NodeId,
    ) -> Result<Self>
    where
        E: GameEngine,
    {
        let prefix = prefix.into();

        // Create host liveliness token for discovery
        let token =
            NodeLivelinessToken::declare(session, prefix.clone(), NodeType::Host, node_id.clone())
                .await?;

        // Declare queryable for host discovery
        let queryable = HostQueryable::declare(session, prefix.clone(), node_id.clone()).await?;

        // Create liveliness watch for monitoring connected clients
        let client_liveliness_watch = NodeLivelinessWatch::new();

        // Create action subscriber to receive actions from clients
        let action_subscriber = NodeSubscriber::new(session, prefix.clone(), LinkType::Action, node_id).await?;

        // Create state publisher to send game state to all clients (using wildcard for receiver)
        let state_publisher = NodePublisher::new(
            session,
            prefix.clone(),
            LinkType::State,
            node_id,
            None, // Broadcast to all clients
        ).await?;

        Ok(NodeStateInternal::Host(HostState {
            connected_clients: Vec::new(),
            engine,
            _liveliness_token: Some(token),
            queryable: Some(Arc::new(queryable)),
            client_liveliness_watch,
            action_subscriber,
            state_publisher,
            game_state: None,
        }))
    }

    /// Create a new Client state
    ///
    /// Subscribes to liveliness events for the host and declares a client liveliness token
    pub async fn client(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        host_id: NodeId,
        client_id: NodeId,
    ) -> Result<Self>
    where
        E::Action: zenoh_ext::Serialize,
    {
        let prefix = prefix.into();

        // Create and subscribe to liveliness events for the host
        let mut liveliness_watch = NodeLivelinessWatch::new();
        liveliness_watch
            .subscribe(session, prefix.clone(), NodeType::Host, Some(host_id.clone()))
            .await?;

        // Declare client liveliness token (type: Client) so host can track our presence
        let liveliness_token =
            NodeLivelinessToken::declare(session, prefix.clone(), NodeType::Client, client_id.clone()).await?;

        // Create publisher for sending actions to the host
        let action_publisher = NodePublisher::new(
            session,
            prefix.clone(),
            LinkType::Action,
            &client_id,
            Some(&host_id),
        ).await?;

        // Create subscriber for receiving game state from the host
        let state_subscriber = NodeSubscriber::new(
            session,
            prefix,
            LinkType::State,
            &client_id,
        ).await?;

        Ok(NodeStateInternal::Client(ClientState {
            host_id,
            liveliness_watch,
            _liveliness_token: liveliness_token,
            action_publisher,
            state_subscriber,
            game_state: None,
        }))
    }
}

impl<E> From<&NodeStateInternal<E>> for NodeState<E::State>
where
    E: GameEngine,
{
    fn from(internal: &NodeStateInternal<E>) -> Self {
        match internal {
            NodeStateInternal::SearchingHost => NodeState::SearchingHost,
            NodeStateInternal::Client(client_state) => NodeState::Client {
                host_id: client_state.host_id.clone(),
                game_state: client_state.game_state.clone(),
            },
            NodeStateInternal::Host(host_state) => {
                // Use the centralized is_accepting_clients() method
                NodeState::Host {
                    is_accepting: host_state.is_accepting_clients(),
                    connected_clients: host_state.connected_clients.clone(),
                    game_state: host_state.game_state.clone(),
                }
            }
            NodeStateInternal::Stop => NodeState::Stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_state_display_searching() {
        let state: NodeState<String> = NodeState::SearchingHost;
        assert_eq!(format!("{}", state), "Searching for host...");
    }

    #[test]
    fn test_node_state_display_client() {
        let host_id = NodeId::from_name("test_host".to_string()).unwrap();
        let state: NodeState<String> = NodeState::Client { 
            host_id,
            game_state: None,
        };
        assert_eq!(
            format!("{}", state),
            "Connected as client to host: test_host"
        );
    }

    #[test]
    fn test_node_state_display_host_empty() {
        let state: NodeState<String> = NodeState::Host {
            is_accepting: true,
            connected_clients: vec![],
            game_state: None,
        };
        assert_eq!(format!("{}", state), "Host mode (open, no clients)");
    }

    #[test]
    fn test_node_state_display_host_with_clients() {
        let client1 = NodeId::from_name("client1".to_string()).unwrap();
        let client2 = NodeId::from_name("client2".to_string()).unwrap();
        let state: NodeState<String> = NodeState::Host {
            is_accepting: true,
            connected_clients: vec![client1, client2],
            game_state: None,
        };
        assert_eq!(format!("{}", state), "Host mode (open, 2 client(s))");
    }

    #[test]
    fn test_node_state_display_host_with_game_state() {
        #[derive(Debug)]
        struct TestGameState {
            score: u32,
        }
        
        impl std::fmt::Display for TestGameState {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Score: {}", self.score)
            }
        }
        
        let state: NodeState<TestGameState> = NodeState::Host {
            is_accepting: true,
            connected_clients: vec![],
            game_state: Some(TestGameState { score: 42 }),
        };
        assert_eq!(format!("{}", state), "Host mode (open, no clients, game: Score: 42)");
    }
}
