/// Node management module
use std::sync::Arc;

use super::config::NodeConfig;
use super::game_engine::GameEngine;
use super::stats::{NodeStats, StatsTracker};
use crate::error::{ArenaError, Result};
use crate::network::NodeLivelinessToken;
use crate::network::keyexpr::NodeType;
use super::types::{NodeId, NodeState, NodeStateInternal, StepResult};

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
pub struct Node<A, S>
where
    A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send,
    S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone,
{
    /// Node identifier
    id: NodeId,

    /// Node configuration
    config: NodeConfig,

    /// Current node state
    state: NodeStateInternal<A, S>,

    /// Zenoh session
    session: zenoh::Session,

    /// Game engine reference
    engine: Arc<dyn GameEngine<Action = A, State = S>>,

    /// Receiver for commands from the application
    command_rx: flume::Receiver<NodeCommand<A>>,

    /// Sender for commands from the application
    command_tx: flume::Sender<NodeCommand<A>>,

    /// Liveliness token for this node's identity (Role::Node)
    /// Kept throughout the node's lifetime to protect against other nodes with the same name
    _node_liveliness_token: NodeLivelinessToken,

    /// Current game state (maintained across state transitions)
    game_state: Option<S>,

    /// Statistics tracker for monitoring data throughput
    stats_tracker: Arc<StatsTracker>,
}

