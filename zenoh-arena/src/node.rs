/// Node management module
use std::sync::Arc;

use crate::config::NodeConfig;
use crate::error::{ArenaError, Result};
use crate::types::{NodeId, NodeState};

/// Main Node interface - manages host/client behavior and game sessions
///
/// A Node is autonomous and manages its own role, connections, and game state.
/// There is no central "Arena" - each node has its local view of the network.
pub struct Node<E: GameEngine, F: Fn() -> E> {
    /// Node identifier
    id: NodeId,
    
    /// Node configuration
    #[allow(dead_code)]
    config: NodeConfig,
    
    /// Current node state
    state: NodeState<E>,
    
    /// Zenoh session (wrapped for shared access)
    session: Arc<zenoh::Session>,
    
    /// Engine factory - called when transitioning to host mode
    get_engine: F,
}

impl<E: GameEngine, F: Fn() -> E> Node<E, F> {
    /// Create a new Node instance
    ///
    /// `get_engine` is a factory function that creates an engine when needed
    /// `session` is a Zenoh session that will be owned by the Node
    pub async fn new(config: NodeConfig, session: zenoh::Session, get_engine: F) -> Result<Self> {
        // Create or validate node ID
        let id = match &config.node_name {
            Some(name) => NodeId::from_name(name.clone())?,
            None => NodeId::generate(),
        };
        
        tracing::info!("Node '{}' initialized with Zenoh session", id);
        
        // Initial state depends on force_host configuration
        let state = if config.force_host {
            tracing::info!("Node '{}' forced to host mode", id);
            let engine = get_engine();
            NodeState::Host {
                is_accepting: true,
                connected_clients: Vec::new(),
                engine,
            }
        } else {
            NodeState::SearchingHost
        };
        
        Ok(Self {
            id,
            config,
            state,
            session: Arc::new(session),
            get_engine,
        })
    }
    
    /// Get node ID
    pub fn id(&self) -> &NodeId {
        &self.id
    }
    
    /// Get reference to Zenoh session
    pub fn session(&self) -> &Arc<zenoh::Session> {
        &self.session
    }
    
    /// Run the node state machine
    ///
    /// This is the main event loop that manages state transitions between
    /// SearchingHost -> Client or Host modes
    pub async fn run(&mut self) -> Result<()> {
        // If force_host is enabled, only Host state is allowed
        if self.config.force_host && !matches!(self.state, NodeState::Host { .. }) {
            return Err(ArenaError::Internal(
                "force_host is enabled but node is not in Host state".to_string(),
            ));
        }
        
        // State machine implementation will be expanded in future phases
        match &mut self.state {
            NodeState::SearchingHost => {
                // force_host check ensures we never reach here if force_host is true
                tracing::info!("Node '{}' searching for hosts...", self.id);
                // TODO: Implement host discovery logic
                // For now, do nothing (placeholder for future implementation)
            }
            NodeState::Client { host_id } => {
                // force_host check ensures we never reach here if force_host is true
                tracing::info!("Node '{}' running as client connected to '{}'", self.id, host_id);
                // TODO: Implement client behavior
            }
            NodeState::Host { is_accepting, connected_clients, engine: _ } => {
                tracing::info!(
                    "Node '{}' running as host (accepting: {}, clients: {})",
                    self.id,
                    is_accepting,
                    connected_clients.len()
                );
                // TODO: Implement host behavior
            }
        }
        
        Ok(())
    }
}

/// Trait for game engine integration
///
/// The engine runs only on the host node and processes actions from clients
pub trait GameEngine: Send + Sync {
    /// Action type from user/client
    type Action: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send;
    
    /// State type sent to clients
    type State: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone;
    
    /// Initialize the game engine
    fn initialize(&mut self) -> Result<Self::State>;
    
    /// Process an action and return new state
    fn process_action(
        &mut self,
        action: Self::Action,
        client_id: &NodeId,
    ) -> Result<Self::State>;
    
