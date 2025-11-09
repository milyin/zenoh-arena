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
    #[allow(dead_code)]
    get_engine: F,

    /// Receiver for commands from the application
    command_rx: flume::Receiver<NodeCommand<E::Action>>,

    /// Sender for commands from the application
    command_tx: flume::Sender<NodeCommand<E::Action>>,
}

impl<E: GameEngine, F: Fn() -> E> Node<E, F> {
    /// Create a new Node instance (internal use only - use builder pattern via SessionExt)
    pub(crate) async fn new_internal(
        config: NodeConfig,
        session: zenoh::Session,
        get_engine: F,
    ) -> Result<Self> {
        let id = config.node_id.clone();

        tracing::info!("Node '{}' initialized with Zenoh session", id);

        // Create command channel
        let (command_tx, command_rx) = flume::unbounded();

        // Initial state depends on force_host configuration
        let mut state = NodeStateInternal::default();
        
        if config.force_host {
            tracing::info!("Node '{}' forced to host mode", id);
            let engine = get_engine();
            
            // Use the transition function to create host state
            state.host(engine, &session, &config.keyexpr_prefix, &id).await?;
        }

        let node = Self {
            id,
            config,
            state,
            session: Arc::new(session),
            get_engine,
            command_rx,
            command_tx,
        };

        Ok(node)
    }

    /// Get node ID
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    /// Get reference to Zenoh session
    pub fn session(&self) -> &Arc<zenoh::Session> {
        &self.session
    }

    /// Get a sender for sending commands to this node
    pub fn sender(&self) -> flume::Sender<NodeCommand<E::Action>> {
        self.command_tx.clone()
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

        // Dispatch based on current state
        match self.state {
            NodeStateInternal::SearchingHost => self.search_for_host().await,
            NodeStateInternal::Client { .. } => self.process_client().await,
            NodeStateInternal::Host { .. } => self.process_host().await,
        }
    }

