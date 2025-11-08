/// Node management module
use std::sync::Arc;

use crate::config::NodeConfig;
use crate::error::{ArenaError, Result};
use crate::types::{NodeId, NodeState, NodeStateInternal, NodeStatus};

/// Commands that can be sent to the node
#[derive(Debug, Clone)]
pub enum NodeCommand<A> {
    /// Process a game engine action
    GameAction(A),
    /// Stop the node's run loop
    Stop,
}

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
    state: NodeStateInternal<E>,

    /// Zenoh session (wrapped for shared access)
    session: Arc<zenoh::Session>,

    /// Engine factory - called when transitioning to host mode
    get_engine: F,

    /// Receiver for commands from the application
    command_rx: flume::Receiver<NodeCommand<E::Action>>,
}

impl<E: GameEngine, F: Fn() -> E> Node<E, F> {
    /// Create a new Node instance
    ///
    /// Returns a tuple of (Node, command_sender) where command_sender is used by the
    /// application to send commands (game actions or control commands) to the node.
    ///
    /// `get_engine` is a factory function that creates an engine when needed
    /// `session` is a Zenoh session that will be owned by the Node
    pub async fn new(
        config: NodeConfig,
        session: zenoh::Session,
        get_engine: F,
    ) -> Result<(Self, flume::Sender<NodeCommand<E::Action>>)> {
        // Create or validate node ID
        let id = match &config.node_name {
            Some(name) => NodeId::from_name(name.clone())?,
            None => NodeId::generate(),
        };

        tracing::info!("Node '{}' initialized with Zenoh session", id);

        // Create command channel
        let (command_tx, command_rx) = flume::unbounded();

        // Initial state depends on force_host configuration
        let state = if config.force_host {
            tracing::info!("Node '{}' forced to host mode", id);
            let engine = get_engine();
            NodeStateInternal::Host {
                is_accepting: true,
                connected_clients: Vec::new(),
                engine,
            }
        } else {
            NodeStateInternal::SearchingHost
        };

        let node = Self {
            id,
            config,
            state,
            session: Arc::new(session),
            get_engine,
            command_rx,
        };

        Ok((node, command_tx))
    }

    /// Get node ID
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    /// Get reference to Zenoh session
    pub fn session(&self) -> &Arc<zenoh::Session> {
        &self.session
    }

    /// Execute one step of the node state machine
    ///
    /// Processes commands from the command channel and returns the current node status.
    /// Returns when either:
    /// - A new game state is produced by the engine
    /// - The step timeout (configured in NodeConfig) elapses
    /// - A Stop command is received (returns None)
    ///
    /// Returns None if Stop command was received, indicating the node should shut down.
    pub async fn step(&mut self) -> Result<Option<NodeStatus<E::State>>> {
        // If force_host is enabled, only Host state is allowed
        if self.config.force_host && !matches!(self.state, NodeStateInternal::Host { .. }) {
            return Err(ArenaError::Internal(
                "force_host is enabled but node is not in Host state".to_string(),
            ));
        }

        let timeout = tokio::time::Duration::from_millis(self.config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout or new state
        loop {
            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    // Build the node state info using From trait
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&self.state),
                        game_state: None,
                    }));
                }
                // Command received
                result = self.command_rx.recv_async() => match result {
                    Err(_) => {
                        // Channel disconnected
                        tracing::info!("Node '{}' command channel closed", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command, exiting", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::GameAction(action)) => {
                        // Process action based on current state
                        match &mut self.state {
                            NodeStateInternal::SearchingHost => {
                                tracing::warn!(
                                    "Node '{}' received action while searching for host, ignoring",
                                    self.id
                                );
                                // Actions are ignored while searching for a host
                            }
                            NodeStateInternal::Client { host_id } => {
                                tracing::debug!(
                                    "Node '{}' forwarding action to host '{}'",
                                    self.id,
                                    host_id
                                );
                                // TODO: Forward action to remote host via Zenoh pub/sub
                                // Placeholder for Phase 4 implementation
                            }
                            NodeStateInternal::Host { engine, .. } => {
                                tracing::debug!(
                                    "Node '{}' processing action in host mode",
                                    self.id
                                );
                                // Process action directly in the engine and get new state
                                let new_game_state = engine.process_action(action, &self.id)?;
                                // Build the node state info using From trait
                                return Ok(Some(NodeStatus {
                                    state: NodeState::from(&self.state),
                                    game_state: Some(new_game_state),
                                }));
                            }
                        }
                    }
                }
            }
        }
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
    fn process_action(&mut self, action: Self::Action, client_id: &NodeId) -> Result<Self::State>;

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

        fn process_action(
            &mut self,
            _action: Self::Action,
            _client_id: &NodeId,
        ) -> Result<Self::State> {
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

        let result = Node::new(config, session, get_engine).await;
        assert!(result.is_ok());

        let (node, _command_tx) = result.unwrap();
        assert!(!node.id().as_str().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_custom_name() {
        let config = NodeConfig::default().with_node_name("my_custom_node".to_string());
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let result = Node::new(config, session, get_engine).await;
        assert!(result.is_ok());

        let (node, _command_tx) = result.unwrap();
        assert_eq!(node.id().as_str(), "my_custom_node");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_invalid_name() {
        let config = NodeConfig::default().with_node_name("invalid/name".to_string());
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
    async fn test_node_step_with_force_host() {
        let config = NodeConfig::default().with_force_host(true);
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let (mut node, command_tx) = Node::new(config, session, get_engine).await.unwrap();

        // Spawn step loop in background
        let step_handle = tokio::spawn(async move {
            loop {
                match node.step().await {
                    Ok(Some(_status)) => {
                        // Continue stepping
                    }
                    Ok(None) => {
                        // Stop command received
                        break Ok(());
                    }
                    Err(e) => break Err(e),
                }
            }
        });

        // Send Stop command to exit the loop
        command_tx.send(NodeCommand::Stop).unwrap();

        let result: Result<()> = step_handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_force_host_starts_in_host_state() {
        let config = NodeConfig::default().with_force_host(true);
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let (node, _command_tx) = Node::new(config, session, get_engine).await.unwrap();
        // Node should be in Host state when force_host is true
        assert!(matches!(node.state, NodeStateInternal::Host { .. }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_default_starts_in_searching_state() {
        let config = NodeConfig::default(); // force_host = false by default
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let (node, _command_tx) = Node::new(config, session, get_engine).await.unwrap();
        // Node should be in SearchingHost state by default
        assert!(matches!(node.state, NodeStateInternal::SearchingHost));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_processes_actions_in_host_mode() {
        let config = NodeConfig::default()
            .with_force_host(true)
            .with_step_timeout_ms(50);
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let (mut node, command_tx) = Node::new(config, session, get_engine).await.unwrap();

        // Send some game actions
        command_tx.send(NodeCommand::GameAction(42)).unwrap();
        command_tx.send(NodeCommand::GameAction(100)).unwrap();

        // Call step to process first action
        let status1 = node.step().await.unwrap();
        assert!(status1.is_some());
        let status1 = status1.unwrap();
        assert!(status1.game_state.is_some());
        assert_eq!(status1.game_state.unwrap(), "processed");

        // Call step to process second action
        let status2 = node.step().await.unwrap();
        assert!(status2.is_some());
        let status2 = status2.unwrap();
        assert!(status2.game_state.is_some());
        assert_eq!(status2.game_state.unwrap(), "processed");
    }
}