    /// Get current state
    fn current_state(&self) -> Self::State;
    
    /// Tick/update game state (for time-based games)
    fn tick(&mut self, delta_ms: u64) -> Option<Self::State>;
    
    /// Client connected notification
    fn client_connected(&mut self, client_id: &NodeId);
    
    /// Client disconnected notification
    fn client_disconnected(&mut self, client_id: &NodeId);
    
    /// Check if game session has ended
    fn is_session_ended(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test engine for testing purposes
    #[derive(Debug)]
    struct TestEngine;
    
    impl GameEngine for TestEngine {
        type Action = u32;
        type State = String;
        
        fn initialize(&mut self) -> Result<Self::State> {
            Ok("initialized".to_string())
        }
        
        fn process_action(&mut self, _action: Self::Action, _client_id: &NodeId) -> Result<Self::State> {
            Ok("processed".to_string())
        }
        
        fn current_state(&self) -> Self::State {
            "current".to_string()
        }
        
        fn tick(&mut self, _delta_ms: u64) -> Option<Self::State> {
            None
        }
        
        fn client_connected(&mut self, _client_id: &NodeId) {}
        fn client_disconnected(&mut self, _client_id: &NodeId) {}
        
        fn is_session_ended(&self) -> bool {
            false
        }
    }

    #[test]
    fn test_node_id_generation() {
        let id1 = NodeId::generate();
        let id2 = NodeId::generate();
        
        // Generated IDs should be different
        assert_ne!(id1, id2);
        
        // Should be non-empty
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn test_node_id_from_name() {
        let result = NodeId::from_name("valid_name123".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "valid_name123");
    }

    #[test]
    fn test_node_id_invalid_characters() {
        // Test each invalid character
        assert!(NodeId::from_name("has/slash".to_string()).is_err());
        assert!(NodeId::from_name("has*star".to_string()).is_err());
        assert!(NodeId::from_name("has$dollar".to_string()).is_err());
        assert!(NodeId::from_name("has?question".to_string()).is_err());
        assert!(NodeId::from_name("has#hash".to_string()).is_err());
        assert!(NodeId::from_name("has@at".to_string()).is_err());
    }

    #[test]
    fn test_node_id_empty() {
        let result = NodeId::from_name("".to_string());
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_auto_generated_id() {
        let config = NodeConfig::default();
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;
        
        let node = Node::new(config, session, get_engine).await;
        assert!(node.is_ok());
        
        let node = node.unwrap();
        assert!(!node.id().as_str().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_custom_name() {
        let config = NodeConfig::default()
            .with_node_name("my_custom_node".to_string());
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;
        
        let node = Node::new(config, session, get_engine).await;
        assert!(node.is_ok());
        
        let node = node.unwrap();
        assert_eq!(node.id().as_str(), "my_custom_node");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_invalid_name() {
        let config = NodeConfig::default()
            .with_node_name("invalid/name".to_string());
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;
        
        let result = Node::new(config, session, get_engine).await;
        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                ArenaError::InvalidNodeName(_) => {} // Expected
                other => panic!("Expected InvalidNodeName error, got {:?}", other),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_run_with_force_host() {
        let config = NodeConfig::default()
            .with_force_host(true);
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;
        
        let mut node = Node::new(config, session, get_engine).await.unwrap();
        let result = node.run().await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_force_host_starts_in_host_state() {
        let config = NodeConfig::default()
            .with_force_host(true);
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;
        
        let node = Node::new(config, session, get_engine).await.unwrap();
        // Node should be in Host state when force_host is true
        assert!(matches!(node.state, NodeState::Host { .. }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_default_starts_in_searching_state() {
        let config = NodeConfig::default(); // force_host = false by default
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;
        
        let node = Node::new(config, session, get_engine).await.unwrap();
        // Node should be in SearchingHost state by default
        assert!(matches!(node.state, NodeState::SearchingHost));
    }
}
