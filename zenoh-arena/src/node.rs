/// Node management module
use crate::config::NodeConfig;
use crate::error::{ArenaError, Result};
use crate::network::NodeLivelinessToken;
use crate::types::{NodeId, NodeState, NodeStateInternal};

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
    config: NodeConfig,

    /// Current node state
    state: NodeStateInternal<E>,

    /// Zenoh session
    session: zenoh::Session,

    /// Engine factory - called when transitioning to host mode
    get_engine: F,

    /// Receiver for commands from the application
    command_rx: flume::Receiver<NodeCommand<E::Action>>,

    /// Sender for commands from the application
    command_tx: flume::Sender<NodeCommand<E::Action>>,

    /// Liveliness token for this node's identity (Role::Node)
    /// Kept throughout the node's lifetime to protect against other nodes with the same name
    _node_liveliness_token: NodeLivelinessToken,
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
            _node_liveliness_token: node_liveliness_token,
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
    /// Processes commands from the command channel and returns the current node state with optional game state.
    /// Returns when either:
    /// - A new game state is produced by the engine
    /// - The step timeout (configured in NodeConfig) elapses
    /// - A Stop command is received (returns None)
    ///
    /// Returns None if Stop command was received, indicating the node should shut down.
    pub async fn step(&mut self) -> Result<Option<NodeState<E::State>>> {
        // If force_host is enabled, only Host state is allowed
        if self.config.force_host && !matches!(self.state, NodeStateInternal::Host { .. }) {
            return Err(ArenaError::Internal(
                "force_host is enabled but node is not in Host state".to_string(),
            ));
        }

        // Dispatch based on current state using state-specific run methods
        let next_result = match std::mem::replace(&mut self.state, NodeStateInternal::SearchingHost) {
            NodeStateInternal::SearchingHost => {
                use crate::searching_host_state::SearchingHostState;
                let searching_state = SearchingHostState;
                searching_state
                    .step(
                        &self.session,
                        &self.config,
                        &self.id,
                        &self.command_rx,
                        &self.get_engine,
                    )
                    .await
            }
            NodeStateInternal::Client(client_state) => {
                client_state
                    .step::<E>(&self.config, &self.id, &self.command_rx)
                    .await
            }
            NodeStateInternal::Host(host_state) => {
                host_state
                    .step(&self.config, &self.id, &self.session, &self.command_rx)
                    .await
            }
        };
        
        // Update state with the next state returned from run() and generate NodeState
        match next_result {
            Ok(Some(next_state)) => {
                let state = NodeState::from(&next_state);
                self.state = next_state;
                Ok(Some(state))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
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

        // Test one step first
        let status = node.step().await.unwrap();
        assert!(status.is_some());

        // Send Stop command via async channel
        command_tx.send(NodeCommand::Stop).unwrap();

        // Execute the next step which should return None due to Stop command
        let result = node.step().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
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
        let _state1 = node.step().await.unwrap();
        assert!(_state1.is_some());
        
        // Game state is now stored internally in HostState
        if let NodeStateInternal::Host(host_state) = &node.state {
            assert!(host_state.game_state.is_some());
            assert_eq!(host_state.game_state.as_ref().unwrap(), &"processed");
        } else {
            panic!("Expected node to be in Host state");
        }

        // Call step to process second action
        let _state2 = node.step().await.unwrap();
        assert!(_state2.is_some());
        
        // Game state is updated
        if let NodeStateInternal::Host(host_state) = &node.state {
            assert!(host_state.game_state.is_some());
            assert_eq!(host_state.game_state.as_ref().unwrap(), &"processed");
        } else {
            panic!("Expected node to be in Host state");
        }
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
