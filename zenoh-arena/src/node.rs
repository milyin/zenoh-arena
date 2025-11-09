/// Node management module
use crate::config::NodeConfig;
use crate::error::{ArenaError, Result};
use crate::network::host_queryable::HostRequest;
use crate::network::{HostQueryable, NodeLivelinessToken, NodeLivelinessWatch};
use crate::types::{NodeId, NodeState, NodeStateInternal, NodeStatus};
use futures::FutureExt;
use std::sync::Arc;

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

    /// Zenoh session
    session: zenoh::Session,

    /// Engine factory - called when transitioning to host mode
    #[allow(dead_code)]
    get_engine: F,

    /// Receiver for commands from the application
    command_rx: flume::Receiver<NodeCommand<E::Action>>,

    /// Sender for commands from the application
    command_tx: flume::Sender<NodeCommand<E::Action>>,

    /// Liveliness token for this node's identity (Role::Node)
    /// Kept throughout the node's lifetime to protect against other nodes with the same name
    #[allow(dead_code)]
    node_liveliness_token: NodeLivelinessToken,
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

        // Create liveliness token for this node's identity (Role::Node)
        // This protects the node name from conflicts with other nodes
        let node_liveliness_token = NodeLivelinessToken::declare(
            &session,
            config.keyexpr_prefix.clone(),
            crate::network::Role::Node,
            id.clone(),
        )
        .await?;

        // Create command channel
        let (command_tx, command_rx) = flume::unbounded();

        // Initial state depends on force_host configuration
        let mut state = NodeStateInternal::default();

        if config.force_host {
            tracing::info!("Node '{}' forced to host mode", id);
            let engine = get_engine();

            // Use the transition function to create host state
            state
                .host(engine, &session, config.keyexpr_prefix.clone(), &id)
                .await?;
        }

        let node = Self {
            id,
            config,
            state,
            session,
            get_engine,
            command_rx,
            command_tx,
            node_liveliness_token,
        };

        Ok(node)
    }

    /// Get node ID
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    /// Get reference to Zenoh session
    pub fn session(&self) -> &zenoh::Session {
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
    /// Monitors liveliness of the connected host and returns to SearchingHost if disconnected.
    /// Returns when either:
    /// - Host liveliness is lost (transitions back to SearchingHost)
    /// - The step timeout elapses
    /// - A Stop command is received (returns None)
    async fn process_client(&mut self) -> Result<Option<NodeStatus<E::State>>> {
        // Extract the client state data temporarily to use the liveliness watch
        let (host_id, mut liveliness_watch, _liveliness_token) =
            match std::mem::take(&mut self.state) {
                NodeStateInternal::Client {
                    host_id,
                    liveliness_watch,
                    liveliness_token,
                } => (host_id, liveliness_watch, liveliness_token),
                other_state => {
                    // Restore state if it wasn't Client
                    self.state = other_state;
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&self.state),
                        game_state: None,
                    }));
                }
            };

        let timeout = tokio::time::Duration::from_millis(self.config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout, shutdown, or host disconnection
        loop {
            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    // No disconnection yet, restore state and return
                    self.state = NodeStateInternal::Client {
                        host_id: host_id.clone(),
                        liveliness_watch,
                        liveliness_token: _liveliness_token,
                    };
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&self.state),
                        game_state: None,
                    }));
                }
                // Host liveliness lost - disconnect and return to searching
                disconnect_result = liveliness_watch.disconnected() => {
                    match disconnect_result {
                        Ok(disconnected_id) => {
                            tracing::info!("Node '{}' detected host '{}' disconnection, returning to search", self.id, disconnected_id);
                            // Transition back to SearchingHost
                            self.state = NodeStateInternal::SearchingHost;
                            return Ok(Some(NodeStatus {
                                state: NodeState::from(&self.state),
                                game_state: None,
                            }));
                        }
                        Err(e) => {
                            tracing::warn!("Node '{}' liveliness error: {}", self.id, e);
                            // Treat error as disconnect
                            self.state = NodeStateInternal::SearchingHost;
                            return Ok(Some(NodeStatus {
                                state: NodeState::from(&self.state),
                                game_state: None,
                            }));
                        }
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
                    Ok(NodeCommand::GameAction(_action)) => {
                        tracing::debug!(
                            "Node '{}' forwarding action to host '{}'",
                            self.id,
                            host_id
                        );
                        // TODO: Forward action to remote host via Zenoh pub/sub
                        // Placeholder for Phase 4 implementation
                        // Continue the loop
                        continue;
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
        use futures::future::select_all;

        let timeout = tokio::time::Duration::from_millis(self.config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout or new state
        loop {
            // Snapshot queryable and pending futures for this iteration
            let (queryable_arc, has_pending) = match &self.state {
                NodeStateInternal::Host {
                    queryable,
                    pending_client_disconnects,
                    ..
                } => (queryable.clone(), !pending_client_disconnects.is_empty()),
                _ => {
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&self.state),
                        game_state: None,
                    }));
                }
            };

            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&self.state),
                        game_state: None,
                    }));
                }
                // Query received from a client (connection request)
                request_result = async {
                    let queryable = queryable_arc.clone().expect("queryable available");
                    queryable.expect_connection().await
                }, if queryable_arc.is_some() => {
                    if let Ok(request) = request_result {
                        self.handle_connection_request(request).await?;
                    }
                }
                // Client disconnect detected - race over pending futures using select_all
                (client_id, disconnect_result) = async {
                    let pending = match &mut self.state {
                        NodeStateInternal::Host {
                            pending_client_disconnects,
                            ..
                        } => std::mem::take(pending_client_disconnects),
                        _ => Vec::new(),
                    };

                    if pending.is_empty() {
                        // No pending futures - return a never-completing future
                        futures::future::pending().await
                    } else {
                        let select_all_fut = select_all(pending).fuse();
                        futures::pin_mut!(select_all_fut);
                        let (result, _idx, remaining) = select_all_fut.await;
                        
                        // Restore remaining futures to state
                        if let NodeStateInternal::Host {
                            pending_client_disconnects,
                            ..
                        } = &mut self.state
                        {
                            *pending_client_disconnects = remaining;
                        }
                        
                        result
                    }
                }, if has_pending => {
                    self.handle_client_disconnect(client_id, disconnect_result).await?;
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

    /// Search for available hosts and attempt to connect
    ///
    /// Uses NodeQuerier to find and connect to available hosts. If timeout expires or
    /// no hosts are available/accept connection, transitions to Host state.
    async fn search_for_host(&mut self) -> Result<Option<NodeStatus<E::State>>> {
        use crate::network::HostQuerier;

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
                connection_result = HostQuerier::connect(&self.session, self.config.keyexpr_prefix.clone(), self.id.clone()) => {
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
            self.state
                .client(
                    &self.session,
                    self.config.keyexpr_prefix.clone(),
                    host_id,
                    self.id.clone(),
                )
                .await?;
            Ok(Some(NodeStatus {
                state: NodeState::from(&self.state),
                game_state: None,
            }))
        } else {
            self.state
                .host(
                    (self.get_engine)(),
                    &self.session,
                    self.config.keyexpr_prefix.clone(),
                    &self.id,
                )
                .await?;
            Ok(Some(NodeStatus {
                state: NodeState::from(&self.state),
                game_state: None,
            }))
        }
    }

    /// Handle a connection request from a client
    ///
    /// Checks if the node is in host mode and if the current client count is below the maximum.
    /// Accepts the connection if capacity is available, otherwise rejects it.
    async fn handle_connection_request(&mut self, request: HostRequest) -> Result<()> {
        let NodeStateInternal::Host {
            engine,
            connected_clients,
            queryable,
            pending_client_disconnects,
            ..
        } = &mut self.state
        else {
            tracing::warn!(
                "Node '{}' received connection request but not in host mode",
                self.id
            );
            return Ok(());
        };

        let current_count = connected_clients.len();
        let max_allowed = engine.max_clients();

        let should_accept = max_allowed.map(|max| current_count < max).unwrap_or(true); // Accept if no limit

        if should_accept {
            match request.accept().await {
                Ok(client_id) => {
                    tracing::info!(
                        "Node '{}' accepted connection from client '{}' ({}/{})",
                        self.id,
                        client_id,
                        connected_clients.len() + 1,
                        max_allowed
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "unlimited".to_string())
                    );
                    // Track accepted client
                    connected_clients.push(client_id.clone());

                    // Register liveliness watch for the client so we can detect disconnects
                    let client_id_for_watch = client_id.clone();

                    let mut watch = NodeLivelinessWatch::new(client_id_for_watch.clone());
                    match watch
                        .subscribe(
                            &self.session,
                            self.config.keyexpr_prefix.clone(),
                            crate::network::Role::Client,
                            &client_id_for_watch,
                        )
                        .await
                    {
                        Ok(()) => {
                            let future: futures::future::BoxFuture<'static, (NodeId, Result<NodeId>)> =
                                async move {
                                    let result = watch.disconnected().await;
                                    (client_id_for_watch, result)
                                }
                                .boxed();

                            pending_client_disconnects.push(future);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Node '{}' failed to create liveliness watch for client '{}': {}",
                                self.id,
                                client_id,
                                e
                            );
                        }
                    }

                    // Update queryable if we've reached capacity
                    let new_count = connected_clients.len();
                    let has_capacity = match max_allowed {
                        None => true, // Unlimited clients
                        Some(max_count) => new_count < max_count,
                    };

                    if !has_capacity && queryable.is_some() {
                        *queryable = None;
                        tracing::debug!("Host '{}' capacity reached (dropped queryable)", self.id);
                    }
                }
                Err(e) => {
                    tracing::warn!("Node '{}' failed to accept connection: {:?}", self.id, e);
                }
            }
        } else {
            tracing::info!(
                "Node '{}' rejected connection from client '{}' (limit reached: {}/{})",
                self.id,
                request.client_id().as_str(),
                current_count,
                max_allowed.unwrap_or(0)
            );
            if let Err(e) = request.reject("Maximum number of clients reached").await {
                tracing::warn!("Node '{}' failed to reject connection: {:?}", self.id, e);
            }
        }
        Ok(())
    }
}

