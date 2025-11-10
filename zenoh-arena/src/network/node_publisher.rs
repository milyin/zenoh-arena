//! Publisher for node data with serialization

use crate::error::Result;
use crate::network::keyexpr::KeyexprLink;
use crate::node::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Publishes to a Zenoh key expression with automatic serialization
///
/// This publisher automatically serializes data of type T before publishing.
/// Internally constructs a Link role keyexpr from the provided prefix and node IDs.
/// Use `put()` to publish a serialized value.
pub struct NodePublisher<T> {
    publisher: zenoh::pubsub::Publisher<'static>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> std::fmt::Debug for NodePublisher<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodePublisher")
            .field("type", &std::any::type_name::<T>())
            .field("key_expr", &self.publisher.key_expr())
            .finish()
    }
}

impl<T> NodePublisher<T>
where
    T: zenoh_ext::Serialize,
{
    /// Create a new publisher for a Link keyexpr
    ///
    /// Immediately declares a Zenoh publisher for the link keyexpr constructed from
    /// the given prefix, sender node ID, and receiver node ID. The keyexpr pattern will be:
    /// `<prefix>/link/<sender_id>/<receiver_id>` (sender_id, receiver_id)
    /// to send messages to the specified remote node.
    pub async fn new(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        sender_id: &NodeId,
        receiver_id: &NodeId,
    ) -> Result<Self> {
        // Construct Link keyexpr: <prefix>/link/<sender_id>/<receiver_id>
        let node_keyexpr = KeyexprLink::new(
            prefix,
            Some(sender_id.clone()),
            Some(receiver_id.clone()),
        );
        let keyexpr: KeyExpr = node_keyexpr.into();

        let publisher = session
            .declare_publisher(keyexpr)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(Self {
            publisher,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Publish a serialized value
    ///
    /// Serializes the value into a ZBytes payload and publishes it.
    /// Returns an error if serialization or publishing fails.
    pub async fn put(&self, value: &T) -> Result<()> {
        let payload = zenoh_ext::z_serialize(value);

        self.publisher
            .put(payload)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(())
    }
}
