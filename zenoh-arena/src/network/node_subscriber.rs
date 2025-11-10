//! Subscriber for node data with deserialization

use crate::error::Result;
use crate::network::keyexpr::KeyexprLink;
use crate::node::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Subscribes to a Zenoh key expression and deserializes received data
///
/// This subscriber automatically deserializes received samples into type T.
/// Internally constructs a Link role keyexpr from the provided prefix and node ID.
/// Use `recv()` to get the next deserialized value.
pub struct NodeSubscriber<T> {
    subscriber: zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> std::fmt::Debug for NodeSubscriber<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeSubscriber")
            .field("type", &std::any::type_name::<T>())
            .field("key_expr", &self.subscriber.key_expr())
            .finish()
    }
}

impl<T> NodeSubscriber<T>
where
    T: zenoh_ext::Deserialize,
{
    /// Create a new subscriber for a Link keyexpr
    ///
    /// Immediately declares a Zenoh subscriber for the link keyexpr constructed from
    /// the given prefix and node ID. The keyexpr pattern will be:
    /// `<prefix>/link/<node_id>/*` (sender_id=node_id, receiver_id=wildcard)
    /// to receive all messages for the specified node (as sender).
    pub async fn new(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        node_id: &NodeId,
    ) -> Result<Self> {
        // Construct Link keyexpr: <prefix>/link/<node_id>/* (sender_id, receiver_id=*)
        let node_keyexpr = KeyexprLink::new(prefix, Some(node_id.clone()), None);
        let keyexpr: KeyExpr = node_keyexpr.into();

        let subscriber = session
            .declare_subscriber(keyexpr)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(Self {
            subscriber,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Receive and deserialize the next value
    ///
    /// Waits for the next sample from the subscriber and deserializes it into type T.
    /// Returns an error if reception fails or deserialization fails.
    pub async fn recv(&mut self) -> Result<T> {
        let sample = self
            .subscriber
            .recv_async()
            .await
            .map_err(|e| crate::error::ArenaError::Internal(format!("Failed to receive sample: {}", e)))?;

        let value: T = zenoh_ext::z_deserialize(sample.payload())
            .map_err(|e| crate::error::ArenaError::Serialization(format!("Failed to deserialize: {}", e)))?;

        Ok(value)
    }
}