impl<E: GameEngine, F: Fn() -> E> Node<E, F> {
    async fn handle_client_disconnect(
        &mut self,
        client_id: NodeId,
        disconnect_result: Result<NodeId>,
    ) -> Result<()> {
        let NodeStateInternal::Host {
            connected_clients,
            queryable,
            engine,
            ..
        } = &mut self.state
        else {
            return Ok(());
        };

        match disconnect_result {
            Ok(disconnected_id) => tracing::info!(
                "Node '{}' detected client '{}' disconnect (liveliness watch returned: {})",
                self.id,
                client_id,
                disconnected_id
            ),
            Err(e) => tracing::warn!(
                "Node '{}' client '{}' liveliness error: {}",
                self.id,
                client_id,
                e
            ),
        }

        let removed = if let Some(pos) = connected_clients.iter().position(|id| id == &client_id) {
            connected_clients.remove(pos);
            true
        } else {
            false
        };

        if !removed {
            tracing::debug!(
                "Node '{}' received disconnect for unknown client '{}'",
                self.id,
                client_id
            );
        }

        let has_capacity = match engine.max_clients() {
            None => true,
            Some(max) => connected_clients.len() < max,
        };

        if has_capacity && queryable.is_none() {
            let new_queryable = HostQueryable::declare(
                &self.session,
                self.config.keyexpr_prefix.clone(),
                self.id.clone(),
            )
            .await?;
            *queryable = Some(Arc::new(new_queryable));
            tracing::debug!("Host '{}' resumed accepting clients", self.id);
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

        let node = session.declare_arena_node(get_engine).await.unwrap();
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
