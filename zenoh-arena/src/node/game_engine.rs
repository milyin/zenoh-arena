use crate::node::types::NodeId;

/// Trait for game engine integration
///
/// The engine runs only on the host node and processes actions from clients via channels
pub trait GameEngine: Send + Sync {
    /// Action type from user/client
    type Action: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send;

    /// State type sent to clients
    type State: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone;

    /// Maximum number of clients allowed (None = unlimited)
    fn max_clients(&self) -> Option<usize>;
}

/// Type alias for engine factory function
///
/// This function creates a game engine instance given:
/// - A receiver for actions from clients (with their NodeId)
/// - A sender for broadcasting state updates to clients
///
/// The factory function should spawn any necessary background tasks for processing
/// actions and generating state updates.
///
/// # Type Parameters
/// * `E` - The GameEngine type to be created
/// * `F` - The factory function type that implements `Fn + Clone`
///
/// # Example
/// ```ignore
/// fn create_engine(
///     input_rx: flume::Receiver<(NodeId, Action)>,
///     output_tx: flume::Sender<State>
/// ) -> MyEngine {
///     // Create and return engine
/// }
/// ```
pub trait EngineFactory<E: GameEngine>: Fn(
    flume::Receiver<(NodeId, E::Action)>,
    flume::Sender<E::State>
) -> E + Clone {}

// Blanket implementation for all types that satisfy the trait bounds
impl<E, F> EngineFactory<E> for F
where
    E: GameEngine,
    F: Fn(flume::Receiver<(NodeId, E::Action)>, flume::Sender<E::State>) -> E + Clone,
{}
