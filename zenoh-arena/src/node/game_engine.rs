use crate::node::types::NodeId;
use std::future::Future;
use std::pin::Pin;

/// Trait for game engine integration
///
/// The engine exists independently of the Node and manages its own lifecycle.
/// It provides channels for action input and state output, and responds to
/// lifecycle events (set_node_id, run, stop).
///
/// The engine should use interior mutability (e.g., Mutex, RwLock) for mutable state
/// since all methods take &self to work with Arc<dyn GameEngine>.
pub trait GameEngine: Send + Sync {
    /// Action type from user/client
    type Action: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send;

    /// State type sent to clients
    type State: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone;

    /// Maximum number of clients allowed (None = unlimited)
    fn max_clients(&self) -> Option<usize>;

    /// Set the node ID for this engine
    ///
    /// Called once when the Node is created, before any other methods.
    /// The engine should store this ID to identify itself as a player.
    fn set_node_id(&self, node_id: NodeId);

    /// Start the engine with optional initial state
    ///
    /// Called when the node enters Host mode. The engine should start
    /// its game loop, reading from action_sender's channel and writing
    /// to state_receiver's channel.
    ///
    /// # Arguments
    /// * `initial_state` - Optional state to restore the engine to
    fn run(&self, initial_state: Option<Self::State>) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Stop the engine
    ///
    /// Called when the node leaves Host mode. The engine should stop
    /// its game loop and clean up any resources.
    fn stop(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Get sender for actions from clients
    ///
    /// Returns a reference to the channel that the host uses to send
    /// actions (with NodeId) to the engine.
    fn action_sender(&self) -> &flume::Sender<(NodeId, Self::Action)>;

    /// Get receiver for state updates from engine
    ///
    /// Returns a reference to the channel that the engine uses to send
    /// state updates to the host for broadcasting.
    fn state_receiver(&self) -> &flume::Receiver<Self::State>;
}