    /// Process actions when in Client state
    ///
    /// Handles commands from the command channel while connected to a host.
    /// Returns when either:
    /// - The step timeout elapses
    /// - A Stop command is received (returns None)
    async fn process_client(&mut self) -> Result<Option<NodeStatus<E::State>>> {
        let timeout = tokio::time::Duration::from_millis(self.config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout or shutdown
        loop {
            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&self.state),
                        game_state: None,
                    }));
                }
                // Command received
                result = self.command_rx.recv_async() => match result {
                    Err(_) => {
                        tracing::info!("Node '{}' command channel closed", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command, exiting", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::GameAction(_action)) => {
                        if let NodeStateInternal::Client { host_id } = &self.state {
                            tracing::debug!(
                                "Node '{}' forwarding action to host '{}'",
                                self.id,
                                host_id
                            );
                            // TODO: Forward action to remote host via Zenoh pub/sub
                            // Placeholder for Phase 4 implementation
                        }
                    }
                }
            }
        }
    }

    /// Process actions when in Host state
    ///
    /// Handles commands from the command channel and processes game actions through the engine.
    /// Returns when either:
    /// - A new game state is produced by the engine
    /// - The step timeout elapses
    /// - A Stop command is received (returns None)
    async fn process_host(&mut self) -> Result<Option<NodeStatus<E::State>>> {
        let timeout = tokio::time::Duration::from_millis(self.config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout or new state
        loop {
            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&self.state),
                        game_state: None,
                    }));
                }
                // Query received from a client
                query_result = self.receive_query() => {
                    if let Some(_query) = query_result {
                        tracing::debug!("Node '{}' received query from client", self.id);
                        // TODO: Process query and send response with host info
                    }
                }
                // Command received
                result = self.command_rx.recv_async() => match result {
                    Err(_) => {
                        tracing::info!("Node '{}' command channel closed", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command, exiting", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::GameAction(action)) => {
                        if let NodeStateInternal::Host { engine, .. } = &mut self.state {
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

    /// Receive a query from the host's queryable
    ///
    /// Returns Some(query) if a query was received, None if no query is available
    async fn receive_query(&self) -> Option<zenoh::query::Query> {
        if let NodeStateInternal::Host { queryable: Some(q), .. } = &self.state {
            return q.recv_query().await.ok();
        }
        None
    }

    /// Search for available hosts and attempt to connect
    ///
    /// Uses NodeQuerier to find and connect to available hosts. If timeout expires or
    /// no hosts are available/accept connection, transitions to Host state.
    async fn search_for_host(&mut self) -> Result<Option<NodeStatus<E::State>>> {
        use crate::network::NodeQuerier;

        tracing::info!("Node '{}' searching for hosts...", self.id);

        let search_timeout = tokio::time::Duration::from_millis(self.config.search_timeout_ms);
        let sleep = tokio::time::sleep(search_timeout);
        tokio::pin!(sleep);

        // Wait for connection success or timeout
        // Returns None if should become host, Some(host_id) if connected
        let connected_host = loop {
            tokio::select! {
                // Search timeout elapsed - no successful connection, become host
                () = &mut sleep => {
                    tracing::info!(
                        "Node '{}' search timeout - no hosts accepted connection",
                        self.id
                    );
                    break None;
                }
                // Try to connect to available hosts
                connection_result = NodeQuerier::connect(&self.session, &self.config.keyexpr_prefix, self.id.clone()) => {
                    match connection_result {
                        Ok(Some(host_id)) => {
                            // Successfully connected to a host
                            tracing::info!("Node '{}' connected to host: {}", self.id, host_id);
                            break Some(host_id);
                        }
                        Ok(None) => {
                            // No hosts available, become host
                            tracing::info!("Node '{}' no hosts available", self.id);
                            break None;
                        }
                        Err(e) => {
                            tracing::warn!("Node '{}' query error during search: {}", self.id, e);
                            return Err(e);
                        }
                    }
                }
                // Check for Stop command while searching
                result = self.command_rx.recv_async() => match result {
                    Err(_) => {
                        tracing::info!("Node '{}' command channel closed during search", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command during search, exiting", self.id);
                        return Ok(None);
                    }
                    Ok(NodeCommand::GameAction(_)) => {
                        tracing::warn!(
                            "Node '{}' received action while searching for host, ignoring",
                            self.id
                        );
                        // Continue searching
                    }
                }
            }
        };

        // Handle connection result - state transition after select!
        if let Some(host_id) = connected_host {
            self.state.client(host_id);
            Ok(Some(NodeStatus {
                state: NodeState::from(&self.state),
                game_state: None,
            }))
        } else {
            self.state.host(
                (self.get_engine)(),
                &self.session,
                &self.config.keyexpr_prefix,
                &self.id
            ).await?;
            Ok(Some(NodeStatus {
                state: NodeState::from(&self.state),
                game_state: None,
            }))
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

    /// Process an action and return new state
    fn process_action(&mut self, action: Self::Action, client_id: &NodeId) -> Result<Self::State>;

    /// Maximum number of clients allowed (None = unlimited)
    fn max_clients(&self) -> Option<usize>;
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

        fn process_action(
            &mut self,
            _action: Self::Action,
            _client_id: &NodeId,
        ) -> Result<Self::State> {
            Ok("processed".to_string())
        }

        fn max_clients(&self) -> Option<usize> {
            Some(4)
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
        use crate::session_ext::SessionExt;
        
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let result = session.declare_arena_node(get_engine).await;
        assert!(result.is_ok());

        let node = result.unwrap();
        assert!(!node.id().as_str().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_custom_name() {
        use crate::session_ext::SessionExt;
        
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let result = session
            .declare_arena_node(get_engine)
            .name("my_custom_node".to_string())
            .unwrap()
            .await;
        assert!(result.is_ok());

        let node = result.unwrap();
        assert_eq!(node.id().as_str(), "my_custom_node");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_invalid_name() {
        use crate::session_ext::SessionExt;
        
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let builder_result = session
            .declare_arena_node(get_engine)
            .name("invalid/name".to_string());
        
        assert!(builder_result.is_err());
        if let Err(e) = builder_result {
            match e {
                ArenaError::InvalidNodeName(_) => {} // Expected
                other => panic!("Expected InvalidNodeName error, got {:?}", other),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_step_with_force_host() {
        use crate::session_ext::SessionExt;
        
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let mut node = session
            .declare_arena_node(get_engine)
            .force_host(true)
            .await
            .unwrap();

        let command_tx = node.sender();

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
        use crate::session_ext::SessionExt;
        
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let node = session
            .declare_arena_node(get_engine)
            .force_host(true)
            .await
            .unwrap();
        // Node should be in Host state when force_host is true
        assert!(matches!(node.state, NodeStateInternal::Host { .. }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_default_starts_in_searching_state() {
        use crate::session_ext::SessionExt;
        
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let node = session
            .declare_arena_node(get_engine)
            .await
            .unwrap();
        // Node should be in SearchingHost state by default
        assert!(matches!(node.state, NodeStateInternal::SearchingHost));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_processes_actions_in_host_mode() {
        use crate::session_ext::SessionExt;
        
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let get_engine = || TestEngine;

        let mut node = session
            .declare_arena_node(get_engine)
            .force_host(true)
            .step_timeout_ms(50)
            .await
            .unwrap();

        let command_tx = node.sender();

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_session_ext_declare_arena_node() {
        use crate::session_ext::SessionExt;

        // Create a zenoh session
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();

        // Use the extension trait to declare a node (name must be called first)
        let node = session
            .declare_arena_node(|| TestEngine)
            .name("test_node".to_string())
            .unwrap()
            .force_host(true)
            .step_timeout_ms(50)
            .await
            .unwrap();

        // Verify the node was created correctly
        assert_eq!(node.id().as_str(), "test_node");

        // Get the sender and send an action to verify it works
        let sender = node.sender();
        sender
            .send_async(NodeCommand::GameAction(42))
            .await
            .unwrap();

        // Drop the node and sender
        drop(sender);
        drop(node);
    }
}
