/// Extension trait for zenoh::Session to declare arena nodes
use crate::config::NodeConfig;
use crate::node::{GameEngine, Node};
use crate::types::NodeId;
use crate::Result;
use zenoh::key_expr::KeyExpr;
use zenoh::Resolvable;

/// Extension trait for zenoh::Session to add arena node declaration
pub trait SessionExt {
    /// Declare an arena node
    ///
    /// # Example
    /// ```no_run
    /// use zenoh_arena::{SessionExt, GameEngine};
    ///
    /// # struct MyEngine;
    /// # impl GameEngine for MyEngine {
    /// #     type Action = String;
    /// #     type State = String;
    /// #     fn process_action(&mut self, _: Self::Action, _: &zenoh_arena::NodeId) -> zenoh_arena::Result<Self::State> {
    /// #         Ok("state".to_string())
    /// #     }
    /// # }
    /// # async fn example() {
    /// let session = zenoh::open(zenoh::Config::default()).await.unwrap();
    /// let node = session
    ///     .declare_arena_node(|| MyEngine)
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    fn declare_arena_node<E, F>(&self, get_engine: F) -> NodeBuilder<'_, E, F>
    where
        E: GameEngine,
        F: Fn() -> E;
}

impl SessionExt for zenoh::Session {
    fn declare_arena_node<E, F>(&self, get_engine: F) -> NodeBuilder<'_, E, F>
    where
        E: GameEngine,
        F: Fn() -> E,
    {
        NodeBuilder::new(self, get_engine)
    }
}

/// Builder for arena nodes
///
/// Allows configuring the node before creating it, similar to zenoh's builder pattern.
#[must_use = "Resolvables do nothing unless you resolve them using `.await` or `zenoh::Wait::wait`"]
pub struct NodeBuilder<'a, E: GameEngine, F: Fn() -> E> {
    session: &'a zenoh::Session,
    get_engine: F,
    config: NodeConfig,
    _phantom: std::marker::PhantomData<E>,
}

impl<'a, E: GameEngine, F: Fn() -> E> NodeBuilder<'a, E, F> {
    /// Create a new NodeBuilder
    fn new(session: &'a zenoh::Session, get_engine: F) -> Self {
        Self {
            session,
            get_engine,
            config: NodeConfig::default(),
            _phantom: std::marker::PhantomData,
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
    pub fn step_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.config.step_timeout_ms = timeout_ms;
        self
    }

    /// Set the key expression prefix
    pub fn prefix(mut self, prefix: KeyExpr<'static>) -> Self {
        self.config.keyexpr_prefix = prefix;
        self
    }
}

impl<'a, E: GameEngine, F: Fn() -> E> Resolvable for NodeBuilder<'a, E, F> {
    type To = Result<Node<E, F>>;
}

impl<'a, E: GameEngine, F: Fn() -> E + Send + 'a> std::future::IntoFuture for NodeBuilder<'a, E, F> {
    type Output = <Self as Resolvable>::To;
    type IntoFuture = std::pin::Pin<
        Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>,
    >;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            // Clone the session since Node::new_internal takes ownership
            let session = self.session.clone();
            Node::new_internal(self.config, session, self.get_engine).await
        })
    }
}
