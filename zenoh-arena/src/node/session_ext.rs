use zenoh::{Resolvable, key_expr::KeyExpr};
use crate::error::Result;
use std::sync::Arc;

use crate::node::{config::NodeConfig, game_engine::GameEngine, arena_node::Node, types::NodeId};

/// Extension trait for zenoh::Session to declare arena nodes
/// Extension trait for zenoh::Session to add arena node declaration
pub trait SessionExt {
    /// Declare an arena node
    ///
    /// # Example
    /// ```no_run
    /// use zenoh_arena::{SessionExt, GameEngine, NodeId};
    /// use std::sync::Arc;
    ///
    /// # struct MyEngine;
    /// # impl GameEngine for MyEngine {
    /// #     type Action = String;
    /// #     type State = String;
    /// #     fn max_clients(&self) -> Option<usize> { None }
    /// #     fn set_node_id(&self, _node_id: NodeId) {}
    /// #     fn run(&self, _initial_state: Option<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> { Box::pin(async {}) }
    /// #     fn stop(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> { Box::pin(async {}) }
    /// #     fn action_sender(&self) -> &flume::Sender<(NodeId, String)> { unimplemented!() }
    /// #     fn state_receiver(&self) -> &flume::Receiver<String> { unimplemented!() }
    /// # }
    /// # async fn example() {
    /// let session = zenoh::open(zenoh::Config::default()).await.unwrap();
    /// let engine = Arc::new(MyEngine);
    /// let node = session
    ///     .declare_arena_node(engine)
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    fn declare_arena_node<A, S>(&self, engine: Arc<dyn GameEngine<Action = A, State = S>>) -> NodeBuilder<'_, A, S>
    where
        A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + 'static,
        S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone + 'static;
}

impl SessionExt for zenoh::Session {
    fn declare_arena_node<A, S>(&self, engine: Arc<dyn GameEngine<Action = A, State = S>>) -> NodeBuilder<'_, A, S>
    where
        A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + 'static,
        S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone + 'static,
    {
        NodeBuilder::new(self, engine)
    }
}

/// Builder for arena nodes
///
/// Allows configuring the node before creating it, similar to zenoh's builder pattern.
#[must_use = "Resolvables do nothing unless you resolve them using `.await` or `zenoh::Wait::wait`"]
pub struct NodeBuilder<'a, A, S>
where
    A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send,
    S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone,
{
    session: &'a zenoh::Session,
    engine: Arc<dyn GameEngine<Action = A, State = S>>,
    config: NodeConfig,
}

impl<'a, A, S> NodeBuilder<'a, A, S>
where
    A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send,
    S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone,
{
    /// Create a new NodeBuilder
    fn new(session: &'a zenoh::Session, engine: Arc<dyn GameEngine<Action = A, State = S>>) -> Self {
        Self {
            session,
            engine,
            config: NodeConfig::default(),
        }
    }

    /// Set the node name
    pub fn name(mut self, name: String) -> Result<Self> {
        self.config.node_id = NodeId::from_name(name)?;
        Ok(self)
    }

    /// Enable force_host mode
    pub fn force_host(mut self, force_host: bool) -> Self {
        self.config.force_host = force_host;
        self
    }

    /// Set the step timeout in milliseconds
    pub fn step_timeout_break_ms(mut self, timeout_ms: u64) -> Self {
        self.config.step_timeout_break_ms = timeout_ms;
        self
    }

    /// Set the search timeout in milliseconds
    /// When in SearchingHost state, if no hosts are found within this timeout,
    /// the node transitions to Host state
    pub fn search_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.config.search_timeout_ms = timeout_ms;
        self
    }

    /// Set the maximum random jitter before searching for hosts (in milliseconds)
    /// Adds a randomized delay (0..jitter_ms) before querying for hosts.
    /// This prevents the "thundering herd" problem when multiple clients lose
    /// their host simultaneously and all try to become the new host at once.
    pub fn search_jitter_ms(mut self, jitter_ms: u64) -> Self {
        self.config.search_jitter_ms = jitter_ms;
        self
    }

    /// Set the key expression prefix
    pub fn prefix(mut self, prefix: KeyExpr<'static>) -> Self {
        self.config.keyexpr_prefix = prefix;
        self
    }
}

impl<'a, A, S> Resolvable for NodeBuilder<'a, A, S>
where
    A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send,
    S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone,
{
    type To = Result<Node<A, S>>;
}

impl<'a, A, S> std::future::IntoFuture for NodeBuilder<'a, A, S>
where
    A: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + 'static,
    S: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone + 'static,
{
    type Output = <Self as Resolvable>::To;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            // Clone the session since Node::new_internal takes ownership
            let session = self.session.clone();
            Node::new_internal(self.config, session, self.engine).await
        })
    }
}