impl<A, S> Node<A, S>
where
    A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send,
    S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone,
{
    /// Create a new Node instance (internal use only - use builder pattern via SessionExt)
    pub(crate) async fn new_internal(
        config: NodeConfig,
        session: zenoh::Session,
        engine: Arc<dyn GameEngine<Action = A, State = S>>,
    ) -> Result<Self> {
        let id = config.node_id.clone();

        tracing::info!("Node '{}' initialized with Zenoh session", id);

        // Inform engine of its node ID
        engine.set_node_id(id.clone());

        // Create liveliness token for this node's identity (NodeType::Node)
        // This protects the node name from conflicts with other nodes
        let node_liveliness_token = NodeLivelinessToken::declare(
            &session,
            config.keyexpr_prefix.clone(),
            NodeType::Node,
            id.clone(),
        )
        .await?;

        // Create command channel
        let (command_tx, command_rx) = flume::unbounded();

        // Create stats tracker
        let stats_tracker = Arc::new(StatsTracker::new());

        // Initial state depends on force_host configuration
        let state = if config.force_host {
            tracing::info!("Node '{}' forced to host mode", id);

            // Use the constructor function to create host state with no initial state
            NodeStateInternal::host(
                engine.clone(),
                &session,
                config.keyexpr_prefix.clone(),
                &id,
                None, // No initial state when force starting as host
                stats_tracker.clone(),
            )
                .await?
        } else {
            // Start in searching state with no initial state
            NodeStateInternal::searching()
        };

        let node = Self {
            id,
            config,
            state,
            session,
            engine,
            command_rx,
            command_tx,
            _node_liveliness_token: node_liveliness_token,
            game_state: None,
            stats_tracker,
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
    pub fn sender(&self) -> flume::Sender<NodeCommand<A>> {
        self.command_tx.clone()
    }

    /// Execute one step of the node state machine
    ///
    /// Processes commands from the command channel and returns the result of the step.
    /// Returns when either:
    /// - A new game state is produced by the engine (returns GameState)
    /// - The step timeout (configured in NodeConfig) elapses (returns Timeout)
    /// - A Stop command is received (returns Stop)
    pub async fn step(&mut self) -> Result<StepResult<S>> {
        // If force_host is enabled, only Host state is allowed
        if self.config.force_host && !matches!(self.state, NodeStateInternal::Host { .. }) {
            return Err(ArenaError::Internal(
                "force_host is enabled but node is not in Host state".to_string(),
            ));
        }

        // Dispatch based on current state using state-specific run methods
        let node_state = std::mem::replace(&mut self.state, NodeStateInternal::Stop);
        let (next_node_state, step_result) = match node_state {
            NodeStateInternal::SearchingHost(searching_state) => {
                searching_state
                    .step(
                        &self.session,
                        &self.config,
                        &self.id,
                        &self.command_rx,
                        self.engine.clone(),
                        self.game_state.clone(),
                        self.stats_tracker.clone(),
                    )
                    .await?
            }
            NodeStateInternal::Client(client_state) => {
                client_state
                    .step(&self.config, &self.id, &self.command_rx, self.game_state.clone())
                    .await?
            }
            NodeStateInternal::Host(host_state) => {
                host_state
                    .step(&self.config, &self.id, &self.session, &self.command_rx)
                    .await?
            }
            NodeStateInternal::Stop => {
                // If already stopped, remain stopped
                (
                    NodeStateInternal::Stop,
                    StepResult::Stop,
                )
            }
        };
        self.state = next_node_state;
        // Update stored game state if a new one was produced
        if let StepResult::GameState(new_state) = &step_result {
            self.game_state = Some(new_state.clone());
        }
        Ok(step_result)
    }

    /// Get the current node state without advancing the state machine
    pub fn state(&self) -> NodeState {
        self.state.to_node_state()
    }

    /// Get the current node state without game state
    pub fn node_state(&self) -> NodeState {
        self.state.to_node_state()
    }

    /// Get the current game state if available
    pub fn game_state(&self) -> Option<S> {
        self.game_state.clone()
    }

    /// Get current node statistics
    ///
    /// Returns a snapshot of the current throughput statistics including:
    /// - Total input/output bytes
    /// - Input/output throughput in KB/s
    pub fn stats(&self) -> NodeStats {
        self.stats_tracker.get_stats()
    }

    /// Reset node statistics
    ///
    /// Resets all byte counters to zero and restarts the timer for throughput calculation
    pub fn reset_stats(&self) {
        self.stats_tracker.reset();
    }
}

#[cfg(test)]
mod tests {
    use crate::node::session_ext::SessionExt;
    use super::GameEngine;

    use super::*;

    // Simple test engine for testing purposes
    #[derive(Debug)]
    struct TestEngine {
        #[allow(dead_code)]
        max_clients: Option<usize>,
        node_id: std::sync::Mutex<Option<NodeId>>,
        input_tx: flume::Sender<(NodeId, u32)>,
        output_rx: flume::Receiver<String>,
        stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl TestEngine {
        fn new() -> Self {
            let (input_tx, input_rx) = flume::unbounded();
            let (output_tx, output_rx) = flume::unbounded();
            let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stop_flag_clone = stop_flag.clone();
            
            // Spawn a task to process actions
            std::thread::spawn(move || {
                while !stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Ok((_node_id, _action)) = input_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        // Process the action
                        let _ = output_tx.send("processed".to_string());
                    }
                }
            });

            Self {
                max_clients: Some(4),
                node_id: std::sync::Mutex::new(None),
                input_tx,
                output_rx,
                stop_flag,
            }
        }
    }

    impl GameEngine for TestEngine {
        type Action = u32;
        type State = String;

        fn max_clients(&self) -> Option<usize> {
            self.max_clients
        }
        
        fn set_node_id(&self, node_id: NodeId) {
            *self.node_id.lock().unwrap() = Some(node_id);
        }
        
        fn run(&self, _initial_state: Option<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
            Box::pin(async {
                self.stop_flag.store(false, std::sync::atomic::Ordering::Relaxed);
            })
        }
        
        fn stop(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
            Box::pin(async {
                self.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            })
        }
        
        fn action_sender(&self) -> &flume::Sender<(NodeId, u32)> {
            &self.input_tx
        }
        
        fn state_receiver(&self) -> &flume::Receiver<String> {
            &self.output_rx
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
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        let result = session.declare_arena_node(engine).await;
        assert!(result.is_ok());

        let node = result.unwrap();
        assert!(!node.id().as_str().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_custom_name() {
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        let result = session
            .declare_arena_node(engine)
            .name("my_custom_node".to_string())
            .unwrap()
            .await;
        assert!(result.is_ok());

        let node = result.unwrap();
        assert_eq!(node.id().as_str(), "my_custom_node");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_creation_with_invalid_name() {
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        let builder_result = session
            .declare_arena_node(engine)
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
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        let mut node = session
            .declare_arena_node(engine)
            .force_host(true)
            .await
            .unwrap();

        let command_tx = node.sender();

        // Test one step first
        let status = node.step().await.unwrap();
        assert!(!matches!(status, StepResult::Stop));

        // Send Stop command via async channel
        command_tx.send(NodeCommand::Stop).unwrap();

        // Execute the next step which should return Stop state
        let result = node.step().await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), StepResult::Stop));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_force_host_starts_in_host_state() {
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        let node = session
            .declare_arena_node(engine)
            .force_host(true)
            .await
            .unwrap();
        // Node should be in Host state when force_host is true
        assert!(matches!(node.state, NodeStateInternal::Host { .. }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_default_starts_in_searching_state() {
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        let node = session.declare_arena_node(engine).await.unwrap();
        // Node should be in SearchingHost state by default
        assert!(matches!(node.state, NodeStateInternal::SearchingHost(_)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_node_processes_actions_in_host_mode() {
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        let mut node = session
            .declare_arena_node(engine)
            .force_host(true)
            .step_timeout_break_ms(50)
            .await
            .unwrap();

        let command_tx = node.sender();

        // Send some game actions
        command_tx.send(NodeCommand::GameAction(42)).unwrap();
        command_tx.send(NodeCommand::GameAction(100)).unwrap();

        // Call step to process first action
        let _state1 = node.step().await.unwrap();
        assert!(!matches!(_state1, StepResult::Stop));
        
        // Game state is now stored in Node itself
        assert!(node.game_state.is_some());
        assert_eq!(node.game_state.as_ref().unwrap(), &"processed");

        // Call step to process second action
        let _state2 = node.step().await.unwrap();
        assert!(!matches!(_state2, StepResult::Stop));
        
        // Game state is updated
        assert!(node.game_state.is_some());
        assert_eq!(node.game_state.as_ref().unwrap(), &"processed");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_session_ext_declare_arena_node() {
        // Create a zenoh session
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let engine = Arc::new(TestEngine::new());

        // Use the extension trait to declare a node (name must be called first)
        let node = session
            .declare_arena_node(engine)
            .name("test_node".to_string())
            .unwrap()
            .force_host(true)
            .step_timeout_break_ms(50)
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
    }
}
