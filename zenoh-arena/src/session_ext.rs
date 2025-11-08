/// Extension trait for zenoh::Session to declare arena nodes
use crate::config::NodeConfig;
use crate::node::{GameEngine, Node, NodeCommand};
use crate::Result;
use zenoh_core::{Resolvable, Wait};

/// Extension trait for zenoh::Session to add arena node declaration
pub trait SessionExt {
    /// Declare an arena node
    ///
    /// # Example
    /// ```no_run
    /// use zenoh_arena::{SessionExt, NodeConfig, GameEngine};
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
    /// let (node, sender) = session
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
    pub fn name(mut self, name: String) -> Self {
        self.config = self.config.with_node_name(name);
        self
    }

    /// Enable force_host mode
    pub fn force_host(mut self, force_host: bool) -> Self {
        self.config = self.config.with_force_host(force_host);
        self
    }

    /// Set the step timeout in milliseconds
    pub fn step_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.config = self.config.with_step_timeout_ms(timeout_ms);
        self
    }
}

impl<'a, E: GameEngine, F: Fn() -> E> Resolvable for NodeBuilder<'a, E, F> {
    type To = Result<(Node<E, F>, flume::Sender<NodeCommand<E::Action>>)>;
}

impl<'a, E: GameEngine, F: Fn() -> E> Wait for NodeBuilder<'a, E, F> {
    fn wait(self) -> <Self as Resolvable>::To {
        // We need to block on the async Node::new() method
        // This is necessary because Wait::wait() is not async
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Clone the session since Node::new takes ownership
                let session = self.session.clone();
                Node::new(self.config, session, self.get_engine).await
            })
        })
    }
}

impl<'a, E: GameEngine, F: Fn() -> E + Send + 'a> std::future::IntoFuture for NodeBuilder<'a, E, F> {
    type Output = <Self as Resolvable>::To;
    type IntoFuture = std::pin::Pin<
        Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>,
    >;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            // Clone the session since Node::new takes ownership
            let session = self.session.clone();
            Node::new(self.config, session, self.get_engine).await
        })
    }
}
